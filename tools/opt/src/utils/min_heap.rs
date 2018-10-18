use std::cmp::Ordering;
use std::collections::BinaryHeap;

struct Item<P, T> {
    priority: P,
    value: T,
}

impl<P: Ord, T> PartialEq for Item<P, T> {
    fn eq(&self, other: &Self) -> bool {
        other.priority == self.priority
    }
}

impl<P: Ord, T> Eq for Item<P, T> {}

impl<P: Ord, T> PartialOrd for Item<P, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.priority.partial_cmp(&self.priority)
    }
}

impl<P: Ord, T> Ord for Item<P, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.priority.cmp(&self.priority)
    }
}

pub struct MinHeap<P, T> {
    items: BinaryHeap<Item<P, T>>,
}

impl<P, T> MinHeap<P, T>
where
    P: Ord,
{
    pub fn new() -> Self {
        MinHeap {
            items: BinaryHeap::new(),
        }
    }

    pub fn singleton(priority: P, value: T) -> Self {
        let mut heap = Self::new();
        heap.push(priority, value);
        heap
    }

    pub fn push(&mut self, priority: P, value: T) {
        self.items.push(Item { priority, value })
    }

    pub fn pop(&mut self) -> Option<(P, T)> {
        self.items.pop().map(|item| (item.priority, item.value))
    }
}
