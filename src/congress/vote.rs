use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

use super::Chamber;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VotePosition {
    Yea,
    Nay,
    Present,
    NotVoting,
}

impl FromStr for VotePosition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Yea" | "yea" | "Yes" | "yes" | "Aye" | "aye" => Ok(VotePosition::Yea),
            "Nay" | "nay" | "No" | "no" => Ok(VotePosition::Nay),
            "Present" | "present" => Ok(VotePosition::Present),
            "Not Voting" | "NotVoting" | "not voting" => Ok(VotePosition::NotVoting),
            other => Err(format!("Unknown vote position: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteResult {
    Passed,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollCall {
    pub congress: u16,
    pub session: u8,
    pub roll_number: u32,
    pub chamber: Chamber,
    pub date: String,
    pub bill_id: Option<String>,
    pub result: VoteResult,
    pub votes: HashMap<String, VotePosition>,
}
