//! A bill the dataset does not hold is refused by name (#337).
//!
//! `votes 119-hr-1071` answered "No votes for bill" on a dataset that holds
//! `119-s-1071` and not `119-hr-1071`: the same words as for a held bill with no
//! House roll call. `residue --bill 119-hr-1071` answered "0 amendment(s) that
//! no standing link names" and exit 0, which reads as "the bill is fully
//! linked". An agent then gave roll calls for a bill the dataset does not hold.
//!
//! Every command that takes one bill asks the same question the same way, and
//! names the held bills that share the bill's number, because a wrong chamber
//! is the usual typing error.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::congress::CongressClient;
use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::uslm::bill_parser::parse_bill_amendments;

const CONGRESS_CACHE: &str = "tests/test_data/congress_client_cache";
const HR_1_LAW: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

/// The bill the fixtures hold.
const HELD: &str = "119-hr-1";
/// A bill with the held bill's number, in the other chamber.
const OTHER_CHAMBER: &str = "119-s-1";
/// A bill that shares nothing with the held bill but its congress.
const UNLIKE: &str = "119-hr-9999";

/// HR 1 with its roll call and every member who voted, from the cached
/// responses. No network and no API key: the fixtures are committed.
fn with_votes() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!("{}/bill_not_held_votes.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        let client =
            CongressClient::with_ttl("".to_string(), Some(CONGRESS_CACHE.to_string()), None);
        let download = client
            .download_bill(HELD)
            .expect("the cached bill should load");
        let mut dataset = Dataset::new(DatasetMetadata::default());
        dataset
            .load_bill_download(&download)
            .expect("the download should load");
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

/// HR 1 as its public law alone: a held bill with no roll call.
fn without_votes() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!(
            "{}/bill_not_held_no_votes.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);
        let mut dataset = Dataset::new(DatasetMetadata::default());
        let bill = parse_bill_amendments(HELD, HR_1_LAW).expect("the bill should parse");
        dataset.add_bill(bill).expect("the bill should add");
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

/// Run the CLI as a subprocess, the way an agent or a shell would.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// Each command that takes one bill, asked about `bill`.
fn each_command(dataset: &'static str, bill: &'static str) -> Vec<Vec<&'static str>> {
    vec![
        vec!["votes", dataset, bill],
        vec!["votes", dataset, bill, "--json"],
        vec!["show-bill", dataset, bill],
        vec!["residue", dataset, "--bill", bill],
        vec!["residue", dataset, "--bill", bill, "--json"],
        vec!["redesignation-report", dataset, "--bill-id", bill],
    ]
}

#[test]
fn should_refuse_and_name_the_held_bill_with_its_number_when_a_command_is_given_a_bill_not_held() {
    for args in each_command(with_votes(), OTHER_CHAMBER) {
        let output = run(&args);

        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("This dataset holds no bill {OTHER_CHAMBER}.")),
            "{args:?}: {stderr}"
        );
        assert!(
            stderr.contains(&format!("Held bills with the number 1: {HELD}")),
            "{args:?}: {stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "{args:?} should print no answer: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

/// With no bill alike to name, the notice says where the list of held bills is.
#[test]
fn should_refuse_and_point_to_the_bill_list_when_no_held_bill_shares_the_number() {
    for args in each_command(with_votes(), UNLIKE) {
        let output = run(&args);

        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("This dataset holds no bill {UNLIKE}.")),
            "{args:?}: {stderr}"
        );
        assert!(
            !stderr.contains("Held bills with the number"),
            "{args:?}: {stderr}"
        );
        assert!(
            stderr.contains(&format!(
                "`words_to_data bills {}` lists the 1 bill(s) it holds.",
                with_votes()
            )),
            "{args:?}: {stderr}"
        );
    }
}

/// A held bill with no roll call is a different fact from a bill not held, so
/// `votes` says it in different words.
#[test]
fn should_say_the_bill_is_held_when_votes_is_given_a_held_bill_with_no_roll_call() {
    let output = run(&["votes", without_votes(), HELD]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!(
            "This dataset holds bill {HELD}, and no House roll call for it."
        )),
        "{stderr}"
    );
    assert!(!stderr.contains("holds no bill"), "{stderr}");
}

/// A held bill still answers as it did.
#[test]
fn should_answer_as_before_when_a_command_is_given_a_held_bill() {
    for args in each_command(with_votes(), HELD) {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
