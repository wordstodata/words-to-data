//! A member's party is dated, so a vote is read with the party of its own day.
//!
//! Every member here comes from a cached Congress API response committed under
//! `tests/test_data/congress_client_cache/member`. None of it is written by
//! hand: the bug this guards against was only visible in the real responses.

use words_to_data::congress::{Member, Party, PartyOnDate};
use words_to_data::date::date_str_to_date;

const MEMBER_CACHE: &str = "tests/test_data/congress_client_cache/member";

/// One member, parsed from the response the Congress API really returned.
fn member(bioguide_id: &str) -> Member {
    let path = format!("{MEMBER_CACHE}/{bioguide_id}.json");
    let json = std::fs::read_to_string(&path).expect("the cached response should read");
    Member::from_api_response(&json).expect("the cached response should parse")
}

/// Kevin Kiley voted Yea on HR 1 on 2025-07-03 as a Republican, and became an
/// Independent in 2026. Reading his vote through an undated party made the
/// dataset report an Independent Yea (#105).
#[test]
fn should_report_republican_when_kiley_is_asked_about_the_day_he_voted_on_hr_1() {
    let kiley = member("K000401");

    let party = kiley.party_on(date_str_to_date("2025-07-03").unwrap());

    assert_eq!(party, PartyOnDate::Resolved(Party::Republican));
}

/// Jefferson Van Drew changed party from Democratic to Republican across
/// 2019-2020. He read correctly before this work by luck, because his change
/// came before every vote in the corpus. He must read correctly on purpose.
#[test]
fn should_report_van_drew_on_both_sides_of_his_party_change() {
    let van_drew = member("V000133");

    assert_eq!(
        van_drew.party_on(date_str_to_date("2019-06-01").unwrap()),
        PartyOnDate::Resolved(Party::Democrat),
        "he sat as a Democrat through 2019"
    );
    assert_eq!(
        van_drew.party_on(date_str_to_date("2021-06-01").unwrap()),
        PartyOnDate::Resolved(Party::Republican),
        "he sat as a Republican from 2021 on"
    );
}

/// The history carries years, so the year of a change belongs to both parties.
/// Kiley's Republican entry ends in 2026 and his Independent entry starts in
/// 2026: no day in 2026 can be given to one of them from this data.
#[test]
fn should_report_an_unresolved_party_when_the_date_falls_in_a_year_two_parties_share() {
    let kiley = member("K000401");

    let party = kiley.party_on(date_str_to_date("2026-03-01").unwrap());

    assert_eq!(
        party,
        PartyOnDate::Ambiguous(vec![Party::Republican, Party::Independent])
    );
    assert_eq!(
        party.resolved(),
        None,
        "an ambiguous party is not an answer"
    );
}

/// A reader that only sees the output must be able to tell the two apart. The
/// text says `unresolved`, and the JSON tags the two cases differently.
#[test]
fn should_distinguish_an_unresolved_party_from_a_resolved_one_in_json_and_in_text() {
    let kiley = member("K000401");

    let voting_day = kiley.party_on(date_str_to_date("2025-07-03").unwrap());
    let change_year = kiley.party_on(date_str_to_date("2026-03-01").unwrap());
    let before_he_arrived = kiley.party_on(date_str_to_date("2001-03-01").unwrap());

    assert_eq!(
        serde_json::to_string(&voting_day).unwrap(),
        r#"{"Resolved":"Republican"}"#
    );
    assert_eq!(
        serde_json::to_string(&change_year).unwrap(),
        r#"{"Ambiguous":["Republican","Independent"]}"#
    );
    assert_eq!(
        serde_json::to_string(&before_he_arrived).unwrap(),
        r#""Unknown""#
    );

    assert_eq!(voting_day.to_string(), "Republican");
    assert_eq!(
        change_year.to_string(),
        "unresolved (Republican or Independent)"
    );
    assert_eq!(before_he_arrived.to_string(), "unknown");
}

