use ethrex_common::types::Block;
use mojave_chain_utils::unique_heap::UniqueHeapItem;

/// A wrapper around a Block that provides ordering based on block number.
///
/// Blocks are ordered by their block number in ascending order, meaning
/// blocks with lower numbers have higher priority and will be processed first
/// when used in a priority queue or heap structure.
///
/// This ensures that blocks are processed in the correct sequential order,
/// with block 1 coming before block 2, block 2 before block 3, and so on.
///
/// # Examples
///
/// ```ignore
/// use ethrex_common::types::{Block, BlockHeader};
/// use ordered_block::OrderedBlock;
///
/// let block1 = OrderedBlock(Block::new(BlockHeader { number: 1, ..Default::default() }, Default::default()));
/// let block2 = OrderedBlock(Block::new(BlockHeader { number: 2, ..Default::default() }, Default::default()));
///
/// assert!(block1 > block2); // block1 has higher priority (lower number)
/// ```
#[derive(Debug, Clone)]
pub struct OrderedBlock(pub Block);

impl PartialEq for OrderedBlock {
    fn eq(&self, other: &Self) -> bool {
        self.0.header.number == other.0.header.number
    }
}

impl Eq for OrderedBlock {}

impl PartialOrd for OrderedBlock {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedBlock {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse ordering so that lower block numbers have higher priority
        // This ensures that when used in a max-heap, blocks with lower numbers
        // (which should be processed first) will be at the top
        other.0.header.number.cmp(&self.0.header.number)
    }
}

impl UniqueHeapItem<u64> for OrderedBlock {
    fn key(&self) -> u64 {
        self.0.header.number
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethrex_common::types::{BlockBody, BlockHeader};
    use mojave_chain_utils::unique_heap::AsyncUniqueHeap;

    fn create_test_block(number: u64) -> OrderedBlock {
        let header = BlockHeader {
            number,
            ..Default::default()
        };
        let body = BlockBody::default();
        OrderedBlock(Block::new(header, body))
    }

    #[test]
    fn test_ordered_block_equality() {
        let block1 = create_test_block(5);
        let block2 = create_test_block(5);
        let block3 = create_test_block(10);

        assert_eq!(block1, block2);
        assert_ne!(block1, block3);
    }

    #[test]
    fn test_ordered_block_ordering_lowest_first() {
        let block1 = create_test_block(1);
        let block2 = create_test_block(2);
        let block3 = create_test_block(3);
        let block10 = create_test_block(10);

        // Lower block numbers should have higher priority (greater in comparison)
        assert!(block1 > block2);
        assert!(block2 > block3);
        assert!(block1 > block10);
        assert!(block3 > block10);

        // Test partial ordering
        assert!(block1.partial_cmp(&block2) == Some(std::cmp::Ordering::Greater));
        assert!(block2.partial_cmp(&block1) == Some(std::cmp::Ordering::Less));
        assert!(block1.partial_cmp(&block1) == Some(std::cmp::Ordering::Equal));
    }

    #[test]
    fn test_unique_heap_item_key() {
        let block = create_test_block(42);
        assert_eq!(block.key(), 42);
    }

    #[tokio::test]
    async fn test_ordered_blocks_in_heap_lowest_first() {
        let heap = AsyncUniqueHeap::new();

        // Push blocks in random order
        heap.push(create_test_block(5)).await;
        heap.push(create_test_block(1)).await;
        heap.push(create_test_block(10)).await;
        heap.push(create_test_block(3)).await;
        heap.push(create_test_block(7)).await;

        assert_eq!(heap.len().await, 5);

        // Pop blocks - should come out in ascending order (lowest numbers first)
        let popped_numbers: Vec<u64> = vec![
            heap.pop().await.unwrap().0.header.number,
            heap.pop().await.unwrap().0.header.number,
            heap.pop().await.unwrap().0.header.number,
            heap.pop().await.unwrap().0.header.number,
            heap.pop().await.unwrap().0.header.number,
        ];

        assert_eq!(popped_numbers, vec![1, 3, 5, 7, 10]);
        assert!(heap.is_empty().await);
    }

    #[tokio::test]
    async fn test_heap_duplicate_block_numbers() {
        let heap = AsyncUniqueHeap::new();

        // Push a block
        assert!(heap.push(create_test_block(5)).await);
        assert_eq!(heap.len().await, 1);

        // Try to push another block with the same number
        assert!(!heap.push(create_test_block(5)).await);
        assert_eq!(heap.len().await, 1);

        // Verify the block can be popped
        let popped = heap.pop().await.unwrap();
        assert_eq!(popped.0.header.number, 5);
        assert!(heap.is_empty().await);

        // After popping, should be able to push a block with the same number again
        assert!(heap.push(create_test_block(5)).await);
        assert_eq!(heap.len().await, 1);
    }

    #[tokio::test]
    async fn test_heap_peek_lowest_block() {
        let heap = AsyncUniqueHeap::new();

        // Push blocks
        heap.push(create_test_block(20)).await;
        heap.push(create_test_block(5)).await;
        heap.push(create_test_block(15)).await;
        heap.push(create_test_block(1)).await;

        // Peek should return the block with the lowest number (highest priority)
        let peeked = heap.peek().await.unwrap();
        assert_eq!(peeked.0.header.number, 1);

        // Length should remain unchanged after peek
        assert_eq!(heap.len().await, 4);

        // Pop should return the same block
        let popped = heap.pop().await.unwrap();
        assert_eq!(popped.0.header.number, 1);
        assert_eq!(heap.len().await, 3);
    }

    #[tokio::test]
    async fn test_sequential_block_processing() {
        let heap = AsyncUniqueHeap::new();

        // Simulate blocks arriving out of order
        let block_numbers = vec![8, 3, 1, 12, 5, 2, 9, 4];
        for &num in &block_numbers {
            heap.push(create_test_block(num)).await;
        }

        // Process blocks - they should come out in sequential order
        let mut processed_numbers = Vec::new();
        while let Some(block) = heap.pop().await {
            processed_numbers.push(block.0.header.number);
        }

        // Should be in ascending order
        let mut expected = block_numbers.clone();
        expected.sort();
        assert_eq!(processed_numbers, expected);

        // Verify it's actually sequential from lowest to highest
        assert_eq!(processed_numbers, vec![1, 2, 3, 4, 5, 8, 9, 12]);
    }

    #[test]
    fn test_ordering_edge_cases() {
        // Test with block number 0
        let block0 = create_test_block(0);
        let block1 = create_test_block(1);
        assert!(block0 > block1);

        // Test with very large block numbers
        let block_max = create_test_block(u64::MAX);
        let block_large = create_test_block(u64::MAX - 1);
        assert!(block_large > block_max);
    }

    #[tokio::test]
    async fn test_concurrent_block_insertion() {
        use std::sync::Arc;

        let heap = Arc::new(AsyncUniqueHeap::new());
        let mut handles = Vec::new();

        // Spawn multiple tasks inserting blocks concurrently
        for i in 0..20 {
            let heap_clone = heap.clone();
            let handle = tokio::spawn(async move { heap_clone.push(create_test_block(i)).await });
            handles.push(handle);
        }

        // Wait for all insertions to complete
        for handle in handles {
            assert!(handle.await.unwrap());
        }

        assert_eq!(heap.len().await, 20);

        // Verify blocks come out in correct order
        for expected in 0..20 {
            let block = heap.pop().await.unwrap();
            assert_eq!(block.0.header.number, expected);
        }

        assert!(heap.is_empty().await);
    }
}
