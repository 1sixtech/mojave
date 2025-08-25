use std::sync::OnceLock;
use tracing::Level;
use tracing_subscriber::{
    EnvFilter, Registry, filter::Directive, fmt, layer::SubscriberExt, reload,
};

static RELOAD_HANDLE: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

pub fn init() {
    let base_filter = EnvFilter::builder()
        .with_default_directive(Directive::from(Level::INFO))
        .from_env_lossy();

    let (filter_layer, handle) = reload::Layer::new(base_filter);

    let subscriber = Registry::default().with(filter_layer).with(fmt::layer());

    RELOAD_HANDLE.set(handle).ok();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

pub fn change_level(log_level: Level) {
    if let Some(handle) = RELOAD_HANDLE.get() {
        let new_filter = EnvFilter::default().add_directive(log_level.into());
        if let Err(e) = handle.modify(|filter| *filter = new_filter) {
            eprintln!("Failed to update log level: {e}");
        }
    } else {
        eprintln!("Logger not initialized; call init() first");
    }
}
