use std::env;

use k8s_openapi::api::coordination::v1::Lease;
use kube::{Api, Client};
use tracing::info;

use crate::coordination_mode::CoordinationMode;

/// Decide coordination mode based on environment (Kubernetes vs standalone).
pub fn detect_coordination_mode() -> CoordinationMode {
    if env::var("KUBERNETES_SERVICE_HOST").is_ok() {
        info!("Detected Kubernetes environment, using LeaseLock coordination");
        CoordinationMode::Kubernetes
    } else {
        info!("No Kubernetes detected, running in standalone mode");
        CoordinationMode::Standalone
    }
}

/// Backwards-compatible helper.
pub fn is_k8s_env() -> bool {
    matches!(detect_coordination_mode(), CoordinationMode::Kubernetes)
}

/// Check if the given identity is the current Lease holder.
async fn is_current_leader(
    client: &Client,
    namespace: &str,
    lease_name: &str,
    identity: &str,
) -> Result<bool, kube::Error> {
    let leases: Api<Lease> = Api::namespaced(client.clone(), namespace);
    let lease = leases.get(lease_name).await?;
    let holder = lease
        .spec
        .as_ref()
        .and_then(|spec| spec.holder_identity.clone());
    Ok(holder.as_deref() == Some(identity))
}
