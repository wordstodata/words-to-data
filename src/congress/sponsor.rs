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
    /// The sponsor's bioguide id, not their name. Use it to read the Member.
    ///
    /// The field name says `sponsor` and the type says `String`, so this reads
    /// like a name and is not one. A rename would change the key stored in a
    /// W2D file, which is a break for readability alone, so the name stays and
    /// this comment carries the meaning instead.
    pub sponsor: String,
    pub cosponsors: Vec<CosponsorRecord>,
}
