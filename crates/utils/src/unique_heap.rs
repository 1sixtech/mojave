use std::{
    cmp::Ord,
    collections::{BinaryHeap, HashSet},
    hash::Hash,
    sync::Arc,
};
use tokio::sync::{Notify, RwLock};

pub trait UniqueHeapItem<K>
where
    K: Eq + Hash + Clone,
{
    fn key(&self) -> K;
}

#[derive(Debug)]
struct InnerHeap<T, K>
where
    K: Eq + Hash + Clone,
    T: Ord + UniqueHeapItem<K> + Clone,
{
    heap: BinaryHeap<T>,
    keys: HashSet<K>,
}

impl<T, K> InnerHeap<T, K>
where
    K: Eq + Hash + Clone,
    T: Ord + UniqueHeapItem<K> + Clone,
{
    fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            keys: HashSet::new(),
        }
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(capacity),
            keys: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AsyncUniqueHeap<T, K>
where
    K: Eq + Hash + Clone,
    T: Ord + UniqueHeapItem<K> + Clone,
{
    inner: Arc<RwLock<InnerHeap<T, K>>>,
    notify: Arc<Notify>,
}

impl<T, K> AsyncUniqueHeap<T, K>
where
    K: Eq + Hash + Clone,
    T: Ord + UniqueHeapItem<K> + Clone,
{
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(InnerHeap::new())),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(InnerHeap::with_capacity(capacity))),
            notify: Arc::new(Notify::new()),
        }
    }

    pub async fn push(&self, item: T) -> bool {
        let key = item.key();
        let mut inner = self.inner.write().await;
        if inner.keys.insert(key) {
            inner.heap.push(item);
            self.notify.notify_one();
            true
        } else {
            false
        }
    }

    pub async fn pop(&self) -> Option<T> {
        let mut inner = self.inner.write().await;
        if let Some(item) = inner.heap.pop() {
            let key = item.key();
            inner.keys.remove(&key);
            Some(item)
        } else {
            None
        }
    }

    pub async fn pop_wait(&self) -> T {
        loop {
            if let Some(item) = self.pop().await {
                return item;
            }
            self.notify.notified().await;
        }
    }

    pub async fn peek(&self) -> Option<T> {
        let inner = self.inner.read().await;
        inner.heap.peek().cloned()
    }

    pub async fn peek_wait(&self) -> T {
        loop {
            if let Some(block) = self.peek().await {
                return block;
            }
            self.notify.notified().await;
        }
    }

    pub async fn len(&self) -> usize {
        let inner = self.inner.read().await;
        inner.heap.len()
    }

    pub async fn is_empty(&self) -> bool {
        let inner = self.inner.read().await;
        inner.heap.is_empty()
    }
}

