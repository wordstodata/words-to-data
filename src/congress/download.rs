use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Raw downloaded data for a bill, ready for Dataset to parse
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillDownload {
    pub bill_id: String,
    pub bill_xml: String,
    pub sponsors_json: String,
    pub cosponsors_json: String,
    pub votes_json: Option<String>,
    pub member_jsons: HashMap<String, String>,
}
