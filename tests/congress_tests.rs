use words_to_data::congress::{CongressClient, VotePosition};

const TEST_CONGRESS_CACHE_DIR: &str = "tests/test_data/congress_client_cache";

#[test]
#[ignore] // Requires live API key - run with: cargo test -- --ignored
fn should_download_bill_data_live() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let cache_dir = dir.path().join("cache");

    let api_key =
        std::env::var("CONGRESS_API_KEY").expect("Set CONGRESS_API_KEY env var to run this test");

    let client = CongressClient::new(api_key, Some(cache_dir.to_string_lossy().to_string()));

    // Use HR 1 (house bill), not pl (public law)
    let download = client.download_bill("119-hr-1").unwrap();

    assert!(!download.bill_xml.is_empty());
    assert!(!download.bill_metadata_json.is_empty());
    assert!(!download.member_jsons.is_empty());
}

#[test]
fn should_parse_vote_position_from_string() {
    assert_eq!("Yea".parse::<VotePosition>().unwrap(), VotePosition::Yea);
    assert_eq!("yea".parse::<VotePosition>().unwrap(), VotePosition::Yea);
    assert_eq!("Nay".parse::<VotePosition>().unwrap(), VotePosition::Nay);
    assert_eq!(
        "Not Voting".parse::<VotePosition>().unwrap(),
        VotePosition::NotVoting
    );
    assert_eq!(
        "Present".parse::<VotePosition>().unwrap(),
        VotePosition::Present
    );
    assert!("Unknown".parse::<VotePosition>().is_err());
}

mod dataset_integration {
    use super::*;
    use words_to_data::dataset::{Dataset, DatasetMetadata};

    fn test_metadata() -> DatasetMetadata {
        DatasetMetadata {
            name: "Test".into(),
            description: "Test dataset".into(),
            author: "Test".into(),
            source_urls: vec![],
            license: "MIT".into(),
            version: "1.0".into(),
            ..Default::default()
        }
    }
    #[test]
    /// This test does NOT require networking or an API key, the data is cached and should pass
    fn test_download_bill_parsing() {
        // Never expire the cache: the fixtures are committed and must be read
        // regardless of their on-disk age (and never deleted by a read).
        let client = CongressClient::with_ttl(
            "".to_string(),
            Some(TEST_CONGRESS_CACHE_DIR.to_string()),
            None,
        );

        let download = client.download_bill("119-hr-1").unwrap();

        assert!(!download.bill_xml.is_empty());
        assert!(!download.bill_metadata_json.is_empty());
        assert!(!download.member_jsons.is_empty());
        let mut dataset = Dataset::new(test_metadata());
        let bill_id = dataset.load_bill_download(&download).unwrap();
        // Votes loaded
        let votes = dataset.get_bill_votes(&bill_id).unwrap().unwrap();
        assert_eq!(votes.roll_calls.len(), 1);
        assert_eq!(votes.roll_calls[0].member_votes.len(), 432);

        // Members exist
        assert!(dataset.get_member("A000375").unwrap().is_some());
        assert_eq!(
            dataset.get_member("A000375").unwrap().unwrap().last_name,
            "Arrington"
        );

        // Can look up by member
        let arrington_votes = dataset.votes_by_member("A000375").unwrap();
        assert_eq!(arrington_votes.len(), 1);

        // Sponsor info
        assert!(dataset.get_sponsor_info("119-hr-1").unwrap().is_some());
        assert_eq!(
            dataset
                .get_sponsor_info("119-hr-1")
                .unwrap()
                .unwrap()
                .sponsor,
            "A000375"
        );

        // Votes by member
        let arrington_votes = dataset.votes_by_member("A000375").unwrap();
        assert_eq!(arrington_votes.len(), 1);
        assert_eq!(arrington_votes[0].1, VotePosition::Yea);

        let pelosi_votes = dataset.votes_by_member("P000197").unwrap();
        assert_eq!(pelosi_votes.len(), 1);
        assert_eq!(pelosi_votes[0].1, VotePosition::Nay);

        // Member not in dataset
        let unknown = dataset.votes_by_member("X000000").unwrap();
        assert!(unknown.is_empty());
    }
}
