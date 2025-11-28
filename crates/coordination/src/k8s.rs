use std::{env, time::Duration};

use kube::Client;
use kube_leader_election::{LeaseLock, LeaseLockParams};
use mojave_utils::signal::wait_for_shutdown_signal;
use tokio::{
    select,
    time::{MissedTickBehavior, interval},
};
use tracing::{error, info};

use crate::utils::is_current_leader;

/// Configuration for Kubernetes-based leader election, loaded from env vars.
struct K8sLeaderConfig {
    identity: String,
    namespace: String,
    lease_lock: LeaseLock,
    lease_name: String,
    renew_every_secs: u64,
}

impl K8sLeaderConfig {
    fn from_env(client: Client) -> Self {
        let identity = env::var("POD_NAME").unwrap_or_else(|_| "sequencer-pod".to_string());
        let namespace = env::var("POD_NAMESPACE").unwrap_or_else(|_| "default".to_string());
        let lease_name = env::var("LEASE_NAME").unwrap_or_else(|_| "sequencer-leader".to_string());

        let lease_ttl_secs = env::var("LEASE_TTL_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(15_u64);

        // Renew roughly every 1/5 of TTL, but never less than 1s.
        let renew_every_secs = std::cmp::max(1, lease_ttl_secs / 5);

        let lease_lock_params = LeaseLockParams {
            lease_name: lease_name.clone(),
            holder_id: identity.clone(),
            lease_ttl: Duration::from_secs(lease_ttl_secs),
        };

        let lease_lock = LeaseLock::new(client, &namespace, lease_lock_params);

        Self {
            identity,
            namespace,
            lease_name,
            renew_every_secs,
            lease_lock,
        }
    }

    fn lease_lock(&self) -> &LeaseLock {
        &self.lease_lock
    }
}

struct LeaderEpoch {
    cancel_token: tokio_util::sync::CancellationToken,
    handle: tokio::task::JoinHandle<()>,
}

/// Run K8s leader election and drive a generic "leader task".
///
/// - When this instance becomes leader, `spawn_leader_task` is called with a fresh
///   `CancellationToken`.
/// - When leadership is lost (or shutdown signal fires), the token is cancelled
///   and the leader task is awaited.
/// - This function returns when the **pod is shutting down** or on error.
///
/// The K8s code does **not** know anything about `MojaveNode`, `NodeOptions`, etc.
/// It only knows how to start/stop some async work via a `CancellationToken`.
pub async fn run_with_k8s_coordination<F, Fut>(
    spawn_leader_task: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(tokio_util::sync::CancellationToken) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let client = Client::try_default().await?;
    let lease_client = client.clone();
    let k8s_config = K8sLeaderConfig::from_env(lease_client);

    let lease_lock = k8s_config.lease_lock();

    let mut renew_interval = interval(Duration::from_secs(k8s_config.renew_every_secs));
    renew_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    // initial attempt to acquire leadership
    if let Err(err) = lease_lock.try_acquire_or_renew().await {
        error!("Error while k8s leader election: {err:?}");
        return Err(Box::new(err));
    }

    let mut am_i_leader = false;
    let mut epoch: Option<LeaderEpoch> = None;

    loop {
        select! {
            _ = wait_for_shutdown_signal() => {
                shutdown(epoch, am_i_leader, lease_lock).await;
                return Ok(());
            }

            _ = renew_interval.tick() => {
                match lease_lock.try_acquire_or_renew().await {
                    Ok(_) => {
                        let currently_leader = match is_current_leader(&client, &k8s_config.namespace, &k8s_config.lease_name, &k8s_config.identity).await {
                            Ok(is_leader) => is_leader,
                            Err(err) => {
                                error!("Error while checking leadership: {err:?}");
                                false
                            }
                        };

                        // beacoming the leader
                        if currently_leader && !am_i_leader {
                            info!("This pod is now the leader (K8s). Starting leader task...");
                            let cancel_token = tokio_util::sync::CancellationToken::new();
                            let fut = spawn_leader_task(cancel_token.clone());
                            let handle = tokio::spawn(async move {
                                fut.await;
                            });
                            epoch = Some(LeaderEpoch { cancel_token, handle });
                            am_i_leader = true;
                        }
                        // becoming a follower
                        else if !currently_leader && am_i_leader {
                            info!("This pod is no longer the leader (K8s). Stopping leader task...");

                            if let Some(LeaderEpoch { cancel_token, handle }) = epoch.take() {
                                cancel_token.cancel();
                                match handle.await {
                                    Ok(()) => {}
                                    Err(join_err) => error!("Leader task panicked: {join_err:?}"),
                                }
                            }
                            if let Err(err) = lease_lock.step_down().await {
                                error!("Error while stepping down from leader: {err:?}");
                            }
                            am_i_leader = false;
                        }
                    }
                    Err(err) => {
                        error!("Error while k8s leader election: {err:?}");
                        return Err(Box::new(err));
                    }
                }
            }
        }
    }
}

async fn shutdown(epoch: Option<LeaderEpoch>, am_i_leader: bool, lease_lock: &LeaseLock) {
    info!("Termination signal received (K8s). Stopping leader task and exiting...");

    if let Some(LeaderEpoch {
        cancel_token,
        handle,
    }) = epoch
    {
        cancel_token.cancel();
        match handle.await {
            Ok(()) => {}
            Err(join_err) => error!("Leader task panicked: {join_err:?}"),
        }
    }

    if am_i_leader && let Err(err) = lease_lock.step_down().await {
        error!("Error while stepping down from leader: {err:?}");
    }

    info!("K8s coordination loop exiting.");
}