impl<T, K> Default for AsyncUniqueHeap<T, K>
where
    K: Eq + Hash + Clone,
    T: Ord + UniqueHeapItem<K> + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, sleep};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct TestItem {
        priority: i32,
        key: String,
    }

    impl TestItem {
        fn new(priority: i32, key: &str) -> Self {
            Self {
                priority,
                key: key.to_string(),
            }
        }
    }

    impl UniqueHeapItem<String> for TestItem {
        fn key(&self) -> String {
            self.key.clone()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct NumericItem {
        priority: i32,
        id: i32,
    }

    impl NumericItem {
        fn new(priority: i32, id: i32) -> Self {
            Self { priority, id }
        }
    }

    impl UniqueHeapItem<i32> for NumericItem {
        fn key(&self) -> i32 {
            self.id
        }
    }

    #[tokio::test]
    async fn test_new() {
        let heap: AsyncUniqueHeap<TestItem, String> = AsyncUniqueHeap::new();
        assert!(heap.is_empty().await);
        assert_eq!(heap.len().await, 0);
    }

    #[tokio::test]
    async fn test_default() {
        let heap: AsyncUniqueHeap<TestItem, String> = AsyncUniqueHeap::default();
        assert!(heap.is_empty().await);
        assert_eq!(heap.len().await, 0);
    }

    #[tokio::test]
    async fn test_with_capacity() {
        let heap: AsyncUniqueHeap<TestItem, String> = AsyncUniqueHeap::with_capacity(10);
        assert!(heap.is_empty().await);
        assert_eq!(heap.len().await, 0);
    }

    #[tokio::test]
    async fn test_push_single_item() {
        let heap = AsyncUniqueHeap::new();
        let item = TestItem::new(10, "key1");
        let result = heap.push(item).await;
        assert!(result);
        assert_eq!(heap.len().await, 1);
        assert!(!heap.is_empty().await);
    }

    #[tokio::test]
    async fn test_push_multiple_items() {
        let heap = AsyncUniqueHeap::new();
        assert!(heap.push(TestItem::new(10, "key1")).await);
        assert!(heap.push(TestItem::new(20, "key2")).await);
        assert!(heap.push(TestItem::new(5, "key3")).await);
        assert_eq!(heap.len().await, 3);
    }

    #[tokio::test]
    async fn test_push_duplicate_key() {
        let heap = AsyncUniqueHeap::new();
        assert!(heap.push(TestItem::new(10, "key1")).await);
        assert!(!heap.push(TestItem::new(20, "key1")).await);
        assert_eq!(heap.len().await, 1);
    }

    #[tokio::test]
    async fn test_push_same_priority_different_keys() {
        let heap = AsyncUniqueHeap::new();
        assert!(heap.push(TestItem::new(10, "key1")).await);
        assert!(heap.push(TestItem::new(10, "key2")).await);
        assert_eq!(heap.len().await, 2);
    }

    #[tokio::test]
    async fn test_pop_empty_heap() {
        let heap: AsyncUniqueHeap<TestItem, String> = AsyncUniqueHeap::new();
        let result = heap.pop().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_pop_single_item() {
        let heap = AsyncUniqueHeap::new();
        let item = TestItem::new(10, "key1");
        heap.push(item.clone()).await;
        let result = heap.pop().await;
        assert_eq!(result, Some(item));
        assert!(heap.is_empty().await);
        assert_eq!(heap.len().await, 0);
    }

    #[tokio::test]
    async fn test_pop_maintains_max_heap_order() {
        let heap = AsyncUniqueHeap::new();
        heap.push(TestItem::new(10, "key1")).await;
        heap.push(TestItem::new(30, "key2")).await;
        heap.push(TestItem::new(20, "key3")).await;
        heap.push(TestItem::new(5, "key4")).await;

        // Should pop in descending order (max-heap behavior)
        assert_eq!(heap.pop().await.map(|item| item.priority), Some(30));
        assert_eq!(heap.pop().await.map(|item| item.priority), Some(20));
        assert_eq!(heap.pop().await.map(|item| item.priority), Some(10));
        assert_eq!(heap.pop().await.map(|item| item.priority), Some(5));
        assert!(heap.is_empty().await);
    }

    #[tokio::test]
    async fn test_pop_removes_key_from_existing_keys() {
        let heap = AsyncUniqueHeap::new();
        heap.push(TestItem::new(10, "key1")).await;
        heap.pop().await;

        // Should be able to push with the same key again
        assert!(heap.push(TestItem::new(20, "key1")).await);
    }

    #[tokio::test]
    async fn test_peek() {
        let heap = AsyncUniqueHeap::new();

        // Peek empty heap
        assert!(heap.peek().await.is_none());

        // Push items
        heap.push(TestItem::new(30, "key1")).await;
        heap.push(TestItem::new(10, "key2")).await;
        heap.push(TestItem::new(20, "key3")).await;

        let peeked = heap.peek().await;
        assert_eq!(peeked.as_ref().map(|item| item.priority), Some(30));

        // Length should remain unchanged
        assert_eq!(heap.len().await, 3);

        // Pop should return the same item
        let popped = heap.pop().await;
        assert_eq!(peeked, popped);
    }

    #[tokio::test]
    async fn test_len_empty_heap() {
        let heap: AsyncUniqueHeap<TestItem, String> = AsyncUniqueHeap::new();
        assert_eq!(heap.len().await, 0);
    }

    #[tokio::test]
    async fn test_len_with_items() {
        let heap = AsyncUniqueHeap::new();
        assert_eq!(heap.len().await, 0);

        heap.push(TestItem::new(10, "key1")).await;
        assert_eq!(heap.len().await, 1);

        heap.push(TestItem::new(20, "key2")).await;
        assert_eq!(heap.len().await, 2);

        heap.pop().await;
        assert_eq!(heap.len().await, 1);
    }

    #[tokio::test]
    async fn test_is_empty_new_heap() {
        let heap: AsyncUniqueHeap<TestItem, String> = AsyncUniqueHeap::new();
        assert!(heap.is_empty().await);
    }

    #[tokio::test]
    async fn test_is_empty_with_items() {
        let heap = AsyncUniqueHeap::new();
        assert!(heap.is_empty().await);

        heap.push(TestItem::new(10, "key1")).await;
        assert!(!heap.is_empty().await);

        heap.pop().await;
        assert!(heap.is_empty().await);
    }

    #[tokio::test]
    async fn test_pop_wait() {
        let heap = Arc::new(AsyncUniqueHeap::new());
        let heap_clone = heap.clone();

        // Spawn a task that will push an item after a delay
        let push_task = tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            heap_clone.push(TestItem::new(42, "delayed_key")).await;
        });

        // This should wait until the item is pushed
        let result = heap.pop_wait().await;
        assert_eq!(result.priority, 42);
        assert_eq!(result.key(), "delayed_key".to_string());

        push_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_complex_scenario() {
        let heap = AsyncUniqueHeap::new();

        // Push items
        assert!(heap.push(TestItem::new(50, "a")).await);
        assert!(heap.push(TestItem::new(30, "b")).await);
        assert!(heap.push(TestItem::new(70, "c")).await);
        assert_eq!(heap.len().await, 3);

        // Try to push duplicate
        assert!(!heap.push(TestItem::new(40, "a")).await);
        assert_eq!(heap.len().await, 3);

        // Pop highest priority item (max-heap behavior)
        assert_eq!(heap.pop().await.map(|item| item.priority), Some(70));
        assert_eq!(heap.len().await, 2);

        // Reuse the key
        assert!(heap.push(TestItem::new(80, "c")).await);
        assert_eq!(heap.len().await, 3);

        // Pop all remaining items in order
        assert_eq!(heap.pop().await.map(|item| item.priority), Some(80));
        assert_eq!(heap.pop().await.map(|item| item.priority), Some(50));
        assert_eq!(heap.pop().await.map(|item| item.priority), Some(30));
        assert!(heap.is_empty().await);
    }

    #[tokio::test]
    async fn test_different_key_types() {
        let heap: AsyncUniqueHeap<NumericItem, i32> = AsyncUniqueHeap::new();
        assert!(heap.push(NumericItem::new(10, 1)).await);
        assert!(heap.push(NumericItem::new(20, 2)).await);
        assert!(!heap.push(NumericItem::new(15, 1)).await); // Duplicate key
        assert_eq!(heap.len().await, 2);
    }

    #[tokio::test]
    async fn test_trait_implementation() {
        let item = TestItem::new(42, "test_key");
        assert_eq!(item.key(), "test_key".to_string());

        let numeric_item = NumericItem::new(100, 5);
        assert_eq!(numeric_item.key(), 5);
    }

    #[tokio::test]
    async fn test_ordering_with_equal_priorities() {
        let heap = AsyncUniqueHeap::new();

        // Items with same priority but different keys
        heap.push(TestItem::new(10, "key1")).await;
        heap.push(TestItem::new(10, "key2")).await;
        heap.push(TestItem::new(10, "key3")).await;

        assert_eq!(heap.len().await, 3);

        // All should have same priority when popped
        let first = heap.pop().await.unwrap();
        let second = heap.pop().await.unwrap();
        let third = heap.pop().await.unwrap();

        assert_eq!(first.priority, 10);
        assert_eq!(second.priority, 10);
        assert_eq!(third.priority, 10);

        // Keys should be different
        let mut keys = vec![first.key(), second.key(), third.key()];
        keys.sort();
        assert_eq!(
            keys,
            vec!["key1".to_string(), "key2".to_string(), "key3".to_string()]
        );
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let heap = Arc::new(AsyncUniqueHeap::new());
        let mut handles = Vec::new();

        // Spawn multiple tasks that push items concurrently
        for i in 0..10 {
            let heap_clone = heap.clone();
            let handle = tokio::spawn(async move {
                heap_clone.push(TestItem::new(i, &format!("key_{i}"))).await
            });
            handles.push(handle);
        }

        // Wait for all pushes to complete
        for handle in handles {
            assert!(handle.await.unwrap());
        }

        assert_eq!(heap.len().await, 10);

        // Pop all items
        let mut popped_values = Vec::new();
        while let Some(item) = heap.pop().await {
            popped_values.push(item.priority);
        }

        // Should be in descending order due to max-heap behavior
        popped_values.sort_by(|a, b| b.cmp(a));
        assert_eq!(popped_values, (0..10).rev().collect::<Vec<i32>>());
        assert!(heap.is_empty().await);
    }
}
