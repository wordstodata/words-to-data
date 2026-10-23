//! A dataset path that names no file is refused, and no file is made (#337).
//!
//! `words_to_data info nope.sqlite` opened SQLite on the path, which made a new,
//! empty database there, and then reported "Works: 0". A typing error in the
//! dataset path gave an empty answer, not an error, and left a file behind. An
//! agent read the empty answer as a fact about the law.
//!
//! Only a command that builds a dataset makes a file. Every other command opens
//! a dataset that is already there.

use std::path::Path;
use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetError};

/// A path in this test binary's own folder that names no file.
fn missing(name: &str) -> String {
    let path = format!("{}/missing_dataset_{name}", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    path
}

/// Run the CLI as a subprocess, the way an agent or a shell would.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

#[test]
fn should_refuse_and_make_no_file_when_a_read_command_is_given_a_path_that_names_no_file() {
    for (command, extension) in [("info", "sqlite"), ("bills", "db"), ("info", "json")] {
        let path = missing(&format!("{command}.{extension}"));

        let output = run(&[command, &path]);

        assert_eq!(
            output.status.code(),
            Some(1),
            "{command} {path} should fail; stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("No dataset file at {path}")),
            "{command} {path}: {stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "{command} {path} should print no answer: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            !Path::new(&path).exists(),
            "{command} {path} should make no file"
        );
    }
}

/// The library door the commands use. A grow command such as `add-bills` opens
/// its dataset here too, so it cannot grow a dataset that is not there.
#[test]
fn should_refuse_and_make_no_file_when_a_dataset_is_opened_at_a_path_that_names_no_file() {
    let path = missing("library.sqlite");

    let opened = Dataset::open_sqlite(&path);

    assert!(
        matches!(&opened, Err(DatasetError::NotFound { path: named }) if named == &path),
        "{:?}",
        opened.err()
    );
    assert!(!Path::new(&path).exists(), "no file should be made");
}
