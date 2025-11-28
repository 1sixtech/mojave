use std::time::Duration;

use mojave_batch_producer::{BatchProducer, types::Request as BatchRequest};
use mojave_block_producer::{
    BlockProducer,
    types::{BlockProducerOptions, Request as BlockRequest},
};
use mojave_node_lib::types::{MojaveNode, NodeOptions};
use mojave_proof_coordinator::{ProofCoordinator, types::ProofCoordinatorOptions};
use mojave_task::{Task, TaskHandle};
use mojave_utils::{network::get_http_socket_addr, signal::wait_for_shutdown_signal};
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{k8s::run_with_k8s_coordination, utils::is_k8s_env};

pub struct LeaderTasks {
    batch: TaskHandle<BatchProducer>,
    block: TaskHandle<BlockProducer>,
    proof: TaskHandle<ProofCoordinator>,
}

const BLOCK_PRODUCER_CAPACITY: usize = 100;

async fn run_sequencer_leader_task(
    node: MojaveNode,
    options: &NodeOptions,
    block_producer_options: &BlockProducerOptions,
    proof_coordinator_options: &ProofCoordinatorOptions,
    cancel_token: CancellationToken,
) {
    info!("Starting Sequencer leader task...");

    let leader_tasks = start_leader_tasks(
        node,
        options,
        block_producer_options,
        proof_coordinator_options,
        cancel_token.clone(),
    )
    .await
    .expect("Failed to start leader tasks");

    cancel_token.cancelled().await;
    info!("Shutdown token triggered, stopping sequencer leader tasks...");

    stop_leader_tasks(leader_tasks)
        .await
        .expect("Failed to stop leader tasks");

    info!("Sequencer leader tasks stopped.");
}

pub async fn run_sequencer(
    node: MojaveNode,
    options: &NodeOptions,
    block_producer_options: &BlockProducerOptions,
    proof_coordinator_options: &ProofCoordinatorOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    if is_k8s_env() {
        run_with_k8s_coordination(move |shutdown_token: CancellationToken| {
            let node_task = node.clone();
            let options_task = options.clone();
            let block_producer_options_task = block_producer_options.clone();
            let proof_coordinator_options_task = proof_coordinator_options.clone();

            async move {
                run_sequencer_leader_task(
                    node_task,
                    &options_task,
                    &block_producer_options_task,
                    &proof_coordinator_options_task,
                    shutdown_token,
                )
                .await
            }
        })
        .await?;
    } else {
        info!("Starting Sequencer in standalone mode...");

        let shutdown = CancellationToken::new();
        let shutdown_for_task = shutdown.clone();
        let node_task = node.clone();
        let options_task = options.clone();
        let block_producer_options_task = block_producer_options.clone();
        let proof_coordinator_options_task = proof_coordinator_options.clone();

        let leader_task = tokio::spawn(async move {
            run_sequencer_leader_task(
                node_task,
                &options_task,
                &block_producer_options_task,
                &proof_coordinator_options_task,
                shutdown_for_task,
            )
            .await;
        });

        select! {
            _ = wait_for_shutdown_signal() => {
                info!("Termination signal received, shutting down sequencer...");
                shutdown.cancel();

                match leader_task.await {
                    Ok(_) => info!("Leader task shut down gracefully."),
                    Err(err) => error!("Error while awaiting leader task shutdown: {err:?}"),
                }
            }
        }
    }
    Ok(())
}

async fn start_leader_tasks(
    node: MojaveNode,
    options: &NodeOptions,
    block_producer_options: &BlockProducerOptions,
    proof_coordinator_options: &ProofCoordinatorOptions,
    cancel_token: CancellationToken,
) -> Result<LeaderTasks, Box<dyn std::error::Error>> {
    let batch_counter = node.rollup_store.get_batch_number().await?.unwrap_or(0);
    let batch_producer = BatchProducer::new(node.clone(), batch_counter);
    let block_producer = BlockProducer::new(node.clone());
    let proof_coordinator =
        ProofCoordinator::new(node.clone(), options, proof_coordinator_options)?;

    let batch = batch_producer
        .clone()
        .spawn_periodic(Duration::from_millis(100_000), || BatchRequest::BuildBatch);

    let block = block_producer.spawn_with_capacity_periodic(
        BLOCK_PRODUCER_CAPACITY,
        Duration::from_millis(block_producer_options.block_time),
        || BlockRequest::BuildBlock,
    );

    let proof = proof_coordinator.spawn();

    // Health probe HTTP endpoint.
    let health_socket_addr =
        get_http_socket_addr(&options.health_addr, &options.health_port).await?;
    let _ = mojave_utils::health::spawn_health_probe(
        health_socket_addr,
        cancel_token.cancelled_owned(),
    )
    .await?;

    Ok(LeaderTasks {
        batch,
        block,
        proof,
    })
}

async fn stop_leader_tasks(lt: LeaderTasks) -> Result<(), Box<dyn std::error::Error>> {
    lt.batch.shutdown().await?;
    lt.block.shutdown().await?;
    lt.proof.shutdown().await?;
    Ok(())
}
