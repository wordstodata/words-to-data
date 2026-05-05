use std::collections::HashMap;
use std::path::PathBuf;
use words_to_data::congress::{Chamber, CongressClient, CongressError, Member, Party, SponsorInfo};

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
#[ignore] // Requires live API key - run with: cargo test -- --ignored
fn should_download_bill_data_live() {
    let cache_dir = std::env::temp_dir().join("congress_download_test");
    let _ = std::fs::remove_dir_all(&cache_dir);

    let api_key =
        std::env::var("CONGRESS_API_KEY").expect("Set CONGRESS_API_KEY env var to run this test");

    let client = CongressClient::new(api_key);

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
            bill_id: "119-hr-1".into(),
            sponsor: "L000174".into(),
            cosponsors: vec![],
        };

        dataset.add_sponsor_info(sponsor_info);

        assert!(dataset.get_sponsor_info("119-hr-1").is_some());
        assert_eq!(
            dataset.get_sponsor_info("119-hr-1").unwrap().sponsor,
            "L000174"
        );
    }

    #[test]
    fn should_load_bill_download_into_dataset() {
        use words_to_data::congress::BillDownload;

        let mut dataset = Dataset::new(test_metadata());

        // Simulate a BillDownload with test data
        let bill_xml = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/test_data/bills/119-hr-1/bill_119_hr_1.xml"),
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
            bill_id: "119-hr-1".to_string(),
            bill_xml,
            sponsors_json: r#"{"bill":{"sponsors":[{"bioguideId":"L000174"}]}}"#.to_string(),
            cosponsors_json: r#"{"cosponsors":[]}"#.to_string(),
            votes_json: None,
            member_jsons,
        };

        let bill_id = dataset.load_bill_download(&download).unwrap();

        // Bill parsed - uses bill_id from BillDownload
        assert_eq!(bill_id, "119-hr-1");
        assert!(dataset.get_bill(&bill_id).is_some());
        // Member loaded
        assert!(dataset.get_member("L000174").is_some());
        // Sponsor info loaded with canonical bill_id
        assert!(dataset.get_sponsor_info(&bill_id).is_some());
    }
}
