//! Congress members, and the party each of them held on a given day.
//!
//! A member's affiliation is dated, in the same way that an expression is one
//! work as it read on one date (`CONTEXT.md`). The API supplies the history, so
//! all of it is kept: reading a 2025 vote through the party a member holds in
//! 2026 states something false (#105).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;
use time::Date;

use super::CongressError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Party {
    Democrat,
    Republican,
    Independent,
    /// A party this build has no variant for, under the name the API gave it.
    ///
    /// Every member in the cached corpus parses to one of the three above. The
    /// variant stays all the same: one corpus is not the whole legislature, and
    /// a party met for the first time must be carried, not dropped.
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

impl fmt::Display for Party {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Party::Democrat => write!(f, "Democrat"),
            Party::Republican => write!(f, "Republican"),
            Party::Independent => write!(f, "Independent"),
            Party::Other(name) => write!(f, "{name}"),
        }
    }
}

/// One party a member belonged to, over the years the API gives for it.
///
/// The API dates an affiliation by year and not by day, and an end year is the
/// year the member left rather than the last full year in the party. Two
/// affiliations therefore share the year of a change, and a day inside that
/// year belongs to both of them as far as this data can tell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyAffiliation {
    pub party: Party,
    /// The API's own name for the party, such as `Democratic`.
    pub name: String,
    pub start_year: u16,
    /// `None` while the member still holds this affiliation.
    pub end_year: Option<u16>,
}

impl PartyAffiliation {
    /// Whether this affiliation covers a year.
    ///
    /// The end year counts as covered, because the member held the party for
    /// part of it. That is what makes a change year ambiguous instead of
    /// quietly resolving to the party that was left.
    fn covers(&self, year: u16) -> bool {
        self.start_year <= year && self.end_year.is_none_or(|end| year <= end)
    }
}

/// The party a member held on one date, or the reason it cannot be told.
///
/// A party worked out from overlapping years is not a known fact, so it is not
/// reported as one. Every reader can tell the two apart, including a reader of
/// `--json`, which is the same separation between what is known and what is
/// suggested that the rest of the project keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartyOnDate {
    /// One affiliation covers the date.
    Resolved(Party),
    /// More than one affiliation covers the date, because the history carries
    /// years and not days. Every party the date can belong to, oldest first.
    Ambiguous(Vec<Party>),
    /// No affiliation on file covers the date.
    Unknown,
}

impl PartyOnDate {
    /// The one party held on the date, or `None` when it is unresolved.
    ///
    /// Use this where a caller must have one party. It gives no answer rather
    /// than a guess, which is why the ambiguous case is not folded into it.
    pub fn resolved(&self) -> Option<&Party> {
        match self {
            PartyOnDate::Resolved(party) => Some(party),
            PartyOnDate::Ambiguous(_) | PartyOnDate::Unknown => None,
        }
    }
}

impl fmt::Display for PartyOnDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartyOnDate::Resolved(party) => write!(f, "{party}"),
            PartyOnDate::Ambiguous(parties) => {
                let names: Vec<String> = parties.iter().map(Party::to_string).collect();
                write!(f, "unresolved ({})", names.join(" or "))
            }
            PartyOnDate::Unknown => write!(f, "unknown"),
        }
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
    /// Every party this member has belonged to, oldest first.
    ///
    /// There is deliberately no single `party` beside this. One undated field
    /// is read as the party held on whatever day the caller is asking about,
    /// and it holds the party the member has today (#105).
    pub party_history: Vec<PartyAffiliation>,
    pub state: String,
    pub district: Option<u8>,
    pub chamber: Chamber,
    pub terms: Vec<MemberTerm>,
}

impl Member {
    /// The party this member held on one date.
    ///
    /// A roll call carries a date, so this is the only honest way to report the
    /// party behind a vote. Where the party history cannot answer for the date,
    /// the answer says so instead of picking one.
    pub fn party_on(&self, date: Date) -> PartyOnDate {
        let Ok(year) = u16::try_from(date.year()) else {
            return PartyOnDate::Unknown;
        };

        let mut covering: Vec<Party> = self
            .party_history
            .iter()
            .filter(|affiliation| affiliation.covers(year))
            .map(|affiliation| affiliation.party.clone())
            .collect();

        match covering.len() {
            0 => PartyOnDate::Unknown,
            1 => PartyOnDate::Resolved(covering.remove(0)),
            _ => PartyOnDate::Ambiguous(covering),
        }
    }

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

        // The whole party history, sorted by the field that decides the order.
        // The API happens to list it newest first and promises nothing, so the
        // order is made here rather than assumed.
        let mut party_history: Vec<PartyAffiliation> = member["partyHistory"]
            .as_array()
            .map(|entries| entries.iter().map(party_affiliation).collect())
            .unwrap_or_default();
        party_history.sort_by_key(|affiliation| affiliation.start_year);

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

        // Same again for the terms: the API lists them oldest first, so the
        // order is imposed here before the most recent one is read off the end.
        terms.sort_by_key(|term| (term.start_year, term.congress));

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
            party_history,
            state,
            district,
            chamber,
            terms,
        })
    }
}

/// One `partyHistory` entry from a member response.
///
/// An entry with no abbreviation keeps the name the API gave, because a dated
/// affiliation this build cannot name is still a dated affiliation.
fn party_affiliation(entry: &Value) -> PartyAffiliation {
    let abbreviation = entry["partyAbbreviation"].as_str().unwrap_or("");
    let name = entry["partyName"]
        .as_str()
        .unwrap_or(abbreviation)
        .to_string();

    PartyAffiliation {
        party: abbreviation
            .parse::<Party>()
            .unwrap_or(Party::Other(abbreviation.to_string())),
        name,
        start_year: entry["startYear"].as_u64().unwrap_or(0) as u16,
        end_year: entry["endYear"].as_u64().map(|year| year as u16),
    }
}
