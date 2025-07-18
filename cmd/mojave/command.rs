use std::{future::IntoFuture, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use clap::Subcommand;
use ethrex::{
    initializers::{get_local_node_record, get_signer, init_blockchain, init_store},
    utils::{store_node_config_file, NodeConfigFile},
};
use ethrex_blockchain::BlockchainType;
use ethrex_l2::SequencerConfig;
use ethrex_p2p::network::peer_table;
use ethrex_vm::EvmEngine;
use mojave_chain_utils::resolve_datadir;
use tokio::sync::Mutex;
use tokio_util::task::TaskTracker;

use crate::{
    full_node_options::FullNodeOptions,
    initializer::{
        get_local_p2p_node, init_full_node_rpc_api, init_metrics, init_network, init_rollup_store,
        init_sequencer_rpc_api,
    },
    options::Options,
    sequencer_options::SequencerOpts,
};

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(name = "full-node", about = "Run a full node")]
    FullNode {
        #[command(flatten)]
        opts: Options,
        #[command(flatten)]
        full_node_opts: FullNodeOptions,
    },
    #[command(name = "sequencer", about = "Run a sequencer")]
    Sequencer {
        #[command(flatten)]
        opts: Options,
        #[command(flatten)]
        sequencer_opts: SequencerOpts,
    },
    #[cfg(feature = "generate-key-pair")]
    #[command(name = "generate-key-pair", about = "Show help information")]
    GenerateKeyPair {},
}

impl Command {
    pub async fn run(self) -> Result<()> {
        match self {
            Command::FullNode {
                opts,
                full_node_opts,
            } => {
                if opts.evm == EvmEngine::REVM {
                    panic!("Mojave doesn't support REVM, use LEVM instead.");
                }

                let data_dir = resolve_datadir(&opts.datadir);
                let rollup_store_dir = data_dir.clone() + "/rollup_store";

                let genesis = opts.network.get_genesis()?;
                let store = init_store(&data_dir, genesis).await;
                let rollup_store = init_rollup_store(&rollup_store_dir).await;

                let blockchain = init_blockchain(opts.evm, store.clone(), BlockchainType::L2);

                let signer = get_signer(&data_dir);

                let local_p2p_node = get_local_p2p_node(&opts, &signer);

                let local_node_record = Arc::new(Mutex::new(get_local_node_record(
                    &data_dir,
                    &local_p2p_node,
                    &signer,
                )));

                let peer_table = peer_table(local_p2p_node.node_id());

                // TODO: Check every module starts properly.
                let tracker = TaskTracker::new();

                let cancel_token = tokio_util::sync::CancellationToken::new();

                init_full_node_rpc_api(
                    &opts,
                    &full_node_opts,
                    peer_table.clone(),
                    local_p2p_node.clone(),
                    local_node_record.lock().await.clone(),
                    store.clone(),
                    blockchain.clone(),
                    cancel_token.clone(),
                    tracker.clone(),
                    rollup_store.clone(),
                )
                .await;

                // Initialize metrics if enabled
                if opts.metrics_enabled {
                    init_metrics(&opts, tracker.clone());
                }

                if opts.p2p_enabled {
                    init_network(
                        &opts,
                        &opts.network,
                        &data_dir,
                        local_p2p_node,
                        local_node_record.clone(),
                        signer,
                        peer_table.clone(),
                        store.clone(),
                        tracker.clone(),
                        blockchain.clone(),
                    )
                    .await;
                } else {
                    tracing::info!("P2P is disabled");
                }

                let l2_sequencer_cfg = SequencerConfig::from(opts.sequencer_opts);

                let l2_sequencer = ethrex_l2::start_l2(
                    store,
                    rollup_store,
                    blockchain,
                    l2_sequencer_cfg,
                    #[cfg(feature = "metrics")]
                    format!("http://{}:{}", opts.http_addr, opts.http_port),
                )
                .into_future();

                tracker.spawn(l2_sequencer);

                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        tracing::info!("Server shut down started...");
                        let node_config_path = PathBuf::from(data_dir + "/node_config.json");
                        tracing::info!("Storing config at {:?}...", node_config_path);
                        cancel_token.cancel();
                        let node_config = NodeConfigFile::new(peer_table, local_node_record.lock().await.clone()).await;
                        store_node_config_file(node_config, node_config_path).await;
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        tracing::info!("Server shutting down!");
                    }
                }
            }
            Command::Sequencer {
                opts,
                sequencer_opts,
            } => {
                if opts.evm == EvmEngine::REVM {
                    panic!("Mojave doesn't support REVM, use LEVM instead.");
                }

                let data_dir = resolve_datadir(&opts.datadir);
                let rollup_store_dir = data_dir.clone() + "/rollup_store";

                let genesis = opts.network.get_genesis()?;
                let store = init_store(&data_dir, genesis).await;
                let rollup_store = init_rollup_store(&rollup_store_dir).await;

                let blockchain = init_blockchain(opts.evm, store.clone(), BlockchainType::L2);

                let signer = get_signer(&data_dir);

                let local_p2p_node = get_local_p2p_node(&opts, &signer);

                let local_node_record = Arc::new(Mutex::new(get_local_node_record(
                    &data_dir,
                    &local_p2p_node,
                    &signer,
                )));

                let peer_table = peer_table(local_p2p_node.node_id());

                // TODO: Check every module starts properly.
                let tracker = TaskTracker::new();

                let cancel_token = tokio_util::sync::CancellationToken::new();

                init_sequencer_rpc_api(
                    &opts,
                    &sequencer_opts,
                    peer_table.clone(),
                    local_p2p_node.clone(),
                    local_node_record.lock().await.clone(),
                    store.clone(),
                    blockchain.clone(),
                    cancel_token.clone(),
                    tracker.clone(),
                    rollup_store.clone(),
                )
                .await;

                // Initialize metrics if enabled
                if opts.metrics_enabled {
                    init_metrics(&opts, tracker.clone());
                }

                let l2_sequencer_cfg = SequencerConfig::from(opts.sequencer_opts);

                let l2_sequencer = ethrex_l2::start_l2(
                    store,
                    rollup_store,
                    blockchain,
                    l2_sequencer_cfg,
                    #[cfg(feature = "metrics")]
                    format!("http://{}:{}", opts.http_addr, opts.http_port),
                )
                .into_future();

                tracker.spawn(l2_sequencer);

                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        tracing::info!("Server shut down started...");
                        let node_config_path = PathBuf::from(data_dir + "/node_config.json");
                        tracing::info!("Storing config at {:?}...", node_config_path);
                        cancel_token.cancel();
                        let node_config = NodeConfigFile::new(peer_table, local_node_record.lock().await.clone()).await;
                        store_node_config_file(node_config, node_config_path).await;
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        tracing::info!("Server shutting down!");
                    }
                }
            }
            #[cfg(feature = "generate-key-pair")]
            Command::GenerateKeyPair {} => {
                println!("Generating key pair...");
                use ed25519_dalek::SigningKey;
                use rand::rngs::OsRng;
                use std::{fs::OpenOptions, io::Write};

                let mut csprng = OsRng;
                let signing_key: SigningKey = SigningKey::generate(&mut csprng);
                let verifying_key = signing_key.verifying_key();

                let public_hex = hex::encode(verifying_key.as_bytes());
                let secret_hex = hex::encode(signing_key.to_bytes());

                let mut env_file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(".env")
                    .expect("failed to open .env");

                writeln!(
                    env_file,
                    "PUBLIC_KEY={public_hex}\nPRIVATE_KEY={secret_hex}\n"
                )
                .expect("failed to write to .env");
            }
        }
        Ok(())
    }
}