/// The API lists the history newest first and promises nothing. Feeding the
/// real response back with its entries reversed must give the same answer.
#[test]
fn should_give_the_same_answer_when_the_party_history_arrives_in_reverse_order() {
    let path = format!("{MEMBER_CACHE}/K000401.json");
    let json = std::fs::read_to_string(&path).expect("the cached response should read");
    let mut response: serde_json::Value =
        serde_json::from_str(&json).expect("the cached response should be JSON");

    let history = response["member"]["partyHistory"]
        .as_array_mut()
        .expect("the response should carry a party history");
    history.reverse();

    let reversed = Member::from_api_response(&response.to_string())
        .expect("the reordered response should parse");

    assert_eq!(
        reversed.party_on(date_str_to_date("2025-07-03").unwrap()),
        PartyOnDate::Resolved(Party::Republican)
    );
    assert_eq!(
        reversed.party_history,
        member("K000401").party_history,
        "the stored history is sorted, not left in the order it arrived"
    );
}

/// The tally the bug was visible in: HR 1's passage on 2025-07-03.
mod hr_1 {
    use super::*;
    use words_to_data::congress::{CongressClient, VotePosition};
    use words_to_data::dataset::{Dataset, DatasetMetadata};
    use words_to_data::inspect::{self, RollCallTally};
    use words_to_data::storage::InMemoryStorage;

    const CONGRESS_CACHE: &str = "tests/test_data/congress_client_cache";

    /// HR 1 with its roll call and every member who voted, out of the cached
    /// responses. No network and no API key: the fixtures are committed.
    fn dataset_holding_hr_1() -> Dataset<InMemoryStorage> {
        let client =
            CongressClient::with_ttl("".to_string(), Some(CONGRESS_CACHE.to_string()), None);
        let download = client
            .download_bill("119-hr-1")
            .expect("the cached bill should load");

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "HR 1".into(),
            description: "HR 1 with its roll call".into(),
            author: "words_to_data tests".into(),
            source_urls: vec![],
            license: "MIT".into(),
            version: "1.0".into(),
            ..Default::default()
        });
        dataset
            .load_bill_download(&download)
            .expect("the download should load");
        dataset
    }

    fn count(tally: &RollCallTally, party: Party, position: VotePosition) -> usize {
        tally
            .by_party
            .iter()
            .find(|row| row.party == party && row.position == position)
            .map(|row| row.count)
            .unwrap_or(0)
    }

    /// The Clerk, and the API's own party totals, report 218 Republican Yea.
    /// The dataset reported 217 Republican and 1 Independent, because Kiley's
    /// 2026 party was read onto his 2025 vote (#105).
    #[test]
    fn should_count_218_republican_yea_when_the_party_is_taken_from_the_day_of_the_vote() {
        let dataset = dataset_holding_hr_1();

        let tallies = inspect::votes(&dataset, "119-hr-1")
            .expect("the bill's votes should read")
            .expect("the dataset should hold HR 1");

        assert_eq!(tallies.len(), 1, "HR 1 carries one House roll call");
        let tally = &tallies[0];
        assert_eq!(tally.roll_number, 190);
        assert_eq!(count(tally, Party::Republican, VotePosition::Yea), 218);
        assert_eq!(count(tally, Party::Independent, VotePosition::Yea), 0);
        assert_eq!(count(tally, Party::Democrat, VotePosition::Nay), 212);
        assert_eq!(count(tally, Party::Republican, VotePosition::Nay), 2);
        assert!(
            tally.unresolved.is_empty(),
            "every party on this date resolves: {:?}",
            tally.unresolved
        );
    }

    /// The same answer through the surface a person or an agent reads.
    #[test]
    fn should_report_the_party_of_each_vote_when_the_votes_command_runs() {
        let path = format!("{}/votes_hr_1.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        dataset_holding_hr_1()
            .save_to_sqlite(&path)
            .expect("the fixture should save");

        let output = std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
            .args(["votes", &path, "119-hr-1", "--json"])
            .output()
            .expect("the binary should run");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let reported: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("--json should emit JSON");
        let republican_yea = reported[0]["by_party"]
            .as_array()
            .expect("the tally should carry party rows")
            .iter()
            .find(|row| row["party"] == "Republican" && row["position"] == "Yea")
            .expect("Republicans voted Yea on HR 1");

        assert_eq!(republican_yea["count"], 218);
        assert_eq!(
            reported[0]["unresolved"].as_array().map(Vec::len),
            Some(0),
            "no party on this date is unresolved"
        );
    }
}
