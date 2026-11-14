//! A wait-free handoff between two threads: a ring of slots allocated when
//! it is made, one producer, and one consumer. Neither side locks, waits,
//! or allocates; a full ring refuses the item and an empty one has none.
use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Ring<T> {
    slots: Box<[UnsafeCell<MaybeUninit<T>>]>,
    /// Items taken so far. Only the consumer stores it.
    head: AtomicUsize,
    /// Items added so far. Only the producer stores it.
    tail: AtomicUsize,
}

// SAFETY: a slot is written only by the producer while it is free, and read
// only by the consumer once the producer has published it, so an item moves
// from one thread to the other once.
unsafe impl<T: Send> Sync for Ring<T> {}

impl<T> Drop for Ring<T> {
    fn drop(&mut self) {
        let (head, tail) = (*self.head.get_mut(), *self.tail.get_mut());
        let len = self.slots.len();
        for index in head..tail {
            // SAFETY: the items from head to tail were written and not taken.
            unsafe { self.slots[index % len].get_mut().assume_init_drop() };
        }
    }
}

/// The side that adds items.
pub(crate) struct Producer<T> {
    ring: Arc<Ring<T>>,
}

/// The side that takes items, oldest first.
pub(crate) struct Consumer<T> {
    ring: Arc<Ring<T>>,
}

/// A handoff of `capacity` slots, at least one.
pub(crate) fn handoff<T: Send>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    let ring = Arc::new(Ring {
        slots: (0..capacity.max(1))
            .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
            .collect(),
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
    });
    (
        Producer {
            ring: Arc::clone(&ring),
        },
        Consumer { ring },
    )
}

impl<T> Producer<T> {
    /// Add `item`, or give it back when the ring is full.
    pub(crate) fn push(&mut self, item: T) -> Result<(), T> {
        let ring = &*self.ring;
        let tail = ring.tail.load(Ordering::Relaxed);
        let head = ring.head.load(Ordering::Acquire);
        if tail.wrapping_sub(head) == ring.slots.len() {
            return Err(item);
        }
        // SAFETY: the slot is free: the consumer has taken what it held, and
        // only this producer writes slots.
        unsafe { (*ring.slots[tail % ring.slots.len()].get()).write(item) };
        ring.tail.store(tail.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Whether the consumer has gone.
    pub(crate) fn abandoned(&self) -> bool {
        Arc::strong_count(&self.ring) == 1
    }
}

impl<T> Consumer<T> {
    /// Take the oldest item, if there is one.
    pub(crate) fn pop(&mut self) -> Option<T> {
        let ring = &*self.ring;
        let head = ring.head.load(Ordering::Relaxed);
        let tail = ring.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        // SAFETY: the producer published this slot, and only this consumer
        // takes items, once each.
        let item = unsafe { (*ring.slots[head % ring.slots.len()].get()).assume_init_read() };
        ring.head.store(head.wrapping_add(1), Ordering::Release);
        Some(item)
    }

    /// Whether the producer has gone. Items it added may remain.
    pub(crate) fn abandoned(&self) -> bool {
        Arc::strong_count(&self.ring) == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_arrive_in_order_and_a_full_ring_refuses_more() {
        let (mut producer, mut consumer) = handoff(3);
        for round in 0..10 {
            for item in 0..3 {
                assert!(producer.push(round * 3 + item).is_ok());
            }
            assert_eq!(producer.push(99), Err(99));
            for item in 0..3 {
                assert_eq!(consumer.pop(), Some(round * 3 + item));
            }
            assert_eq!(consumer.pop(), None);
        }
        assert!(!producer.abandoned());
        drop(consumer);
        assert!(producer.abandoned());
    }

    #[test]
    fn items_left_in_the_ring_are_dropped_with_it() {
        let item = Arc::new(());
        let (mut producer, consumer) = handoff(4);
        producer.push(Arc::clone(&item)).unwrap();
        producer.push(Arc::clone(&item)).unwrap();
        assert_eq!(Arc::strong_count(&item), 3);
        drop((producer, consumer));
        assert_eq!(Arc::strong_count(&item), 1);
    }

    #[test]
    fn items_cross_threads_once_each() {
        let (mut producer, mut consumer) = handoff::<u64>(8);
        let sender = std::thread::spawn(move || {
            for item in 0..100_000_u64 {
                let mut item = item;
                while let Err(back) = producer.push(item) {
                    item = back;
                    std::hint::spin_loop();
                }
            }
        });
        let mut expected = 0;
        while expected < 100_000 {
            if let Some(item) = consumer.pop() {
                assert_eq!(item, expected);
                expected += 1;
            }
        }
        sender.join().unwrap();
        assert_eq!(consumer.pop(), None);
    }
}
