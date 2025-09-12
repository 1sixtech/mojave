use crate::unique_heap::UniqueHeapItem;
use ethrex_common::types::Block;

#[derive(Debug, Clone, Eq)]
pub struct OrderedBlock(pub Block);

impl PartialEq for OrderedBlock {
    fn eq(&self, other: &Self) -> bool {
        self.0.header.number == other.0.header.number
    }
}

impl PartialOrd for OrderedBlock {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedBlock {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse ordering so that lower block numbers have higher priority
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
    use super::OrderedBlock;
    use ethrex_common::types::{Block, BlockBody, BlockHeader};

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

        assert!(block1 > block2);
        assert!(block2 > block3);
        assert!(block1 > block10);
        assert!(block3 > block10);
    }
}
