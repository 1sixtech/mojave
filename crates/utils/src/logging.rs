use tracing::Level;
use tracing_subscriber::{EnvFilter, FmtSubscriber, filter::Directive};

pub fn init_logging(log_level: Level) {
    let log_filter = EnvFilter::builder()
        .with_default_directive(Directive::from(log_level))
        .from_env_lossy();
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(log_filter)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}
