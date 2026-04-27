use std::path::PathBuf;
use std::time::Duration;

use super::{
    BillDownload, Chamber, CongressError, CosponsorRecord, Member, ResponseCache, RollCall,
    SponsorInfo, VotePosition, VoteResult,
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
    pub fn new(api_key: String, cache_dir: PathBuf) -> Self {
        let cache = ResponseCache::new(cache_dir, Duration::from_secs(DEFAULT_TTL_SECS));
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

    fn fetch(&self, endpoint: &str, filetype: &str) -> Result<String, CongressError> {
        // Check cache first
        let key = format!("{}.{}", endpoint, filetype);
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
        let json = self.fetch(&endpoint, "json")?;
        Member::from_api_response(&json)
    }

    pub fn get_bill_sponsors(
        &self,
        congress: u16,
        bill_type: &str,
        number: u32,
    ) -> Result<SponsorInfo, CongressError> {
        let bill_endpoint = format!("bill/{}/{}/{}", congress, bill_type, number);
        let bill_json = self.fetch(&bill_endpoint, "json")?;
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
        let cosponsor_json = self.fetch(&cosponsor_endpoint, "json")?;
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

    pub fn get_roll_call(
        &self,
        congress: u16,
        chamber: Chamber,
        session: u8,
        roll_number: u32,
    ) -> Result<RollCall, CongressError> {
        let chamber_str = match chamber {
            Chamber::Senate => "senate",
            Chamber::House => "house",
        };

        // Note: Congress.gov API only has vote data for 118th Congress onwards
        let endpoint = format!(
            "vote/{}/{}/{}/{}",
            congress, chamber_str, session, roll_number
        );
        let json = self.fetch(&endpoint, "json")?;
        let v: Value = serde_json::from_str(&json)?;

        let vote = &v["vote"];

        let date = vote["date"].as_str().unwrap_or("").to_string();

        let bill_id = vote["bill"].as_object().map(|b| {
            let congress = b.get("congress").and_then(|v| v.as_u64()).unwrap_or(0);
            let bill_type = b.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let number = b.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
            format!("{}-{}-{}", congress, bill_type, number)
        });

        let result_str = vote["result"].as_str().unwrap_or("");
        let result = match result_str.to_lowercase().as_str() {
            s if s.contains("passed") || s.contains("agreed") => VoteResult::Passed,
            s if s.contains("failed") || s.contains("rejected") => VoteResult::Failed,
            _ => VoteResult::Unknown,
        };

        let mut votes = HashMap::new();
        if let Some(positions) = vote["positions"].as_array() {
            for pos in positions {
                let member_id = pos["member"]["bioguideId"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let vote_cast = pos["votePosition"]
                    .as_str()
                    .unwrap_or("Not Voting")
                    .parse::<VotePosition>()
                    .unwrap_or(VotePosition::NotVoting);

                if !member_id.is_empty() {
                    votes.insert(member_id, vote_cast);
                }
            }
        }

        Ok(RollCall {
            congress,
            session,
            roll_number,
            chamber,
            date,
            bill_id,
            result,
            votes,
        })
    }

    /// Download all data for a bill: XML, sponsors, votes, members
    ///
    /// bill_id format: (congress-type-number) "119-hr-1"
    pub fn download_bill(&self, bill_id: &str) -> Result<BillDownload, CongressError> {
        let (congress, bill_type, number) = Self::parse_bill_id(bill_id)?;

        // Fetch bill XML from api.congress.gov
        let bill_xml = self.fetch_bill_xml(congress, &bill_type, number)?;

        // Fetch sponsors
        let bill_endpoint = format!("bill/{}/{}/{}", congress, bill_type, number);
        let sponsors_json = self.fetch(&bill_endpoint, "json")?;

        // Fetch cosponsors
        let cosponsors_endpoint = format!("{}/cosponsors", bill_endpoint);
        let cosponsors_json = self.fetch(&cosponsors_endpoint, "json")?;

        // Collect member bioguide IDs from sponsors
        let mut member_ids: Vec<String> = Vec::new();
        if let Ok(v) = serde_json::from_str::<Value>(&sponsors_json)
            && let Some(sponsors) = v["bill"]["sponsors"].as_array()
        {
            for s in sponsors {
                if let Some(id) = s["bioguideId"].as_str() {
                    member_ids.push(id.to_string());
                }
            }
        }

        if let Ok(v) = serde_json::from_str::<Value>(&cosponsors_json)
            && let Some(cosponsors) = v["cosponsors"].as_array()
        {
            for c in cosponsors {
                if let Some(id) = c["bioguideId"].as_str() {
                    member_ids.push(id.to_string());
                }
            }
        }

        // Fetch member details
        let mut member_jsons = HashMap::new();
        for id in member_ids {
            let endpoint = format!("member/{}", id);
            if let Ok(json) = self.fetch(&endpoint, "json") {
                member_jsons.insert(id, json);
            }
        }

        // TODO: Fetch votes for bill (requires finding roll call numbers from actions)
        let votes_json = None;

        Ok(BillDownload {
            bill_id: bill_id.to_string(),
            bill_xml,
            sponsors_json,
            cosponsors_json,
            votes_json,
            member_jsons,
        })
    }

    /// Parse bill_id like "119-pl-21" into (congress, type, number)
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

    /// Fetch bill text info from Congress API, then fetch XML from URL
    fn fetch_bill_xml(
        &self,
        congress: u16,
        bill_type: &str,
        number: u32,
    ) -> Result<String, CongressError> {
        let cache_key = format!("bill_{}_{}_{}.xml", congress, bill_type, number);

        if let Some(xml) = self.cache.get(&cache_key) {
            return Ok(xml);
        }

        // Get text versions list from Congress API
        let text_endpoint = format!("bill/{}/{}/{}/text", congress, bill_type, number);
        let text_json = self.fetch(&text_endpoint, "json")?;
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
                CongressError::NotFound(format!("No XML format for bill {}", &text_endpoint))
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
