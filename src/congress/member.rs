use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

use super::CongressError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Party {
    Democrat,
    Republican,
    Independent,
    Other(String),
}

impl FromStr for Party {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "D" | "Democrat" | "Democratic" => Party::Democrat,
            "R" | "Republican" => Party::Republican,
            "I" | "ID" | "Independent" => Party::Independent,
            other => Party::Other(other.to_string()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Chamber {
    Senate,
    House,
}

impl FromStr for Chamber {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Senate" | "senate" | "S" => Ok(Chamber::Senate),
            "House" | "house" | "H" | "House of Representatives" => Ok(Chamber::House),
            other => Err(format!("Unknown chamber: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberTerm {
    pub congress: u16,
    pub chamber: Chamber,
    pub state: String,
    pub district: Option<u8>,
    pub start_year: u16,
    pub end_year: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub bioguide_id: String,
    pub name: String,
    pub first_name: String,
    pub last_name: String,
    pub party: Party,
    pub state: String,
    pub district: Option<u8>,
    pub chamber: Chamber,
    pub terms: Vec<MemberTerm>,
}

impl Member {
    pub fn from_api_response(json: &str) -> Result<Self, CongressError> {
        let v: Value = serde_json::from_str(json)?;
        let member = &v["member"];

        let bioguide_id = member["bioguideId"]
            .as_str()
            .ok_or_else(|| CongressError::Parse("missing bioguideId".into()))?
            .to_string();

        let first_name = member["firstName"].as_str().unwrap_or("").to_string();

        let last_name = member["lastName"].as_str().unwrap_or("").to_string();

        let name = member["directOrderName"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| format!("{} {}", first_name, last_name));

        // Parse party from partyHistory (most recent)
        let party = member["partyHistory"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|p| p["partyAbbreviation"].as_str())
            .map(|s| s.parse::<Party>().unwrap_or(Party::Other(s.to_string())))
            .unwrap_or(Party::Other("Unknown".to_string()));

        // Parse terms
        let mut terms = Vec::new();
        if let Some(terms_arr) = member["terms"].as_array() {
            for term in terms_arr {
                let chamber_str = term["chamber"].as_str().unwrap_or("House");
                let chamber = chamber_str.parse::<Chamber>().unwrap_or(Chamber::House);

                let state_code = term["stateCode"].as_str().unwrap_or("").to_string();

                let congress = term["congress"].as_u64().unwrap_or(0) as u16;
                let start_year = term["startYear"].as_u64().unwrap_or(0) as u16;
                let end_year = term["endYear"].as_u64().map(|y| y as u16);

                let district = term["district"].as_u64().map(|d| d as u8);

                terms.push(MemberTerm {
                    congress,
                    chamber,
                    state: state_code,
                    district,
                    start_year,
                    end_year,
                });
            }
        }

        // Get state and chamber from most recent term
        let (state, chamber, district) = terms
            .last()
            .map(|t| (t.state.clone(), t.chamber, t.district))
            .unwrap_or(("".to_string(), Chamber::House, None));

        Ok(Member {
            bioguide_id,
            name,
            first_name,
            last_name,
            party,
            state,
            district,
            chamber,
            terms,
        })
    }
}
