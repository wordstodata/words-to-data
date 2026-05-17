use std::{collections::HashSet, sync::Arc};

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

    pub fn intern(&mut self, s: &str) -> Arc<str> {
        if let Some(val) = self.strings.get(s) {
            val.clone()
        } else {
            self.strings.insert(s.into());
            self.strings.get(s).unwrap().clone()
        }
    }

    pub fn intern_option(&mut self, s: &Option<Arc<str>>) -> Option<Arc<str>> {
        s.as_ref().map(|val| self.intern(val))
    }
}
