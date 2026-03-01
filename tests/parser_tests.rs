use words_to_data::{
    diff::TreeDiff,
    uslm::{USLMElement, parser::parse},
};

#[test]
fn test_parse_usc_title_7() {
    let result = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18");
    assert!(
        result.is_ok(),
        "Failed to parse USC Title 7: {:?}",
        result.err()
    );

    let root = result.unwrap();
    // Check that the root path is in USLM format
    assert_eq!(root.data.uslm_id.unwrap(), "/us/usc/t7");

    // Check that children also have USLM format paths
    // In USC, the first child is a Title element which has the same path as the document
    if !root.children.is_empty() {
        // The first child is the Title, which should have path /us/usc/t7
        if let Some(uslm_id) = &root.children[0].data.uslm_id {
            assert_eq!(
                uslm_id, "/us/usc/t7",
                "First child (Title) should have same path as document"
            );
        }
    }
}

#[test]
fn test_parse_public_law() {
    let result = parse("tests/test_data/bills/hr-119-21.xml", "2025-07-04");
    assert!(
        result.is_ok(),
        "Failed to parse Public Law: {:?}",
        result.err()
    );

    let root = result.unwrap();
    // Check that the root path is in USLM format
    // Note: XML uses "119-21" format (with hyphen)
    let uslm_id = root.data.uslm_id.unwrap();
    assert_eq!(uslm_id, "/us/pl/119-21");

    // Check that children have structural format paths
    for child in &root.children {
        if let Some(uslm_id) = &child.data.uslm_id {
            assert!(uslm_id.starts_with("/us/pl/119-21/"));
        }
    }
}

#[test]
fn test_parse_and_diff_usc_title() {
    println!("\n=== Testing Parse and Diff with USLM Format ===\n");

    // Test with Title 7 (Agriculture) - it has changes between dates
    let title = "07";
    let date1 = "2025-07-18";
    let date2 = "2025-07-30";

    println!("Parsing USC Title {} from {}", title, date1);
    let file1 = format!("tests/test_data/usc/{}/usc{}.xml", date1, title);
    let tree1 = parse(&file1, date1).expect("Failed to parse first file");
    println!("✓ Parsed {} elements", count_elements(&tree1));
    println!("  Root path: {}", tree1.data.path);

    println!("\nParsing USC Title {} from {}", title, date2);
    let file2 = format!("tests/test_data/usc/{}/usc{}.xml", date2, title);
    let tree2 = parse(&file2, date2).expect("Failed to parse second file");
    println!("✓ Parsed {} elements", count_elements(&tree2));
    println!("  Root path: {}", tree2.data.path);

    println!("\nGenerating diff...");
    let diff = TreeDiff::from_elements(&tree1, &tree2);

    println!("\n=== Diff Results ===");
    println!("Root path: {}", diff.root_path);
    println!("Field changes: {}", diff.changes.len());
    println!("Elements added: {}", diff.added.len());
    println!("Elements removed: {}", diff.removed.len());
    println!("Child diffs: {}", diff.child_diffs.len());

    if !diff.added.is_empty() {
        println!("\nSample added elements (first 5):");
        for elem in diff.added.iter().take(5) {
            println!("  + {}", elem.path);
        }
    }

    if !diff.removed.is_empty() {
        println!("\nSample removed elements (first 5):");
        for elem in diff.removed.iter().take(5) {
            println!("  - {}", elem.path);
        }
    }

    if !diff.changes.is_empty() {
        println!("\nSample field changes (first 5):");
        for change in diff.changes.iter().take(5) {
            println!("  ~ {} ({:?})", diff.root_path, change.field_name);
        }
    }

    // Validate USLM format in diff paths
    println!("\nValidating USLM format in diff paths...");
    for elem in &diff.added {
        assert!(
            elem.path.starts_with("/us/usc/t"),
            "Added path should use USLM format: {}",
            elem.path
        );
    }
    for elem in &diff.removed {
        assert!(
            elem.path.starts_with("/us/usc/t"),
            "Removed path should use USLM format: {}",
            elem.path
        );
    }
    assert!(
        diff.root_path.starts_with("uscodedocument_"),
        "Root path should use structural format: {}",
        diff.root_path
    );
    println!("✓ All diff paths use USLM format");
}

#[test]
#[ignore] // Ignore by default - this parses all USC titles (slow)
fn test_parse_all_usc_titles() {
    use std::fs;

    println!("\n=== Comprehensive USC Parse & Diff Test ===\n");

    let date1 = "2025-07-18";
    let date2 = "2025-07-30";

    // Get all XML files from first date directory
    let dir1 = format!("usc_data/{}", date1);
    let entries = fs::read_dir(&dir1).expect("Failed to read USC data directory");

    let mut title_files: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "xml" {
                let filename = path.file_name()?.to_str()?.to_string();
                // Extract title identifier (e.g., "07" from "usc07.xml")
                if filename.starts_with("usc") && filename.ends_with(".xml") {
                    let title = filename.strip_prefix("usc")?.strip_suffix(".xml")?;
                    Some(title.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    title_files.sort();

    println!("Found {} USC titles to test\n", title_files.len());

    for title in title_files.iter() {
        print!("[{}] Testing Title {}... ", title_files.len(), title);
        std::io::Write::flush(&mut std::io::stdout()).unwrap();

        let file1 = format!("usc_data/{}/usc{}.xml", date1, title);
        let file2 = format!("usc_data/{}/usc{}.xml", date2, title);

        // Parse both versions
        let tree1 = parse(&file1, date1).unwrap();

        let tree2 = parse(&file2, date2).unwrap();

        // Validate USLM format on root path
        // Note: File names may have leading zeros (usc01.xml) but XML uses title numbers without them (t1)
        let title_num = title.trim_start_matches('0');
        let title_num = if title_num.is_empty() { "0" } else { title_num }; // Handle edge case of "00"
        let expected_path_prefix = format!("/us/usc/t{}", title_num.to_lowercase());
        if let Some(uslm_id) = &tree1.data.uslm_id {
            assert!(*uslm_id == expected_path_prefix);
        }

        // Generate diff
        let _diff = TreeDiff::from_elements(&tree1, &tree2);
    }
}

fn count_elements(elem: &USLMElement) -> usize {
    1 + elem
        .children
        .iter()
        .map(|c| count_elements(c))
        .sum::<usize>()
}
