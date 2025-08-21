use crate::error::BlockWatcherError;

mod block_watcher;
mod block_watcher_builder;
mod block_watcher_handle;

type Result<T> = core::result::Result<T, BlockWatcherError>;

pub use block_watcher::*;
pub use block_watcher_builder::*;
pub use block_watcher_handle::*;
