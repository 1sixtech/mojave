use mojave_block_producer::types::BlockProducerOptions;
use mojave_node_lib::types::{MojaveNode, NodeOptions};
use mojave_proof_coordinator::types::ProofCoordinatorOptions;

use crate::{coordination_mode::CoordinationMode, utils::detect_coordination_mode};

/// Single entry point used by `main`: picks the right coordination mode
/// and runs the sequencer accordingly.
pub async fn run_sequencer_with_coordination(
    node: MojaveNode,
    node_options: &NodeOptions,
    block_producer_options: &BlockProducerOptions,
    proof_coordinator_options: &ProofCoordinatorOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    match detect_coordination_mode() {
        CoordinationMode::Kubernetes => {
            run_with_k8s_coordination(
                node,
                node_options,
                block_producer_options,
                proof_coordinator_options,
            )
            .await
        }
        CoordinationMode::Standalone => {
            run_without_k8s_coordination(
                node,
                node_options,
                block_producer_options,
                proof_coordinator_options,
            )
            .await
        }
    }
}
