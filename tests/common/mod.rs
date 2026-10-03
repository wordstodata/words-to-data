//! Helpers that more than one test binary uses.
//!
//! A test file brings this in with `mod common;`. Each test binary compiles
//! its own copy and uses only some of it, so the unused parts are allowed.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use words_to_data::dataset::{Dataset, Expression, ExpressionId, WorkId, work_roots};
use words_to_data::document::DocumentNode;
use words_to_data::storage::Storage;
use words_to_data::uslm::parser::parse;

/// A USLM file as `parse(file, date)` reads it, parsed once per test binary.
///
/// Each caller gets its own clone, so a test can change its copy and no other
/// test sees the change. The parse is the slow part: title 26 is 55 MB and
/// takes seconds in a debug build. A clone takes a fraction of that.
pub fn parsed(file: &str, date: &str) -> DocumentNode {
    type Slot = Arc<OnceLock<DocumentNode>>;
    static PARSED: OnceLock<Mutex<HashMap<(String, String), Slot>>> = OnceLock::new();

    // The lock is held only to find the file's slot, so two tests that need
    // two different files parse them at the same time.
    let slot = PARSED
        .get_or_init(Default::default)
        .lock()
        .expect("no test panics while it holds the lock")
        .entry((file.to_string(), date.to_string()))
        .or_default()
        .clone();
    slot.get_or_init(|| parse(file, date).unwrap_or_else(|e| panic!("{file} should parse: {e}")))
        .clone()
}

/// Add each work of a USLM file to the dataset, as
/// `Dataset::add_uslm_xml(file, date, None)` does, from the parse that
/// [`parsed`] keeps.
pub fn add_uslm_xml<S: Storage>(dataset: &mut Dataset<S>, file: &str, date: &str) {
    for root in work_roots(parsed(file, date)) {
        dataset
            .add_expression(Expression {
                id: ExpressionId::new(WorkId::new(root.data.path.to_string()), date),
                label: None,
                root,
            })
            .unwrap_or_else(|e| panic!("{file} should load: {e}"));
    }
}
