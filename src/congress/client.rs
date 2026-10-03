use std::time::Duration;

use super::{
    BillDownload, CongressError, CosponsorRecord, HouseRollCall, Member, RecordedVoteRef,
    ResponseCache, SponsorInfo,
};
use rayon::prelude::*;
use serde_json::Value;
use std::collections::HashMap;

use crate::progress::{Progress, Silent};

const BASE_URL: &str = "https://api.congress.gov/v3";
const DEFAULT_TTL_SECS: u64 = 24 * 60 * 60; // 24 hours

/// How many member requests a bill download sends at once. A House roll call
/// names approximately 430 members, and the API allows 5,000 requests an hour,
/// so a small pool makes a download fast and keeps a run inside the limit.
const MEMBER_FETCHES_AT_ONCE: usize = 8;

pub struct CongressClient {
    api_key: String,
    cache: ResponseCache,
    http: crate::http::Http,
    /// Where requests go. [`BASE_URL`] unless a caller set another with
    /// [`CongressClient::with_base_url`].
    base_url: String,
    /// Read only the cache, and never reach the network. See
    /// [`CongressClient::cached_only`].
    offline: bool,
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
        Self {
            api_key,
            cache,
            http: crate::http::Http::new(),
            base_url: BASE_URL.to_string(),
            offline: false,
        }
    }

    /// Send requests to another server, such as a local one in a test.
    pub fn with_base_url(self, base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            ..self
        }
    }

    /// Build a client that reads only the cache and never reaches the network.
    ///
    /// It needs no API key. An entry the cache has not got is an error that
    /// names the file, [`CongressError::NotCached`], and not a request. A
    /// cached entry never expires here, because the network cannot replace it.
    pub fn cached_only(cache_dir: Option<String>) -> Self {
        Self {
            offline: true,
            ..Self::with_ttl(String::new(), cache_dir, None)
        }
    }

    /// The error for an entry an offline client has not got.
    fn not_cached(&self, key: &str) -> CongressError {
        CongressError::NotCached(self.cache.directory().join(key).display().to_string())
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
        if self.offline {
            return Err(self.not_cached(&key));
        }

        let url = format!("{}/{}", self.base_url, endpoint);

        let mut response = self
            .http
            .call(|agent| agent.get(&url).header("X-Api-Key", &self.api_key).call())
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
        self.download_bill_reporting(bill_id, &Silent)
    }

    /// [`Self::download_bill`], saying each step to `progress` as it goes.
    ///
    /// Most of the time goes to members. A bill with a roll call names every
    /// member who voted, about 432 for a House vote, and each is its own
    /// request. The cache is shared across bills, so the first bill of a
    /// Congress is slow and the next ones are fast (#124). The report says how
    /// many members are cached before the fetching starts, which is what tells
    /// a person the wait is the roster and not the bill.
    ///
    /// A roll call or member the API does not answer for is left out, and
    /// `progress` hears about it by name. A record an offline client has not
    /// got stops the download.
    pub fn download_bill_reporting(
        &self,
        bill_id: &str,
        progress: &dyn Progress,
    ) -> Result<BillDownload, CongressError> {
        let (congress, bill_type, number) = Self::parse_bill_id(bill_id)?;

        progress.begin("text of the law", None);
        let bill_xml = self.fetch_bill_xml(congress, &bill_type, number)?;

        // Bill metadata (sponsors, actions, committees, etc.) and cosponsors.
        progress.begin("sponsors", None);
        let bill_endpoint = format!("bill/{}/{}/{}", congress, bill_type, number);
        let bill_metadata_json = self.fetch(
            &bill_endpoint,
            "json",
            format!("bill/{}/{}/{}/metadata", congress, bill_type, number).into(),
        )?;
        let cosponsors_endpoint = format!("{}/cosponsors", bill_endpoint);
        let cosponsors_json = self.fetch(&cosponsors_endpoint, "json", None)?;

        let mut member_ids = sponsor_ids(&bill_metadata_json, &cosponsors_json);

        // House votes, deduped by roll number.
        progress.begin("roll calls", None);
        let refs = match self.get_bill_house_votes(congress, &bill_type, number) {
            Ok(refs) => refs,
            Err(error @ CongressError::NotCached(_)) => return Err(error),
            Err(error) => {
                progress.note(&format!(
                    "The votes on {bill_id} could not be listed, so none are held: {error}"
                ));
                Vec::new()
            }
        };
        let mut seen_rolls = std::collections::HashSet::new();
        let mut roll_calls = Vec::new();
        for r in &refs {
            if !seen_rolls.insert(r.roll_number) {
                continue;
            }
            match self.get_house_vote(r.congress, r.session, r.roll_number) {
                Ok(vote) => {
                    vote.member_votes.iter().for_each(|member_vote| {
                        member_ids.insert(member_vote.bioguide_id.clone());
                    });
                    roll_calls.push(vote);
                }
                Err(error @ CongressError::NotCached(_)) => return Err(error),
                Err(error) => progress.note(&format!(
                    "Roll call {} of {bill_id} could not be fetched and is left out: {error}",
                    r.roll_number
                )),
            }
        }
        let votes_json = if roll_calls.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&roll_calls).unwrap_or_default())
        };

        let member_jsons = self.fetch_members(member_ids, progress)?;

        Ok(BillDownload {
            bill_id: bill_id.to_string(),
            bill_xml,
            bill_metadata_json,
            cosponsors_json,
            votes_json,
            member_jsons,
        })
    }

    /// Fetch the member record for each id, several at once (#284).
    ///
    /// The label of the step says the split, "432 members, 411 cached", so a
    /// person can see why one bill takes minutes and the next takes seconds.
    /// The first error that stops the download ends the fetch: no new request
    /// starts after it.
    fn fetch_members(
        &self,
        ids: std::collections::HashSet<String>,
        progress: &dyn Progress,
    ) -> Result<HashMap<String, String>, CongressError> {
        let cached = ids
            .iter()
            .filter(|id| self.cache.holds(&format!("{}.json", member_endpoint(id))))
            .count();
        progress.begin(
            &format!("{} members, {cached} cached", ids.len()),
            Some(ids.len() as u64),
        );

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(MEMBER_FETCHES_AT_ONCE)
            .build()
            .map_err(|e| CongressError::Io(std::io::Error::other(e)))?;
        let members: Vec<Option<(String, String)>> = pool.install(|| {
            ids.into_par_iter()
                .map(|id| {
                    let member = self.member_json(id, progress);
                    progress.advance(1);
                    member
                })
                .collect::<Result<_, _>>()
        })?;
        Ok(members.into_iter().flatten().collect())
    }

    /// One member's response, keyed by the member's bioguide ID.
    ///
    /// A member the API does not answer for is `None`, and is left out, and
    /// `progress` hears about it by name. A member an offline client has not
    /// got is a gap in the cache, and a member refused by the rate limit is a
    /// gap in the data, so each of those is an error that stops the download
    /// (#284).
    fn member_json(
        &self,
        id: String,
        progress: &dyn Progress,
    ) -> Result<Option<(String, String)>, CongressError> {
        match self.fetch(&member_endpoint(&id), "json", None) {
            Ok(json) => Ok(Some((id, json))),
            Err(error @ (CongressError::NotCached(_) | CongressError::RateLimited)) => Err(error),
            Err(error) => {
                progress.note(&format!(
                    "Member {id} could not be fetched and is left out: {error}"
                ));
                Ok(None)
            }
        }
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
        if self.offline {
            return Err(self.not_cached(&cache_key));
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
            .http
            .call(|agent| agent.get(&xml_url).call())
            .map_err(|e| CongressError::Http(e.to_string()))?;

        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| CongressError::Http(e.to_string()))?;

        self.cache.set(&cache_key, &body)?;

        Ok(body)
    }
}

/// The endpoint, and the cache key less its extension, of one member's record.
fn member_endpoint(id: &str) -> String {
    format!("member/{id}")
}

/// The bioguide id of the sponsor and of each cosponsor of a bill.
fn sponsor_ids(
    bill_metadata_json: &str,
    cosponsors_json: &str,
) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    if let Ok(v) = serde_json::from_str::<Value>(bill_metadata_json)
        && let Some(sponsors) = v["bill"]["sponsors"].as_array()
    {
        ids.extend(
            sponsors
                .iter()
                .filter_map(|s| s["bioguideId"].as_str())
                .map(String::from),
        );
    }
    if let Ok(v) = serde_json::from_str::<Value>(cosponsors_json)
        && let Some(cosponsors) = v["cosponsors"].as_array()
    {
        ids.extend(
            cosponsors
                .iter()
                .filter_map(|c| c["bioguideId"].as_str())
                .map(String::from),
        );
    }
    ids
}
