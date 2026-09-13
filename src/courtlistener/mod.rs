//! Reading court opinions from CourtListener, and storing them as documents.
//!
//! CourtListener is Free Law Project's case-law database. This module reads two
//! of its records — an opinion and the cluster above it — and turns the pair into
//! an [`Expression`]: one work as it read on one date, with one node
//! (`docs/adr/0006-a-document-node-is-class-neutral.md`, #53).
//!
//! [`Expression`]: crate::dataset::Expression
//!
//! # An opinion is one node
//!
//! The whole text as the content of a single node at `judicial/opinion_<id>`. The
//! text has structure — `xml_harvard` carries `<opinion type="majority">` and
//! `<author>` — and nothing here parses it. The node gains children later and its
//! type does not change.
//!
//! # The filing date is on the cluster
//!
//! An opinion record carries `date_created` and `date_modified`, and neither is
//! the date the court decided anything: they say when CourtListener ingested and
//! last touched the record. The court's date is `date_filed` on the cluster,
//! which is a second request. [`opinion_expression`] takes both records for that
//! reason, and there is no single-record form of it to take by mistake.
//!
//! # Which field holds the text decides the answer
//!
//! A record carries the text in up to eight fields, from different donors, at
//! different quality. [`opinion::TextSource`] says which one was taken and what
//! that means for trusting it. The choice is not cosmetic: *Snow v.
//! Commissioner* writes its citation to 26 U.S.C. § 174 with the section sign as
//! the character reference `&#167;`, so reading a markup field without decoding
//! entities loses the citation and the query answers "this case does not cite
//! § 174". See [`markup`].
//!
//! # Attribution, and the limits
//!
//! The data is Free Law Project's. Nothing here is produced, endorsed or
//! verified by them, and their terms ask that this be said plainly rather than
//! implied away.
//!
//! The API allows 5 requests a minute, 50 an hour and 125 a day, on a rolling
//! window. [`CourtListenerClient`] therefore paces itself and caches every
//! response, and a second run of the same command spends nothing. The website
//! answers a plain client with HTTP 202 and an empty body, and the terms forbid
//! scraping it, so the API is the only door.

pub mod client;
pub mod markup;
pub mod opinion;

pub use client::CourtListenerClient;
pub use opinion::{ClusterRecord, OpinionRecord, TextSource, opinion_expression, work_id};

/// Something went wrong reading CourtListener.
#[derive(Debug, thiserror::Error)]
pub enum CourtListenerError {
    /// No token was given, or the one given was refused.
    #[error(
        "CourtListener refused the credentials. Set COURTLISTENER_API_KEY to a \
         token from https://www.courtlistener.com/profile/api-tokens/"
    )]
    Unauthorized,

    /// The rolling quota is spent: 5 a minute, 50 an hour, 125 a day.
    #[error(
        "CourtListener is rate limiting this token (5 requests/minute, \
         50/hour, 125/day, rolling). Wait rather than retry: every refused \
         request still counts."
    )]
    RateLimited,

    /// The record is not there.
    #[error("CourtListener holds no {kind} {id}")]
    NotFound { kind: &'static str, id: u64 },

    #[error("CourtListener request failed: {0}")]
    Http(String),

    #[error("CourtListener sent something this build cannot read: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Could not write the response cache: {0}")]
    Cache(#[from] std::io::Error),

    /// A request was needed and this client is not allowed to make one.
    ///
    /// What a test run gets, and what a run against committed fixtures gets. It
    /// is an error rather than an empty answer, because a silently missing
    /// opinion is a dataset that is quietly short of one.
    #[error(
        "{kind} {id} is not in the cache at {directory}, and this client is \
         offline. Fetch it with an API token first."
    )]
    Offline {
        kind: &'static str,
        id: u64,
        directory: String,
    },

    /// The record arrived, and it does not carry what an expression needs.
    #[error("CourtListener {kind} {id} carries no {field}")]
    Incomplete {
        kind: &'static str,
        id: u64,
        field: &'static str,
    },
}
