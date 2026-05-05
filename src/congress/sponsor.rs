use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CosponsorRecord {
    pub bioguide_id: String,
    pub date: String,
    pub withdrawn: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SponsorInfo {
    pub bill_id: String,
    pub sponsor: String,
    pub cosponsors: Vec<CosponsorRecord>,
}
