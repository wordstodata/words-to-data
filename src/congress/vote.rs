use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;
use time::Date;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::CongressError;
use crate::date::date_str_to_date;

/// How a member voted on a roll call
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum VotePosition {
    Yea,
    Nay,
    NotVoting,
    Present,
}

impl fmt::Display for VotePosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VotePosition::Yea => write!(f, "Yea"),
            VotePosition::Nay => write!(f, "Nay"),
            VotePosition::NotVoting => write!(f, "Not Voting"),
            VotePosition::Present => write!(f, "Present"),
        }
    }
}

impl FromStr for VotePosition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "yea" | "aye" | "yes" => Ok(VotePosition::Yea),
            "nay" | "no" => Ok(VotePosition::Nay),
            "not voting" | "notvoting" | "not_voting" => Ok(VotePosition::NotVoting),
            "present" => Ok(VotePosition::Present),
            other => Err(format!("Unknown vote position: {}", other)),
        }
    }
}

/// A single member's vote on a roll call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberVote {
    pub bioguide_id: String,
    pub position: VotePosition,
}

/// A House roll call vote with all member votes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HouseRollCall {
    pub congress: u16,
    pub session: u8,
    pub roll_number: u32,
    pub date: String,
    pub question: String,
    pub result: String,
    pub yea_count: u32,
    pub nay_count: u32,
    pub not_voting_count: u32,
    pub present_count: u32,
    pub member_votes: Vec<MemberVote>,
}

impl HouseRollCall {
    /// The day this roll call was held.
    ///
    /// This is what joins a vote to the party the member held, so it is read
    /// from the roll call rather than worked out by each caller. The API gives
    /// an offset date-time, such as `2025-07-03T14:31:00-04:00`, and the day is
    /// taken in the offset it supplied: a vote held late in the evening belongs
    /// to the day the chamber sat.
    ///
    /// `None` means the date is not one this build can read. A caller must
    /// answer that it does not know, not pick a day.
    pub fn day(&self) -> Option<Date> {
        if let Ok(moment) = OffsetDateTime::parse(&self.date, &Rfc3339) {
            return Some(moment.date());
        }
        date_str_to_date(self.date.get(..10)?).ok()
    }

    /// Parse from Congress API house-vote response JSON
    ///
    /// Expects the response from `/house-vote/{congress}/{session}/{voteNumber}`
    /// combined with `/house-vote/{congress}/{session}/{voteNumber}/members`
    pub fn from_api_response(vote_json: &str, members_json: &str) -> Result<Self, CongressError> {
        let vote: Value = serde_json::from_str(vote_json)?;
        let members: Value = serde_json::from_str(members_json)?;

        // API returns "houseRollCallVote" as root key
        let vote_data = &vote["houseRollCallVote"];

        let congress = vote_data["congress"]
            .as_u64()
            .ok_or_else(|| CongressError::Parse("missing congress".into()))?
            as u16;

        let session = vote_data["sessionNumber"]
            .as_u64()
            .ok_or_else(|| CongressError::Parse("missing sessionNumber".into()))?
            as u8;

        let roll_number = vote_data["rollCallNumber"]
            .as_u64()
            .ok_or_else(|| CongressError::Parse("missing rollCallNumber".into()))?
            as u32;

        // API uses startDate
        let date = vote_data["startDate"].as_str().unwrap_or("").to_string();

        let question = vote_data["voteQuestion"].as_str().unwrap_or("").to_string();

        let result = vote_data["result"].as_str().unwrap_or("").to_string();

        // Parse vote totals from votePartyTotal array
        let mut yea_count = 0u32;
        let mut nay_count = 0u32;
        let mut not_voting_count = 0u32;
        let mut present_count = 0u32;

        if let Some(party_totals) = vote_data["votePartyTotal"].as_array() {
            for pt in party_totals {
                yea_count += pt["yeaTotal"].as_u64().unwrap_or(0) as u32;
                nay_count += pt["nayTotal"].as_u64().unwrap_or(0) as u32;
                not_voting_count += pt["notVotingTotal"].as_u64().unwrap_or(0) as u32;
                present_count += pt["presentTotal"].as_u64().unwrap_or(0) as u32;
            }
        }

        // Parse member votes from houseRollCallVoteMemberVotes.results
        let mut member_votes = Vec::new();
        let members_data = &members["houseRollCallVoteMemberVotes"];
        if let Some(arr) = members_data["results"].as_array() {
            for m in arr {
                // API uses bioguideID (uppercase ID)
                let bioguide_id = m["bioguideID"].as_str().unwrap_or("").to_string();

                if bioguide_id.is_empty() {
                    continue;
                }

                // API uses voteCast with "Aye"/"Nay" etc
                let vote_str = m["voteCast"].as_str().unwrap_or("");

                if let Ok(position) = vote_str.parse::<VotePosition>() {
                    member_votes.push(MemberVote {
                        bioguide_id,
                        position,
                    });
                }
            }
        }

        Ok(HouseRollCall {
            congress,
            session,
            roll_number,
            date,
            question,
            result,
            yea_count,
            nay_count,
            not_voting_count,
            present_count,
            member_votes,
        })
    }
}

/// All roll call votes for a bill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillVotes {
    pub bill_id: String,
    pub roll_calls: Vec<HouseRollCall>,
}

/// Reference to a recorded vote from bill actions
#[derive(Debug, Clone)]
pub struct RecordedVoteRef {
    pub chamber: String,
    pub roll_number: u32,
    pub session: u8,
    pub congress: u16,
    pub date: String,
}

impl RecordedVoteRef {
    /// Extract House roll call references from bill actions JSON
    ///
    /// Parses the `/bill/{congress}/{type}/{number}/actions` response
    /// and returns only House chamber votes
    pub fn extract_house_votes_from_actions(
        actions_json: &str,
    ) -> Result<Vec<Self>, CongressError> {
        let v: Value = serde_json::from_str(actions_json)?;
        let mut refs = Vec::new();

        if let Some(actions) = v["actions"].as_array() {
            for action in actions {
                if let Some(recorded_votes) = action["recordedVotes"].as_array() {
                    for rv in recorded_votes {
                        let chamber = rv["chamber"].as_str().unwrap_or("");
                        if chamber != "House" {
                            continue;
                        }

                        let roll_number = rv["rollNumber"].as_u64().unwrap_or(0) as u32;
                        let session = rv["sessionNumber"].as_u64().unwrap_or(1) as u8;
                        let congress = rv["congress"].as_u64().unwrap_or(0) as u16;
                        let date = rv["date"].as_str().unwrap_or("").to_string();

                        if roll_number > 0 && congress > 0 {
                            refs.push(RecordedVoteRef {
                                chamber: chamber.to_string(),
                                roll_number,
                                session,
                                congress,
                                date,
                            });
                        }
                    }
                }
            }
        }

        Ok(refs)
    }
}
