//! The head of a compact W2D file is read on its own.
//!
//! A compact file states its schema version first and its metadata second, so
//! neither needs the body. Reading them used to cost a walk of the whole
//! document: the file was read into a string, and serde tokenized all of it to
//! reach one integer. A file written by another build therefore cost as much to
//! refuse as a current one cost to read.
//!
//! The tests here state the cost as a relationship between two measurements,
//! never as a number of seconds: how much of the file a reader consumed. A
//! wall-clock figure would say the same thing and fail on a loaded machine.

use std::fs::File;
use std::io::Read;

use serde::Deserialize;
use words_to_data::compact::{
    DatasetCompact, ExpressionCompact, StringTable, check_schema_version, metadata_of,
};
use words_to_data::dataset::{
    Dataset, DatasetError, DatasetMetadata, Expression, ExpressionId, Format, WorkId, work_roots,
};
use words_to_data::storage::SCHEMA_VERSION;
use words_to_data::uslm::parser::parse;

const TITLE_9: &str = "tests/test_data/usc/2025-07-18/usc09.xml";
const RELEASE: &str = "2025-07-18";

/// A reader that reports how many bytes of the file were actually read.
struct Counted<R> {
    inner: R,
    bytes: usize,
}

impl<R: Read> Counted<R> {
    fn new(inner: R) -> Self {
        Self { inner, bytes: 0 }
    }
}

impl<R: Read> Read for Counted<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.bytes += read;
        Ok(read)
    }
}

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "Compact head".to_string(),
        description: "Title 9 at one release point".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
    }
}

/// One title of the corpus as an expression, as the writer splits it.
fn expression_of(source: &str) -> Expression {
    let parsed = parse(source, RELEASE).expect("the corpus should parse");
    let root = work_roots(parsed).pop().expect("the file holds one title");
    Expression {
        id: ExpressionId::new(WorkId::new(root.data.path.to_string()), RELEASE),
        label: None,
        root,
    }
}

/// Write a real compact file holding one title, and hand back its path.
fn written_dataset(name: &str, source: &str) -> String {
    let path = format!("{}/{name}.w2d", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);

    let mut dataset = Dataset::new(metadata());
    dataset
        .add_expression(expression_of(source))
        .expect("an expression should be added");
    dataset
        .save(&path, Format::Compact)
        .expect("the dataset should save");

    path
}

/// The fields of a compact file, as the file states them.
fn fields_of(path: &str) -> serde_json::Map<String, serde_json::Value> {
    let json = std::fs::read_to_string(path).expect("the file should read");
    serde_json::from_str(&json).expect("a compact file is a JSON object")
}

/// The same fields written out as one object, with the named ones first.
///
/// serde_json orders an object's keys by name, so an object it writes cannot
/// state a chosen order. The text is therefore built here instead.
fn written_in_order(
    fields: &serde_json::Map<String, serde_json::Value>,
    first: &[&str],
    name: &str,
) -> String {
    let rest = fields.keys().filter(|key| !first.contains(&key.as_str()));
    let members: Vec<String> = first
        .iter()
        .map(|key| key.to_string())
        .chain(rest.cloned())
        .map(|key| {
            let value = fields.get(&key).expect("the field should be in the file");
            format!("\"{key}\":{value}")
        })
        .collect();

    let path = format!("{}/{name}.w2d", env!("CARGO_TARGET_TMPDIR"));
    std::fs::write(&path, format!("{{{}}}", members.join(","))).expect("the file should write");
    path
}

/// The same dataset, marked as written by another build.
///
/// The version stays first, where the writer states it, because that is what a
/// file from an older build looks like.
fn marked_as_schema(path: &str, name: &str, version: i32) -> String {
    let mut fields = fields_of(path);
    fields.insert("schema_version".to_string(), version.into());
    written_in_order(&fields, &["schema_version"], name)
}

/// How much of the file one read consumed.
fn bytes_read<T>(path: &str, read: impl FnOnce(&mut dyn Read) -> T) -> usize {
    let mut counted = Counted::new(File::open(path).expect("the file should open"));
    read(&mut counted);
    counted.bytes
}

