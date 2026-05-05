use std::{str::FromStr, thread};

use serde::{Deserialize, Serialize};
use words_to_data::{
    annotation::{AnnotationMetadata, AnnotationStatus, BillReference, ChangeAnnotation},
    dataset::{Dataset, DatasetMetadata, VersionSnapshot},
    diff::{MentionMatch, TreeDiff},
    uslm::{
        AmendingAction,
        bill_parser::{AmendmentData, parse_bill_amendments},
    },
    utils::parse_uslm_xml,
};

/// Response structure for load_dataset command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetLoadResponse {
    /// The computed tree diff between versions
    pub tree_diff: TreeDiff,

    /// All annotations from the dataset
    pub annotations: Vec<ChangeAnnotation>,

    /// All bills from the dataset
    pub bills: Vec<AmendmentData>,

    /// Version dates
    pub from_date: String,
    pub to_date: String,
}

/// Scan all amendment texts for mentions of changed sections.
///
/// Uses the library's `scan_for_mentions()` method on TreeDiff to find
/// section mentions in amendment texts.
///
/// Returns a map from amendment_id to list of matches found.
#[tauri::command]
fn scan_amendments_for_mentions(
    tree_diff_json: String,
    bills_json: String,
) -> Result<String, String> {
    let tree_diff: TreeDiff = serde_json::from_str(&tree_diff_json).map_err(|e| e.to_string())?;
    let bills: Vec<AmendmentData> = serde_json::from_str(&bills_json).map_err(|e| e.to_string())?;

    // Aggregate results across all bills
    let mut results: std::collections::HashMap<String, Vec<MentionMatch>> =
        std::collections::HashMap::new();

    for bill in &bills {
        let mentions = tree_diff.scan_for_mentions(bill);
        for (amendment_id, matches) in mentions {
            results.entry(amendment_id).or_default().extend(matches);
        }
    }

    serde_json::to_string(&results).map_err(|e| e.to_string())
}

/// Load two USC XML files, compute and return the TreeDiff as a JSON string.
///
/// Parsing large XML files (e.g. Title 26) is CPU-intensive, so this runs on
#[tauri::command]
async fn load_usc_pair(
    old_path: String,
    old_date: String,
    new_path: String,
    new_date: String,
) -> Result<String, String> {
    let old_handle =
        thread::spawn(move || parse_uslm_xml(&old_path, &old_date).map_err(|e| e.to_string()));
    let new_handle =
        thread::spawn(move || parse_uslm_xml(&new_path, &new_date).map_err(|e| e.to_string()));

    let old = old_handle.join().unwrap()?;
    let new = new_handle.join().unwrap()?;
    let diff = TreeDiff::from_elements(&old, &new);
    serde_json::to_string(&diff).map_err(|e| e.to_string())
}

