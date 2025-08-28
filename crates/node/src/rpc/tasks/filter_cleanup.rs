use std::time::Duration;

use ethrex_rpc::ActiveFilters;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;


pub const FILTER_DURATION: Duration = {
    if cfg!(test) {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(5 * 60)
    }
};

pub(crate) fn spawn_filter_cleanup_task(
    active_filters: ActiveFilters,
    shutdown_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let mut interval = tokio::time::interval(FILTER_DURATION);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    tracing::info!("Running filter clean task");
                    ethrex_rpc::clean_outdated_filters(active_filters.clone(), FILTER_DURATION);
                    tracing::info!("Filter clean task complete");
                }
                _ = shutdown_token.cancelled() => {
                    tracing::info!("Shutting down filter clean task");
                    break;
                }
            }
        }
    })
}
