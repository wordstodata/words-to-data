use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use words_to_data::congress::{
    Chamber, CongressClient, CongressError, CosponsorRecord, Member, Party, ResponseCache,
    RollCall, SponsorInfo, VotePosition, VoteResult,
};

#[test]
fn should_parse_party_from_string() {
    assert_eq!("D".parse::<Party>().unwrap(), Party::Democrat);
    assert_eq!("R".parse::<Party>().unwrap(), Party::Republican);
    assert_eq!("I".parse::<Party>().unwrap(), Party::Independent);
    assert_eq!("ID".parse::<Party>().unwrap(), Party::Independent);
    assert_eq!("X".parse::<Party>().unwrap(), Party::Other("X".to_string()));
}

#[test]
fn should_parse_chamber_from_string() {
    assert_eq!("Senate".parse::<Chamber>().unwrap(), Chamber::Senate);
    assert_eq!("House".parse::<Chamber>().unwrap(), Chamber::House);
    assert_eq!(
        "House of Representatives".parse::<Chamber>().unwrap(),
        Chamber::House
    );
    assert!("Unknown".parse::<Chamber>().is_err());
}

#[test]
fn should_display_congress_error() {
    let err = CongressError::NotFound("member A000360".to_string());
    assert!(err.to_string().contains("A000360"));
}

#[test]
fn should_parse_vote_position_from_string() {
    assert_eq!("Yea".parse::<VotePosition>().unwrap(), VotePosition::Yea);
    assert_eq!("Nay".parse::<VotePosition>().unwrap(), VotePosition::Nay);
    assert_eq!(
        "Present".parse::<VotePosition>().unwrap(),
        VotePosition::Present
    );
    assert_eq!(
        "Not Voting".parse::<VotePosition>().unwrap(),
        VotePosition::NotVoting
    );
}

#[test]
fn should_create_roll_call() {
    let mut votes = HashMap::new();
    votes.insert("A000360".to_string(), VotePosition::Yea);
    votes.insert("B000575".to_string(), VotePosition::Nay);

    let roll = RollCall {
        congress: 118,
        session: 1,
        roll_number: 123,
        chamber: Chamber::Senate,
        date: "2023-03-15".to_string(),
        bill_id: Some("hr-1234".to_string()),
        result: VoteResult::Passed,
        votes,
    };

    assert_eq!(roll.votes.get("A000360"), Some(&VotePosition::Yea));
}

#[test]
fn should_create_sponsor_info() {
    let sponsor_info = SponsorInfo {
        bill_id: "119-pl-21".to_string(),
        sponsor: "A000360".to_string(),
        cosponsors: vec![CosponsorRecord {
            bioguide_id: "B000575".to_string(),
            date: "2023-01-15".to_string(),
            withdrawn: false,
        }],
    };

    assert_eq!(sponsor_info.cosponsors.len(), 1);
    assert!(!sponsor_info.cosponsors[0].withdrawn);
}

