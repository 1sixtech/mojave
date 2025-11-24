use kube::Client;
use kube_leader_election::{LeaseLock, LeaseLockParams};
use std::{env, result::Result::*, time::Duration};
use tokio::time::sleep;

use mojave_batch_producer::{BatchProducer, types::Request as BatchProducerRequest};
use mojave_batch_submitter::committer::Committer;
use mojave_block_producer::{
    BlockProducer,
    types::{BlockProducerOptions, Request as BlockProducerRequest},
};
use mojave_node_lib::{
    types::{MojaveNode, NodeConfigFile},
    utils::store_node_config_file,
};
use mojave_proof_coordinator::{ProofCoordinator, types::ProofCoordinatorOptions};
use mojave_task::{Runner, Task, TaskHandle};
use tracing::{error, info};

pub struct LeaderTasks {
    batch: TaskHandle<BatchProducer>,
    block: TaskHandle<BlockProducer>,
    proof: TaskHandle<ProofCoordinator>,
    committer: tokio::task::JoinHandle<Result<(), mojave_batch_submitter::error::Error>>,
}

const BLOCK_PRODUCER_CAPACITY: usize = 100;

pub async fn run_with_k8s_coordination(
    node: MojaveNode,
    node_options: mojave_node_lib::types::NodeOptions,
    block_producer_options: BlockProducerOptions,
    proof_coordinator_options: ProofCoordinatorOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::try_default().await?;
    let identity = env::var("POD_NAME").unwrap_or_else(|_| "sequencer-pod".to_string());
    let namespace = env::var("POD_NAMESPACE").unwrap_or_else(|_| "default".to_string());

    let lease_name = env::var("LEASE_NAME").unwrap_or_else(|_| "sequencer-leader".to_string());
    let lease_ttl_sec = env::var("LEASE_TTL_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(15_u64);
    let renew_every_secs = lease_ttl_sec / 5; // 1/5 of TTL

    let lease_lock = LeaseLock::new(
        client,
        &namespace,
        LeaseLockParams {
            lease_name,
            holder_id: identity,
            lease_ttl: Duration::from_secs(lease_ttl_sec),
        },
    );

    let mut am_i_leader = false;
    let mut leader_tasks: Option<LeaderTasks> = None;

    loop {
        tokio::select! {
            _ = mojave_utils::signal::wait_for_shutdown_signal() => {
                info!("Termination signal received (K8s). Stopping leader tasks and exiting...");
                if am_i_leader {
                    if let Some(lt) = leader_tasks.take() {
                        stop_leader_tasks(lt).await?;
                    }
                    if let Err(err) = lease_lock.step_down().await {
                        error!("Error while stepping down from leader: {err:?}");
                    }
                }

                let (data_dir, _) = mojave_node_lib::utils::resolve_data_dir(&node_options.datadir).await?;
                let node_config_path = data_dir.join("node_config.json");
                info!("Storing config at {:?}...", node_config_path);
                let node_config = NodeConfigFile::new(node.peer_table.clone(), node.local_node_record.lock().await.clone()).await;
                store_node_config_file(node_config, node_config_path).await;

                info!("Shutdown complete.");
                break Ok(());
            }
            res = lease_lock.try_acquire_or_renew() => {
                match res {
                    Ok(res) => {
                        let became_leader = res.acquired_lease;

                        if !am_i_leader && became_leader {
                            // GW - Do I need to add Sleep here to wait all the leader task stop?
                            sleep(Duration::from_secs(2)).await;

                            info!("Became a leader. Start leader tasks");
                            leader_tasks = Some(
                                start_leader_tasks(
                                    node.clone(),
                                    &node_options,
                                    &block_producer_options,
                                    &proof_coordinator_options,
                                )
                                .await?,
                            );
                            am_i_leader = true;
                        } else if !became_leader && am_i_leader {
                            info!("Became a follower. Stop leader tasks");
                            if let Some(lt) = leader_tasks.take() {
                                stop_leader_tasks(lt).await?;
                            }
                            am_i_leader = false;
                        }

                        sleep(Duration::from_secs(renew_every_secs)).await;
                    }
                    Err(err) => {
                        error!("Error while k8s leader election: {err:?}");
                        break Err(Box::new(err));
                    }
                }
            }
        }
    }
}

pub async fn start_leader_tasks(
    node: MojaveNode,
    node_options: &mojave_node_lib::types::NodeOptions,
    block_producer_options: &BlockProducerOptions,
    proof_coordinator_options: &ProofCoordinatorOptions,
) -> Result<LeaderTasks, Box<dyn std::error::Error>> {
    let cancel_token = node.cancel_token.clone();

    // TODO: replace by implementation backed by a real queue
    let q = mojave_msgio::dummy::Dummy;

    let batch_counter = node.rollup_store.get_batch_number().await?.unwrap_or(0);
    let batch_producer = BatchProducer::new(node.clone(), batch_counter);
    let block_producer = BlockProducer::new(node.clone());
    let proof_coordinator =
        ProofCoordinator::new(node.clone(), node_options, proof_coordinator_options)?;

    let batch = batch_producer
        .clone()
        .spawn_periodic(Duration::from_millis(100_000), || {
            BatchProducerRequest::BuildBatch
        });

    let block = block_producer.spawn_with_capacity_periodic(
        BLOCK_PRODUCER_CAPACITY,
        Duration::from_millis(block_producer_options.block_time),
        || BlockProducerRequest::BuildBlock,
    );

    let committer = Runner::new(
        Committer::new(batch_producer.subscribe(), q, node.p2p_context.clone()),
        cancel_token.clone(),
    )
    .spawn();

    let proof = proof_coordinator.spawn();

    Ok(LeaderTasks {
        batch,
        block,
        proof,
        committer,
    })
}

pub async fn stop_leader_tasks(lt: LeaderTasks) -> Result<(), Box<dyn std::error::Error>> {
    lt.batch.shutdown().await?;
    lt.block.shutdown().await?;
    lt.proof.shutdown().await?;
    let _ = lt.committer.await?;
    Ok(())
}

pub fn is_k8s_env() -> bool {
    match std::env::var("KUBERNETES_SERVICE_HOST") {
        Ok(_) => {
            info!("Starting service as K8s version");
            true
        }
        _ => false,
    }
}
