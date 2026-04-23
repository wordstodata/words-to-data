use std::{str::FromStr, thread};

use serde::{Deserialize, Serialize};
use words_to_data::{
    diff::{MentionMatch, TreeDiff},
    legal_diff::{
        AnnotationMetadata, AnnotationStatus, BillReference, ChangeAnnotation, LegalDiff,
    },
    uslm::{
        AmendingAction,
        bill_parser::{AmendmentData, parse_bill_amendments},
    },
    utils::parse_uslm_xml,
};

/// A workspace containing all state for an annotation session.
///
/// This allows saving and loading the complete state of an annotation session,
/// including the tree diff, loaded bills, and annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// When this workspace was created/last saved
    #[serde(with = "time::serde::rfc3339")]
    pub saved_at: time::OffsetDateTime,

    /// The computed tree diff between old and new USC versions
    pub tree_diff: TreeDiff,

    /// USC file metadata for reference
    pub usc_old_path: String,
    pub usc_old_date: String,
    pub usc_new_path: String,
    pub usc_new_date: String,

    /// All loaded bill data
    pub bills: Vec<AmendmentData>,

    /// Original paths of loaded bills
    pub bill_paths: Vec<String>,

    /// Annotations created during this session
    pub annotations: Vec<ChangeAnnotation>,
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

/// Parse a bill XML file and return the AmendmentData as a JSON string.
#[tauri::command]
async fn load_bill(path: String) -> Result<String, String> {
    let bill_handle = thread::spawn(move || {
        let data = parse_bill_amendments(&path).map_err(|e| e.to_string())?;
        serde_json::to_string(&data).map_err(|e| e.to_string())
    });
    bill_handle.join().unwrap()
}

/// Parse multiple bill XML files and return all AmendmentData as a JSON array string.
#[tauri::command]
async fn load_bills(paths: Vec<String>) -> Result<String, String> {
    let handles: Vec<_> = paths
        .into_iter()
        .map(|path| thread::spawn(move || parse_bill_amendments(&path).map_err(|e| e.to_string())))
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

/// Combine a TreeDiff with a list of annotations into a LegalDiff and write
/// it as pretty-printed JSON to the given output path.
///
/// The amendments parameter contains all bill amendments; only those referenced
/// by the annotations will be included in the output.
#[tauri::command]
fn export_legal_diff(
    tree_diff_json: String,
    annotations_json: String,
    bills_json: String,
    output_path: String,
) -> Result<(), String> {
    let tree_diff: TreeDiff = serde_json::from_str(&tree_diff_json).map_err(|e| e.to_string())?;
    let annotations: Vec<ChangeAnnotation> =
        serde_json::from_str(&annotations_json).map_err(|e| e.to_string())?;
    let bills: Vec<AmendmentData> = serde_json::from_str(&bills_json).map_err(|e| e.to_string())?;

    // Collect amendment IDs referenced by annotations
    let referenced_ids: std::collections::HashSet<&str> = annotations
        .iter()
        .map(|a| a.source_bill.amendment_id.as_str())
        .collect();

    // Build a map of only the referenced amendments
    let mut amendments = std::collections::HashMap::new();
    for bill in &bills {
        for (id, amendment) in &bill.amendments {
            if referenced_ids.contains(id.as_str()) {
                amendments.insert(id.clone(), amendment.clone());
            }
        }
    }

    let mut legal_diff = LegalDiff::new(&tree_diff);
    legal_diff.set_amendments(amendments);
    for annotation in annotations {
        legal_diff.add_annotation(annotation);
    }
    let json = serde_json::to_string_pretty(&legal_diff).map_err(|e| e.to_string())?;
    std::fs::write(&output_path, json).map_err(|e| e.to_string())
}

/// Save workspace state to a JSON file.
///
/// Combines all session state (tree diff, bills, annotations) into a single
/// file that can be loaded later to continue work.
#[tauri::command]
fn save_workspace(
    tree_diff_json: String,
    usc_old_path: String,
    usc_old_date: String,
    usc_new_path: String,
    usc_new_date: String,
    bills_json: String,
    bill_paths: Vec<String>,
    annotations_json: String,
    output_path: String,
) -> Result<(), String> {
    let tree_diff: TreeDiff = serde_json::from_str(&tree_diff_json).map_err(|e| e.to_string())?;
    let bills: Vec<AmendmentData> = serde_json::from_str(&bills_json).map_err(|e| e.to_string())?;
    let annotations: Vec<ChangeAnnotation> =
        serde_json::from_str(&annotations_json).map_err(|e| e.to_string())?;

    let workspace = Workspace {
        saved_at: time::OffsetDateTime::now_utc(),
        tree_diff,
        usc_old_path,
        usc_old_date,
        usc_new_path,
        usc_new_date,
        bills,
        bill_paths,
        annotations,
    };

    let json = serde_json::to_string_pretty(&workspace).map_err(|e| e.to_string())?;
    std::fs::write(&output_path, json).map_err(|e| e.to_string())
}

/// Load a workspace from a JSON file.
///
/// Returns the full workspace state as a JSON string.
#[tauri::command]
fn load_workspace(path: String) -> Result<String, String> {
    let contents = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    // Validate it's a valid workspace by parsing it
    let _workspace: Workspace = serde_json::from_str(&contents).map_err(|e| e.to_string())?;
    // Return the raw JSON string (frontend will parse it)
    Ok(contents)
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

/// Load a LegalDiff JSON file for review.
///
/// Validates the file is a valid LegalDiff and returns the raw JSON for frontend parsing.
#[tauri::command]
fn load_legal_diff(path: String) -> Result<String, String> {
    let contents = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    // Validate it's a valid LegalDiff by parsing it
    let _legal_diff: LegalDiff = serde_json::from_str(&contents).map_err(|e| e.to_string())?;
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
            export_legal_diff,
            save_workspace,
            load_workspace,
            read_json_file,
            scan_amendments_for_mentions,
            load_legal_diff,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
