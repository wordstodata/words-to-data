use std::time::Duration;

use super::{
    BillDownload, CongressError, CosponsorRecord, HouseRollCall, Member, RecordedVoteRef,
    ResponseCache, SponsorInfo,
};
use serde_json::Value;
use std::collections::HashMap;

const BASE_URL: &str = "https://api.congress.gov/v3";
const DEFAULT_TTL_SECS: u64 = 24 * 60 * 60; // 24 hours

pub struct CongressClient {
    api_key: String,
    cache: ResponseCache,
    agent: ureq::Agent,
}

impl CongressClient {
    pub fn new(api_key: String, cache_dir: Option<String>) -> Self {
        Self::with_ttl(
            api_key,
            cache_dir,
            Some(Duration::from_secs(DEFAULT_TTL_SECS)),
        )
    }

    /// Build a client with an explicit cache TTL. Pass `None` to make cached
    /// entries never expire, e.g. when reading from committed test fixtures.
    pub fn with_ttl(api_key: String, cache_dir: Option<String>, ttl: Option<Duration>) -> Self {
        let cache_path = cache_dir.map(std::path::PathBuf::from);
        let cache = ResponseCache::new(ttl, cache_path);
        let agent = ureq::Agent::new_with_defaults();

        Self {
            api_key,
            cache,
            agent,
        }
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    fn fetch(
        &self,
        endpoint: &str,
        filetype: &str,
        custom_key: Option<String>,
    ) -> Result<String, CongressError> {
        // Check cache first
        let key = match custom_key {
            Some(val) => val,
            None => endpoint.to_string(),
        };
        let key = format!("{}.{}", key, filetype);

        if let Some(cached) = self.cache.get(&key) {
            return Ok(cached);
        }

        let url = format!("{}/{}", BASE_URL, endpoint);

        let mut response = self
            .agent
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .call()
            .map_err(|e| match e {
                ureq::Error::StatusCode(429) => CongressError::RateLimited,
                ureq::Error::StatusCode(401) | ureq::Error::StatusCode(403) => {
                    CongressError::InvalidApiKey
                }
                ureq::Error::StatusCode(404) => CongressError::NotFound(endpoint.to_string()),
                _ => CongressError::Http(e.to_string()),
            })?;

        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| CongressError::Http(e.to_string()))?;

        // Cache successful response
        self.cache.set(&key, &body)?;

        Ok(body)
    }

    pub fn get_member(&self, bioguide_id: &str) -> Result<Member, CongressError> {
        let endpoint = format!("member/{}", bioguide_id);
        let json = self.fetch(&endpoint, "json", None)?;
        Member::from_api_response(&json)
    }

    pub fn get_bill_sponsors(
        &self,
        congress: u16,
        bill_type: &str,
        number: u32,
    ) -> Result<SponsorInfo, CongressError> {
        let bill_endpoint = format!("bill/{}/{}/{}", congress, bill_type, number);
        let bill_json = self.fetch(&bill_endpoint, "json", None)?;
        let bill: Value = serde_json::from_str(&bill_json)?;

        let bill_id = format!("{}-{}-{}", congress, bill_type, number);

        let sponsor = bill["bill"]["sponsors"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|s| s["bioguideId"].as_str())
            .unwrap_or("")
            .to_string();

        // Fetch cosponsors
        let cosponsor_endpoint = format!("{}/cosponsors", bill_endpoint);
        let cosponsor_json = self.fetch(&cosponsor_endpoint, "json", None)?;
        let cosponsor_data: Value = serde_json::from_str(&cosponsor_json)?;

        let mut cosponsors = Vec::new();
        if let Some(arr) = cosponsor_data["cosponsors"].as_array() {
            for c in arr {
                let bioguide = c["bioguideId"].as_str().unwrap_or("").to_string();
                let date = c["sponsorshipDate"].as_str().unwrap_or("").to_string();
                let withdrawn = c["sponsorshipWithdrawnDate"].as_str().is_some();

                cosponsors.push(CosponsorRecord {
                    bioguide_id: bioguide,
                    date,
                    withdrawn,
                });
            }
        }

        Ok(SponsorInfo {
            bill_id,
            sponsor,
            cosponsors,
        })
    }

