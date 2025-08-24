use crate::error::WatcherError;

mod watcher;
mod watcher_builder;
mod watcher_handle;

type Result<T, M> = core::result::Result<T, WatcherError<M>>;

pub use watcher::*;
pub use watcher_builder::*;
pub use watcher_handle::*;
