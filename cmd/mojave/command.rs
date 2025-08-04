use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use clap::Subcommand;
use ethrex::{
    initializers::{get_local_node_record, get_signer, init_blockchain, init_store},
    utils::{store_node_config_file, NodeConfigFile},
};
use ethrex_blockchain::BlockchainType;
use ethrex_common::types::ELASTICITY_MULTIPLIER;
use ethrex_p2p::network::peer_table;
use ethrex_vm::EvmEngine;
use mojave_block_builder::{BlockBuilder, BlockBuilderContext};
use mojave_chain_utils::resolve_datadir;
use mojave_networking::rpc::clients::mojave::Client;
use tokio::sync::Mutex;
use tokio_util::task::TaskTracker;

use crate::{
    full_node_options::FullNodeOptions,
    initializer::{
        get_local_p2p_node, init_full_node_rpc_api, init_metrics, init_rollup_store,
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
                let store = init_store(&data_dir, genesis.clone()).await;
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

                // Spawn block builder.
                let block_builder_context = BlockBuilderContext::new(
                    store.clone(),
                    blockchain.clone(),
                    rollup_store.clone(),
                    genesis.coinbase,
                    ELASTICITY_MULTIPLIER,
                );
                let block_builder = BlockBuilder::start(block_builder_context, 100);
                let addrs: Vec<String> = sequencer_opts
                    .full_node_addresses
                    .iter()
                    .map(|addr| format!("http://{addr}"))
                    .collect();
                let mojave_client = Client::new(&addrs)?;
                tokio::spawn(async move {
                    loop {
                        match block_builder.build_block().await {
                            Ok(block) => mojave_client
                                .send_broadcast_block(&block)
                                .await
                                .unwrap_or_else(|error| tracing::error!("{}", error)),
                            Err(error) => {
                                tracing::error!("Error {}", error);
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(sequencer_opts.block_time)).await;
                    }
                });

                // start_l2(store, rollup_store, blockchain, cfg)
                // tracker.spawn(l2_sequencer);

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
