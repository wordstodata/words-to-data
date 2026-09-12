use words_to_data::uscode::{MirrorIndex, extract_release};

#[test]
fn should_look_up_download_url_by_date_when_present_in_manifest() {
    let json = std::fs::read_to_string("tests/test_data/mirror/index.json").unwrap();
    let index: MirrorIndex = serde_json::from_str(&json).unwrap();

    assert_eq!(index.versions.len(), 3);
    assert_eq!(
        index.url_for("2025-07-18"),
        Some("https://www.wordstodata.com/mirror/uslm/2025-07-18.zip")
    );
    assert_eq!(index.url_for("1999-01-01"), None);
}

#[test]
fn should_extract_all_xml_files_flat_when_given_a_release_zip() {
    let zip_bytes = std::fs::read("tests/test_data/mirror/sample.zip").unwrap();
    let dir = tempfile::tempdir().expect("a temporary directory");
    let dest = dir.path().join("release");

    extract_release(&zip_bytes, &dest).unwrap();

    // Both titles land as flat xml files in the destination.
    let mut names: Vec<String> = std::fs::read_dir(&dest)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["usc01.xml", "usc09.xml"]);
}