/// Extract bill_id from filename (e.g., "bill_119_hr_1.xml" -> "119-hr-1")
fn extract_bill_id(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.replace('_', "-").trim_start_matches("bill-").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Parse a bill XML file and return the AmendmentData as a JSON string.
#[tauri::command]
async fn load_bill(path: String) -> Result<String, String> {
    let bill_handle = thread::spawn(move || {
        let bill_id = extract_bill_id(&path);
        let data = parse_bill_amendments(&bill_id, &path).map_err(|e| e.to_string())?;
        serde_json::to_string(&data).map_err(|e| e.to_string())
    });
    bill_handle.join().unwrap()
}

/// Parse multiple bill XML files and return all AmendmentData as a JSON array string.
#[tauri::command]
async fn load_bills(paths: Vec<String>) -> Result<String, String> {
    let handles: Vec<_> = paths
        .into_iter()
        .map(|path| {
            thread::spawn(move || {
                let bill_id = extract_bill_id(&path);
                parse_bill_amendments(&bill_id, &path).map_err(|e| e.to_string())
            })
        })
        .collect();

    let mut results = Vec::new();
    for handle in handles {
        let data = handle.join().unwrap()?;
        results.push(data);
    }

    serde_json::to_string(&results).map_err(|e| e.to_string())
}

/// Build a ChangeAnnotation from form fields and return it as a JSON string.
///
/// The timestamp is set server-side to UTC now. The status is always Pending
/// for human annotations created through this tool.
#[tauri::command]
fn create_annotation(
    operation: String,
    bill_id: String,
    amendment_id: String,
    causative_text: String,
    paths: Vec<String>,
    annotator: String,
    notes: Option<String>,
) -> Result<String, String> {
    let op = AmendingAction::from_str(&operation).map_err(|e| e.to_string())?;
    let annotation = ChangeAnnotation {
        operation: op,
        source_bill: BillReference {
            bill_id,
            amendment_id,
            causative_text,
        },
        paths,
        metadata: AnnotationMetadata {
            status: AnnotationStatus::Pending,
            confidence: None,
            annotator,
            timestamp: time::OffsetDateTime::now_utc(),
            notes,
            reasoning: None,
        },
    };
    serde_json::to_string(&annotation).map_err(|e| e.to_string())
}

/// Save a Dataset containing versions, annotations, and bills.
///
/// Reloads USC files to get full USLMElement trees for the Dataset.
#[tauri::command]
fn save_dataset(
    usc_old_path: String,
    usc_old_date: String,
    usc_new_path: String,
    usc_new_date: String,
    annotations_json: String,
    bills_json: String,
    output_path: String,
) -> Result<(), String> {
    let annotations: Vec<ChangeAnnotation> =
        serde_json::from_str(&annotations_json).map_err(|e| e.to_string())?;
    let bills: Vec<AmendmentData> = serde_json::from_str(&bills_json).map_err(|e| e.to_string())?;

    // Reload USC files to get full elements
    let old_element = parse_uslm_xml(&usc_old_path, &usc_old_date).map_err(|e| e.to_string())?;
    let new_element = parse_uslm_xml(&usc_new_path, &usc_new_date).map_err(|e| e.to_string())?;

    // Create dataset metadata
    let metadata = DatasetMetadata {
        name: "Annotated USC Changes".to_string(),
        description: format!(
            "Changes from {} to {} with bill annotations",
            usc_old_date, usc_new_date
        ),
        author: "Annotator Tool".to_string(),
        source_urls: vec![],
        license: "Public Domain".to_string(),
        version: "1.0.0".to_string(),
    };

    let mut dataset = Dataset::new(metadata);

    // Add versions
    dataset.add_version(VersionSnapshot {
        date: usc_old_date.clone(),
        label: Some("Before".to_string()),
        element: old_element,
    });
    dataset.add_version(VersionSnapshot {
        date: usc_new_date.clone(),
        label: Some("After".to_string()),
        element: new_element,
    });

    // Add bills
    for bill in bills {
        dataset.add_bill(bill);
    }

    // Add annotations for this version pair
    for annotation in annotations {
        dataset.add_annotation(&usc_old_date, &usc_new_date, annotation);
    }

    dataset.save(&output_path).map_err(|e| e.to_string())
}

/// Load a Dataset and return the TreeDiff, annotations, and bills.
///
/// Computes the diff between the first two versions in the dataset.
#[tauri::command]
fn load_dataset(path: String) -> Result<String, String> {
    let dataset = Dataset::load(&path).map_err(|e| e.to_string())?;

    if dataset.versions.len() < 2 {
        return Err("Dataset must have at least 2 versions".to_string());
    }

    let from_date = &dataset.versions[0].date;
    let to_date = &dataset.versions[1].date;

    let tree_diff = dataset
        .compute_diff(from_date, to_date)
        .map_err(|e| e.to_string())?;

    let annotations = dataset
        .get_annotations(from_date, to_date)
        .cloned()
        .unwrap_or_default();

    let response = DatasetLoadResponse {
        tree_diff,
        annotations,
        bills: dataset.bills.clone(),
        from_date: from_date.clone(),
        to_date: to_date.clone(),
    };

    serde_json::to_string(&response).map_err(|e| e.to_string())
}

/// Read an arbitrary JSON file and return its contents.
///
/// Used for loading external data like similarity scores.
#[tauri::command]
fn read_json_file(path: String) -> Result<String, String> {
    let contents = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    // Validate it's valid JSON by parsing it
    let _: serde_json::Value = serde_json::from_str(&contents).map_err(|e| e.to_string())?;
    Ok(contents)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_usc_pair,
            load_bill,
            load_bills,
            create_annotation,
            save_dataset,
            load_dataset,
            read_json_file,
            scan_amendments_for_mentions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
