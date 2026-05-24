use std::{collections::HashSet, sync::Arc};

// static INTERN_HITS: AtomicUsize = AtomicUsize::new(0);
// static INTERN_MISSES: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Default)]
pub struct StringInterner {
    /// Note: this needs to be Arc instead of Rc for python bindings to work
    strings: HashSet<Arc<str>>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self {
            strings: HashSet::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    pub fn intern(&mut self, s: &str) -> Arc<str> {
        if let Some(val) = self.strings.get(s) {
            // INTERN_HITS.fetch_add(1, Ordering::Relaxed);
            val.clone()
        } else {
            // INTERN_MISSES.fetch_add(1, Ordering::Relaxed);
            self.strings.insert(s.into());
            self.strings.get(s).unwrap().clone()
        }
    }

    pub fn intern_option(&mut self, s: &Option<Arc<str>>) -> Option<Arc<str>> {
        s.as_ref().map(|val| self.intern(val))
    }

    pub fn stats(&self) -> (usize, usize) {
        let count = self.strings.len();
        let bytes: usize = self.strings.iter().map(|s| s.len()).sum();
        (count, bytes)
    }

    // pub fn hit_stats() -> (usize, usize) {
    //     (
    //         INTERN_HITS.load(Ordering::Relaxed),
    //         INTERN_MISSES.load(Ordering::Relaxed),
    //     )
    // }

    // pub fn reset_hit_stats() {
    //     INTERN_HITS.store(0, Ordering::Relaxed);
    //     INTERN_MISSES.store(0, Ordering::Relaxed);
    // }
}
