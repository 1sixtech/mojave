use bitcoin::{Block, Transaction};

use crate::{
    sequence::Sequence,
    watch::{Watcher, WatcherBuilder, WatcherHandle},
};

pub type SequenceWatcher = Watcher<Sequence>;
pub type SequenceWatcherBuilder = WatcherBuilder<Sequence>;
pub type SequenceWatcherHandle = WatcherHandle<Sequence>;

pub type BlockWatcher = Watcher<Block>;
pub type BlockWatcherBuilder = WatcherBuilder<Block>;
pub type BlockWatcherHandle = WatcherHandle<Block>;

pub type TransactionWatcher = Watcher<Transaction>;
pub type TransactionWatcherBuilder = WatcherBuilder<Transaction>;
pub type TransactionWatcherHandle = WatcherHandle<Transaction>;