#[test]
fn should_cache_and_retrieve_response() {
    let temp_dir = std::env::temp_dir().join("congress_cache_test");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let cache = ResponseCache::new(temp_dir.clone(), Duration::from_secs(3600));
    let key = "test/member/A000360";
    let data = r#"{"name": "Test"}"#;

    // Cache miss
    assert!(cache.get(key).is_none());

    // Store
    cache.set(key, data).unwrap();

    // Cache hit
    let retrieved = cache.get(key).unwrap();
    assert_eq!(retrieved, data);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn should_expire_stale_cache() {
    let temp_dir = std::env::temp_dir().join("congress_cache_expire_test");
    let _ = std::fs::remove_dir_all(&temp_dir);

    // 0 second TTL = immediate expiry
    let cache = ResponseCache::new(temp_dir.clone(), Duration::from_secs(0));
    let key = "test/expire";

    cache.set(key, "data").unwrap();

    // Should be expired immediately
    assert!(cache.get(key).is_none());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn should_parse_member_from_api_response() {
    let json_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_data/congress/members/L000174.json");
    let json = std::fs::read_to_string(json_path).unwrap();

    let member = Member::from_api_response(&json).unwrap();

    assert_eq!(member.bioguide_id, "L000174");
    assert_eq!(member.first_name, "Patrick");
    assert_eq!(member.last_name, "Leahy");
    assert_eq!(member.party, Party::Democrat);
    assert_eq!(member.state, "VT");
    assert_eq!(member.chamber, Chamber::Senate);
    assert!(member.district.is_none());
    assert!(!member.terms.is_empty());
}

#[test]
fn should_create_congress_client() {
    let temp_dir = std::env::temp_dir().join("congress_client_test");
    let client = CongressClient::new("fake_key".to_string(), temp_dir);
    // Just test construction doesn't panic
    assert!(client.api_key() == "fake_key");
}

#[test]
#[ignore] // Requires live API key - run with: cargo test -- --ignored
fn should_download_bill_data_live() {
    let cache_dir = std::env::temp_dir().join("congress_download_test");
    let _ = std::fs::remove_dir_all(&cache_dir);

    let api_key =
        std::env::var("CONGRESS_API_KEY").expect("Set CONGRESS_API_KEY env var to run this test");

    let client = CongressClient::new(api_key, cache_dir.clone());

    // Use HR 1 (house bill), not pl (public law)
    let download = client.download_bill("119-hr-1").unwrap();

    assert!(!download.bill_xml.is_empty());
    assert!(!download.sponsors_json.is_empty());
    assert!(!download.member_jsons.is_empty());

    let _ = std::fs::remove_dir_all(&cache_dir);
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
        }
    }

    #[test]
    fn should_add_member_to_dataset() {
        let mut dataset = Dataset::new(test_metadata());

        let member = Member {
            bioguide_id: "L000174".into(),
            name: "Patrick J. Leahy".into(),
            first_name: "Patrick".into(),
            last_name: "Leahy".into(),
            party: Party::Democrat,
            state: "VT".into(),
            district: None,
            chamber: Chamber::Senate,
            terms: vec![],
        };

        dataset.add_member(member.clone());

        assert!(dataset.get_member("L000174").is_some());
        assert_eq!(dataset.get_member("L000174").unwrap().last_name, "Leahy");
    }

    #[test]
    fn should_add_sponsor_info_to_dataset() {
        let mut dataset = Dataset::new(test_metadata());

        let sponsor_info = SponsorInfo {
            bill_id: "119-pl-21".into(),
            sponsor: "L000174".into(),
            cosponsors: vec![],
        };

        dataset.add_sponsor_info(sponsor_info);

        assert!(dataset.get_sponsor_info("119-pl-21").is_some());
        assert_eq!(
            dataset.get_sponsor_info("119-pl-21").unwrap().sponsor,
            "L000174"
        );
    }

    #[test]
    fn should_load_bill_download_into_dataset() {
        use words_to_data::congress::BillDownload;

        let mut dataset = Dataset::new(test_metadata());

        // Simulate a BillDownload with test data
        let bill_xml = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/test_data/bills/pl-119-21.xml"),
        )
        .unwrap();

        let member_json = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/test_data/congress/members/L000174.json"),
        )
        .unwrap();

        let mut member_jsons = HashMap::new();
        member_jsons.insert("L000174".to_string(), member_json);

        let download = BillDownload {
            bill_id: "119-pl-21".to_string(),
            bill_xml,
            sponsors_json: r#"{"bill":{"sponsors":[{"bioguideId":"L000174"}]}}"#.to_string(),
            cosponsors_json: r#"{"cosponsors":[]}"#.to_string(),
            votes_json: None,
            member_jsons,
        };

        let bill_id = dataset.load_bill_download(&download).unwrap();

        // Bill parsed - uses canonical ID from XML ("119-21")
        assert_eq!(bill_id, "119-21");
        assert!(dataset.get_bill(&bill_id).is_some());
        // Member loaded
        assert!(dataset.get_member("L000174").is_some());
        // Sponsor info loaded with canonical bill_id
        assert!(dataset.get_sponsor_info(&bill_id).is_some());
    }

    #[test]
    fn should_add_roll_call_to_dataset() {
        let mut dataset = Dataset::new(test_metadata());

        let mut votes = HashMap::new();
        votes.insert("L000174".into(), VotePosition::Yea);

        let roll = RollCall {
            congress: 118,
            session: 1,
            roll_number: 100,
            chamber: Chamber::Senate,
            date: "2023-05-01".into(),
            bill_id: Some("118-hr-1234".into()),
            result: VoteResult::Passed,
            votes,
        };

        dataset.add_roll_call(roll);

        assert_eq!(dataset.roll_calls().len(), 1);
        assert_eq!(dataset.roll_calls()[0].roll_number, 100);
    }
}