    /// Download all data for a bill: XML, sponsors, votes, members
    ///
    /// bill_id format: (congress-type-number) "119-hr-1"
    pub fn download_bill(&self, bill_id: &str) -> Result<BillDownload, CongressError> {
        let (congress, bill_type, number) = Self::parse_bill_id(bill_id)?;

        // Fetch bill XML from api.congress.gov
        let bill_xml = self.fetch_bill_xml(congress, &bill_type, number)?;

        // Fetch bill metadata (includes sponsors, actions, committees, etc.)
        let bill_endpoint = format!("bill/{}/{}/{}", congress, bill_type, number);
        let bill_metadata_json = self.fetch(
            &bill_endpoint,
            "json",
            format!("bill/{}/{}/{}/metadata", congress, bill_type, number).into(),
        )?;

        // Fetch cosponsors
        let cosponsors_endpoint = format!("{}/cosponsors", bill_endpoint);
        let cosponsors_json = self.fetch(&cosponsors_endpoint, "json", None)?;

        // Collect member bioguide IDs from sponsors
        let mut member_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        if let Ok(v) = serde_json::from_str::<Value>(&bill_metadata_json)
            && let Some(sponsors) = v["bill"]["sponsors"].as_array()
        {
            for s in sponsors {
                if let Some(id) = s["bioguideId"].as_str() {
                    member_ids.insert(id.to_string());
                }
            }
        }

        if let Ok(v) = serde_json::from_str::<Value>(&cosponsors_json)
            && let Some(cosponsors) = v["cosponsors"].as_array()
        {
            for c in cosponsors {
                if let Some(id) = c["bioguideId"].as_str() {
                    member_ids.insert(id.to_string());
                }
            }
        }

        // Fetch House votes (dedupe by roll number)
        let votes_json = match self.get_bill_house_votes(congress, &bill_type, number) {
            Ok(refs) if !refs.is_empty() => {
                let mut seen_rolls = std::collections::HashSet::new();
                let mut roll_calls = Vec::new();
                for r in &refs {
                    if seen_rolls.insert(r.roll_number)
                        && let Ok(vote) = self.get_house_vote(r.congress, r.session, r.roll_number)
                    {
                        vote.member_votes.iter().for_each(|member_vote| {
                            member_ids.insert(member_vote.bioguide_id.clone());
                        });
                        roll_calls.push(vote);
                    }
                }
                if !roll_calls.is_empty() {
                    Some(serde_json::to_string(&roll_calls).unwrap_or_default())
                } else {
                    None
                }
            }
            _ => None,
        };

        // Fetch member details
        let mut member_jsons = HashMap::new();
        for id in member_ids {
            let endpoint = format!("member/{}", id);
            if let Ok(json) = self.fetch(&endpoint, "json", None) {
                member_jsons.insert(id, json);
            }
        }

        Ok(BillDownload {
            bill_id: bill_id.to_string(),
            bill_xml,
            bill_metadata_json,
            cosponsors_json,
            votes_json,
            member_jsons,
        })
    }

    /// Parse bill_id like "119-hr-1" into (congress, type, number)
    fn parse_bill_id(bill_id: &str) -> Result<(u16, String, u32), CongressError> {
        let parts: Vec<&str> = bill_id.split('-').collect();
        if parts.len() != 3 {
            return Err(CongressError::Parse(format!(
                "Invalid bill_id: {}",
                bill_id
            )));
        }

        let congress: u16 = parts[0]
            .parse()
            .map_err(|_| CongressError::Parse(format!("Invalid congress: {}", parts[0])))?;

        let bill_type = parts[1].to_string();

        let number: u32 = parts[2]
            .parse()
            .map_err(|_| CongressError::Parse(format!("Invalid number: {}", parts[2])))?;

        Ok((congress, bill_type, number))
    }

    /// Fetch bill actions and extract House roll call references
    pub fn get_bill_house_votes(
        &self,
        congress: u16,
        bill_type: &str,
        number: u32,
    ) -> Result<Vec<RecordedVoteRef>, CongressError> {
        let actions_endpoint = format!("bill/{}/{}/{}/actions", congress, bill_type, number);
        let actions_json = self.fetch(&actions_endpoint, "json", None)?;
        RecordedVoteRef::extract_house_votes_from_actions(&actions_json)
    }

    /// Fetch a House roll call vote with member votes
    pub fn get_house_vote(
        &self,
        congress: u16,
        session: u8,
        roll_number: u32,
    ) -> Result<HouseRollCall, CongressError> {
        let vote_endpoint = format!("house-vote/{}/{}/{}", congress, session, roll_number);
        let vote_json = self.fetch(
            &vote_endpoint,
            "json",
            format!(
                "house-vote/{}/{}/{}/roll_call",
                congress, session, roll_number
            )
            .into(),
        )?;

        let members_endpoint = format!("{}/members", vote_endpoint);
        let members_json = self.fetch(&members_endpoint, "json", None)?;

        HouseRollCall::from_api_response(&vote_json, &members_json)
    }

    /// Fetch bill text info from Congress API, then fetch XML from URL
    fn fetch_bill_xml(
        &self,
        congress: u16,
        bill_type: &str,
        number: u32,
    ) -> Result<String, CongressError> {
        let cache_key = format!("bill/{}/{}/{}/public_law.xml", congress, bill_type, number);

        if let Some(xml) = self.cache.get(&cache_key) {
            return Ok(xml);
        }

        // Get text versions list from Congress API
        let text_endpoint = format!("bill/{}/{}/{}/text", congress, bill_type, number);
        let text_json = self.fetch(&text_endpoint, "json", None)?;
        let text_data: Value = serde_json::from_str(&text_json)?;

        // Find XML URL from text versions (prefer enrolled, then engrossed, then introduced)
        let xml_url = text_data["textVersions"]
            .as_array()
            .and_then(|versions| {
                for v in versions {
                    if v["type"].as_str() == Some("Public Law")
                        && let Some(formats) = v["formats"].as_array()
                    {
                        for f in formats {
                            if f["type"].as_str() == Some("United States Legislative Markup") {
                                return f["url"].as_str().map(String::from);
                            }
                        }
                    }
                }
                None
            })
            .ok_or_else(|| {
                CongressError::NotFound(format!("No XML format for bill {}", text_endpoint))
            })?;
        // Fetch XML from URL
        let mut response = self
            .agent
            .get(&xml_url)
            .call()
            .map_err(|e| CongressError::Http(e.to_string()))?;

        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| CongressError::Http(e.to_string()))?;

        self.cache.set(&cache_key, &body)?;

        Ok(body)
    }
}
