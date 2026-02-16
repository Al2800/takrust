use std::collections::VecDeque;

use thiserror::Error;

use crate::{SendQueueConfig, SendQueueMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueuePriority {
    High,
    Normal,
    Low,
}

pub trait SendQueueClassifier<T> {
    fn byte_size(&self, item: &T) -> usize;

    fn priority(&self, _item: &T) -> QueuePriority {
        QueuePriority::Normal
    }

    fn coalesce_key(&self, _item: &T) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QueueEnqueueReport {
    pub replaced_existing: bool,
    pub dropped_messages: usize,
    pub dropped_bytes: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SendQueueError {
    #[error("send queue max_messages must be > 0")]
    ZeroMaxMessages,
    #[error("send queue max_bytes must be > 0")]
    ZeroMaxBytes,
}

pub struct OutboundSendQueue<T, C> {
    config: SendQueueConfig,
    classifier: C,
    current_bytes: usize,
    storage: QueueStorage<T>,
}

impl<T, C> OutboundSendQueue<T, C>
where
    C: SendQueueClassifier<T>,
{
    pub fn new(config: SendQueueConfig, classifier: C) -> Result<Self, SendQueueError> {
        if config.max_messages == 0 {
            return Err(SendQueueError::ZeroMaxMessages);
        }
        if config.max_bytes == 0 {
            return Err(SendQueueError::ZeroMaxBytes);
        }

        let storage = match config.mode {
            SendQueueMode::Fifo => QueueStorage::Fifo(VecDeque::new()),
            SendQueueMode::Priority => QueueStorage::Priority(PriorityBuckets::default()),
            SendQueueMode::CoalesceLatestByUid => QueueStorage::Coalesce(VecDeque::new()),
        };

        Ok(Self {
            config,
            classifier,
            current_bytes: 0,
            storage,
        })
    }

    #[must_use]
    pub fn mode(&self) -> SendQueueMode {
        self.config.mode.clone()
    }

    #[must_use]
    pub fn len_messages(&self) -> usize {
        match &self.storage {
            QueueStorage::Fifo(queue) => queue.len(),
            QueueStorage::Priority(buckets) => buckets.len(),
            QueueStorage::Coalesce(entries) => entries.len(),
        }
    }

    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.current_bytes
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len_messages() == 0
    }

    pub fn enqueue(&mut self, item: T) -> QueueEnqueueReport {
        let mut report = QueueEnqueueReport::default();
        let item_size = self.classifier.byte_size(&item);

        match &mut self.storage {
            QueueStorage::Fifo(queue) => {
                queue.push_back(item);
                self.current_bytes += item_size;
            }
            QueueStorage::Priority(buckets) => {
                let bucket = match self.classifier.priority(&item) {
                    QueuePriority::High => &mut buckets.high,
                    QueuePriority::Normal => &mut buckets.normal,
                    QueuePriority::Low => &mut buckets.low,
                };
                bucket.push_back(item);
                self.current_bytes += item_size;
            }
            QueueStorage::Coalesce(entries) => {
                if let Some(key) = self.classifier.coalesce_key(&item) {
                    if let Some(existing) = entries
                        .iter_mut()
                        .find(|entry| entry.key.as_deref() == Some(key.as_str()))
                    {
                        let replaced_size = self.classifier.byte_size(&existing.item);
                        self.current_bytes = self.current_bytes.saturating_sub(replaced_size);
                        existing.item = item;
                        self.current_bytes += item_size;
                        report.replaced_existing = true;
                    } else {
                        entries.push_back(CoalescedEntry {
                            key: Some(key),
                            item,
                        });
                        self.current_bytes += item_size;
                    }
                } else {
                    entries.push_back(CoalescedEntry { key: None, item });
                    self.current_bytes += item_size;
                }
            }
        }

        while self.len_messages() > self.config.max_messages
            || self.current_bytes > self.config.max_bytes
        {
            let dropped_size = match self.drop_for_pressure() {
                Some(size) => size,
                None => break,
            };
            report.dropped_messages += 1;
            report.dropped_bytes += dropped_size;
        }

        report
    }

    pub fn dequeue(&mut self) -> Option<T> {
        let maybe_item = match &mut self.storage {
            QueueStorage::Fifo(queue) => queue.pop_front(),
            QueueStorage::Priority(buckets) => buckets.pop_front(),
            QueueStorage::Coalesce(entries) => entries.pop_front().map(|entry| entry.item),
        };

        if let Some(item) = maybe_item {
            let bytes = self.classifier.byte_size(&item);
            self.current_bytes = self.current_bytes.saturating_sub(bytes);
            Some(item)
        } else {
            None
        }
    }

    fn drop_for_pressure(&mut self) -> Option<usize> {
        let maybe_item = match &mut self.storage {
            QueueStorage::Fifo(queue) => queue.pop_front(),
            QueueStorage::Priority(buckets) => buckets.pop_for_pressure(),
            QueueStorage::Coalesce(entries) => entries.pop_front().map(|entry| entry.item),
        };

        maybe_item.map(|item| {
            let bytes = self.classifier.byte_size(&item);
            self.current_bytes = self.current_bytes.saturating_sub(bytes);
            bytes
        })
    }
}

enum QueueStorage<T> {
    Fifo(VecDeque<T>),
    Priority(PriorityBuckets<T>),
    Coalesce(VecDeque<CoalescedEntry<T>>),
}

struct CoalescedEntry<T> {
    key: Option<String>,
    item: T,
}

struct PriorityBuckets<T> {
    high: VecDeque<T>,
    normal: VecDeque<T>,
    low: VecDeque<T>,
}

impl<T> PriorityBuckets<T> {
    fn len(&self) -> usize {
        self.high.len() + self.normal.len() + self.low.len()
    }

    fn pop_front(&mut self) -> Option<T> {
        self.high
            .pop_front()
            .or_else(|| self.normal.pop_front())
            .or_else(|| self.low.pop_front())
    }

    fn pop_for_pressure(&mut self) -> Option<T> {
        self.low
            .pop_front()
            .or_else(|| self.normal.pop_front())
            .or_else(|| self.high.pop_front())
    }
}

impl<T> Default for PriorityBuckets<T> {
    fn default() -> Self {
        Self {
            high: VecDeque::new(),
            normal: VecDeque::new(),
            low: VecDeque::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        OutboundSendQueue, QueuePriority, SendQueueClassifier, SendQueueConfig, SendQueueMode,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestItem {
        id: &'static str,
        bytes: usize,
        priority: QueuePriority,
        coalesce_key: Option<&'static str>,
    }

    #[derive(Debug, Clone, Copy, Default)]
    struct TestClassifier;

    impl SendQueueClassifier<TestItem> for TestClassifier {
        fn byte_size(&self, item: &TestItem) -> usize {
            item.bytes
        }

        fn priority(&self, item: &TestItem) -> QueuePriority {
            item.priority
        }

        fn coalesce_key(&self, item: &TestItem) -> Option<String> {
            item.coalesce_key.map(str::to_string)
        }
    }

    fn config(max_messages: usize, max_bytes: usize, mode: SendQueueMode) -> SendQueueConfig {
        SendQueueConfig {
            max_messages,
            max_bytes,
            mode,
        }
    }

    fn test_item(
        id: &'static str,
        bytes: usize,
        priority: QueuePriority,
        coalesce_key: Option<&'static str>,
    ) -> TestItem {
        TestItem {
            id,
            bytes,
            priority,
            coalesce_key,
        }
    }

    #[test]
    fn fifo_mode_drops_oldest_message_when_capacity_exceeded() {
        let mut queue = OutboundSendQueue::new(config(2, 128, SendQueueMode::Fifo), TestClassifier)
            .expect("config should be valid");

        queue.enqueue(test_item("a", 10, QueuePriority::Normal, None));
        queue.enqueue(test_item("b", 10, QueuePriority::Normal, None));
        let report = queue.enqueue(test_item("c", 10, QueuePriority::Normal, None));

        assert_eq!(report.dropped_messages, 1);
        assert_eq!(report.dropped_bytes, 10);
        assert_eq!(queue.len_messages(), 2);

        let first = queue.dequeue().expect("first item");
        let second = queue.dequeue().expect("second item");
        assert_eq!(first.id, "b");
        assert_eq!(second.id, "c");
        assert!(queue.is_empty());
    }

    #[test]
    fn priority_mode_prefers_high_priority_and_drops_lowest_first() {
        let mut queue =
            OutboundSendQueue::new(config(2, 128, SendQueueMode::Priority), TestClassifier)
                .expect("config should be valid");

        queue.enqueue(test_item("low-1", 10, QueuePriority::Low, None));
        queue.enqueue(test_item("high-1", 10, QueuePriority::High, None));
        let report = queue.enqueue(test_item("low-2", 10, QueuePriority::Low, None));

        assert_eq!(report.dropped_messages, 1);
        assert_eq!(report.dropped_bytes, 10);
        assert_eq!(queue.len_messages(), 2);

        let first = queue.dequeue().expect("first item");
        let second = queue.dequeue().expect("second item");
        assert_eq!(first.id, "high-1");
        assert_eq!(second.id, "low-2");
    }

    #[test]
    fn coalesce_mode_replaces_existing_uid_entry() {
        let mut queue = OutboundSendQueue::new(
            config(4, 256, SendQueueMode::CoalesceLatestByUid),
            TestClassifier,
        )
        .expect("config should be valid");

        queue.enqueue(test_item("first", 8, QueuePriority::Normal, Some("uid-a")));
        let report = queue.enqueue(test_item(
            "latest",
            12,
            QueuePriority::Normal,
            Some("uid-a"),
        ));

        assert!(report.replaced_existing);
        assert_eq!(report.dropped_messages, 0);
        assert_eq!(queue.len_messages(), 1);
        assert_eq!(queue.len_bytes(), 12);

        let item = queue.dequeue().expect("coalesced entry");
        assert_eq!(item.id, "latest");
    }

    #[test]
    fn byte_limit_pressure_drops_oldest_entries() {
        let mut queue = OutboundSendQueue::new(config(8, 10, SendQueueMode::Fifo), TestClassifier)
            .expect("config should be valid");

        queue.enqueue(test_item("a", 6, QueuePriority::Normal, None));
        let report = queue.enqueue(test_item("b", 6, QueuePriority::Normal, None));

        assert_eq!(report.dropped_messages, 1);
        assert_eq!(report.dropped_bytes, 6);
        assert_eq!(queue.len_messages(), 1);
        assert_eq!(queue.len_bytes(), 6);

        let item = queue.dequeue().expect("remaining item");
        assert_eq!(item.id, "b");
    }
}