#[test]
fn should_read_the_schema_version_without_reading_the_whole_file() {
    let path = written_dataset("head_version", TITLE_9);

    let version_bytes = bytes_read(&path, |reader| {
        check_schema_version(reader).expect("this build wrote the file");
    });
    let dataset_bytes = bytes_read(&path, |reader| {
        let _: DatasetCompact = serde_json::from_reader(reader).expect("this build wrote the file");
    });

    assert!(
        version_bytes * 10 < dataset_bytes,
        "the version read {version_bytes} bytes and the dataset read {dataset_bytes}"
    );
}

/// The metadata is the other thing a reader asks for without the body: who
/// wrote the dataset, and what it says it covers. Only the string table sat
/// between the version and it, and the string table is most of the file.
#[test]
fn should_read_the_metadata_without_reading_the_whole_file() {
    let path = written_dataset("head_metadata", TITLE_9);

    let metadata_bytes = bytes_read(&path, |reader| {
        let metadata = metadata_of(reader).expect("the file states its metadata");
        assert_eq!(metadata.name, "Compact head");
        assert_eq!(metadata.license, "MIT");
    });
    let dataset_bytes = bytes_read(&path, |reader| {
        let _: DatasetCompact = serde_json::from_reader(reader).expect("this build wrote the file");
    });

    assert!(
        metadata_bytes * 10 < dataset_bytes,
        "the metadata read {metadata_bytes} bytes and the dataset read {dataset_bytes}"
    );
}

/// A refusal must not cost what an answer costs. A file from another build was
/// read in full before the reader could say it could not read it.
#[test]
fn should_refuse_a_file_from_another_schema_without_reading_the_whole_file() {
    let path = written_dataset("head_stale", TITLE_9);
    let stale = marked_as_schema(&path, "head_stale_rewritten", 0);
    let whole = std::fs::metadata(&stale)
        .expect("the file should be there")
        .len() as usize;

    let mut found_and_expected = None;
    let refusal_bytes = bytes_read(&stale, |reader| match check_schema_version(reader) {
        Err(DatasetError::SchemaVersionMismatch { found, expected }) => {
            found_and_expected = Some((found, expected));
        }
        Err(other) => panic!("the refusal should name the schema, got {other}"),
        Ok(()) => panic!("a file from another schema must be refused"),
    });

    let (found, expected) = found_and_expected.expect("the file should be refused");
    assert_eq!(found, 0, "the file states the schema it was marked with");
    assert_eq!(expected, SCHEMA_VERSION);
    assert!(
        refusal_bytes * 10 < whole,
        "the refusal read {refusal_bytes} bytes of a file of {whole}"
    );
}

// --- The order the fields are written in is a cost, not a format ---
//
// Moving the metadata above the string table changes what a file looks like,
// and it must not change what a file means. The two tests below take the claim
// in both directions instead of trusting it: a file in the old order read by
// this build, and a file in this build's order read by a reader that names the
// fields in the old order.

/// The reader as it was before the metadata moved.
///
/// A build from before the change cannot be compiled beside this one, so this
/// stands in for it: the same types, named in the order that build named them.
#[derive(Deserialize)]
struct ReaderOfTheOldOrder {
    schema_version: i32,
    string_table: StringTable,
    metadata: DatasetMetadata,
    expressions: Vec<ExpressionCompact>,
}

#[test]
fn should_read_a_file_whose_fields_are_in_the_old_order() {
    let path = written_dataset("head_old_order", TITLE_9);
    let old_order = written_in_order(
        &fields_of(&path),
        &["schema_version", "string_table", "metadata"],
        "head_old_order_rewritten",
    );

    let dataset = Dataset::load(&old_order, Format::Compact).expect("the file should load");

    assert_eq!(dataset.metadata().name, "Compact head");
    assert_eq!(dataset.works().expect("works should list").len(), 1);
}

#[test]
fn should_be_read_by_a_reader_that_names_the_fields_in_the_old_order() {
    let path = written_dataset("head_new_order", TITLE_9);
    let json = std::fs::read_to_string(&path).expect("the file should read");

    let old: ReaderOfTheOldOrder =
        serde_json::from_str(&json).expect("the old reader should read this file");

    assert_eq!(old.schema_version, SCHEMA_VERSION);
    assert_eq!(old.metadata.name, "Compact head");
    assert_eq!(old.expressions.len(), 1);
    assert_eq!(old.string_table.get(0), "uscode/title_9");
}
