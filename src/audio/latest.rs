//! The latest value of a setting that the window writes and the audio thread
//! reads while it plays. A reader never locks, waits, or allocates: it pins
//! the published slot, copies it, and lets it go. A writer fills a slot that
//! no reader holds and publishes it; writers take turns under a lock of
//! their own, which no reader touches.
use std::{
    cell::UnsafeCell,
    sync::{
        Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

/// One published slot and room for writes while readers hold others.
const SLOTS: usize = 4;

pub(crate) struct Latest<T> {
    slots: [UnsafeCell<T>; SLOTS],
    /// Readers that hold each slot.
    readers: [AtomicUsize; SLOTS],
    /// The published slot.
    current: AtomicUsize,
    /// Publications so far.
    generation: AtomicU64,
    writer: Mutex<()>,
}

// SAFETY: a writer writes only a slot that is not published and that no
// reader holds (see `set`), and a reader reads only the published slot while
// it holds it, so no slot is read and written at once.
unsafe impl<T: Copy + Send> Sync for Latest<T> {}

impl<T: Copy + PartialEq> Latest<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            slots: std::array::from_fn(|_| UnsafeCell::new(value)),
            readers: std::array::from_fn(|_| AtomicUsize::new(0)),
            current: AtomicUsize::new(0),
            generation: AtomicU64::new(0),
            writer: Mutex::new(()),
        }
    }

    /// Publish `value` unless it is the latest already. Only writers wait,
    /// for each other and for a free slot.
    pub(crate) fn set(&self, value: T) {
        let _turn = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        let current = self.current.load(Ordering::SeqCst);
        // SAFETY: only writers write slots, and this one has the turn.
        if unsafe { *self.slots[current].get() } == value {
            return;
        }
        loop {
            // A reader that pinned a slot before it was replaced still
            // counts here, so its slot is not overwritten under it.
            if let Some(free) = (0..SLOTS)
                .find(|&slot| slot != current && self.readers[slot].load(Ordering::SeqCst) == 0)
            {
                // SAFETY: the slot is not published, and no reader holds it:
                // one that pins it now sees that it is not published and lets
                // it go without reading it.
                unsafe { *self.slots[free].get() = value };
                self.current.store(free, Ordering::SeqCst);
                self.generation.fetch_add(1, Ordering::Release);
                return;
            }
            std::thread::yield_now();
        }
    }

    /// The latest value, unless a write overlaps two attempts to read it.
    pub(crate) fn try_get(&self) -> Option<T> {
        for _ in 0..2 {
            let slot = self.current.load(Ordering::SeqCst);
            self.readers[slot].fetch_add(1, Ordering::SeqCst);
            let value = (self.current.load(Ordering::SeqCst) == slot)
                // SAFETY: the slot is published and pinned, so no writer
                // writes it until it is let go.
                .then(|| unsafe { *self.slots[slot].get() });
            self.readers[slot].fetch_sub(1, Ordering::Release);
            if value.is_some() {
                return value;
            }
        }
        None
    }

    /// The latest value. For threads that may wait for a writer.
    pub(crate) fn get(&self) -> T {
        loop {
            if let Some(value) = self.try_get() {
                return value;
            }
            std::thread::yield_now();
        }
    }

    /// The value published since `seen`, if any, and if a write does not
    /// overlap the read. A failed read leaves `seen` for the next try.
    pub(crate) fn newer(&self, seen: &mut u64) -> Option<T> {
        let generation = self.generation.load(Ordering::Acquire);
        if generation == *seen {
            return None;
        }
        let value = self.try_get()?;
        *seen = generation;
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::AtomicBool};

    #[test]
    fn a_reader_gets_the_latest_value_once_per_publication() {
        let latest = Latest::new(1_u32);
        let mut seen = 0;
        assert_eq!(latest.newer(&mut seen), None);
        latest.set(2);
        assert_eq!(latest.newer(&mut seen), Some(2));
        assert_eq!(latest.newer(&mut seen), None);
        latest.set(2);
        assert_eq!(latest.newer(&mut seen), None);
        latest.set(3);
        latest.set(4);
        assert_eq!(latest.newer(&mut seen), Some(4));
        assert_eq!(latest.get(), 4);
    }

    #[test]
    fn readers_never_see_a_torn_value() {
        // Each value repeats one number, so a read that a write overlapped
        // would mix two.
        let latest = Arc::new(Latest::new([0_u64; 32]));
        let done = Arc::new(AtomicBool::new(false));
        let reads: Arc<[AtomicU64; 3]> = Arc::new(std::array::from_fn(|_| AtomicU64::new(0)));
        let readers: Vec<_> = (0..3)
            .map(|reader| {
                let (latest, done, reads) =
                    (Arc::clone(&latest), Arc::clone(&done), Arc::clone(&reads));
                std::thread::spawn(move || {
                    while !done.load(Ordering::Relaxed) {
                        if let Some(value) = latest.try_get() {
                            assert!(value.iter().all(|word| *word == value[0]));
                            reads[reader].fetch_add(1, Ordering::Relaxed);
                        }
                    }
                })
            })
            .collect();
        // Write until every reader has read many times while writes ran,
        // however late the system starts it.
        let mut value = 0_u64;
        while value < 20_000
            || reads
                .iter()
                .any(|count| count.load(Ordering::Relaxed) < 1_000)
        {
            value += 1;
            latest.set([value; 32]);
            assert!(value < 100_000_000, "a reader starves");
        }
        done.store(true, Ordering::Relaxed);
        for reader in readers {
            reader.join().unwrap();
        }
        assert_eq!(latest.get(), [value; 32]);
    }
}
