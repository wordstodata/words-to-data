//! Reading court opinions from CourtListener, and storing them as documents.
//!
//! CourtListener is Free Law Project's case-law database. This module reads two
//! of its records — a cluster, which is the case, and the opinion records under
//! it — and turns them into an [`Expression`]: one case as it read on one date
//! (`docs/adr/0006-a-document-node-is-class-neutral.md`, #53).
//!
//! [`Expression`]: crate::dataset::Expression
//!
//! # A case is a work, and each writing is a child
//!
//! The case is `judicial/case_<cluster>`, and each writing is one node under it,
//! `judicial/case_<cluster>/opinion_<id>`. When
//! the publisher holds both a combined record of the case and split records of
//! its writings, the split records are the text and the combined record is only
//! a source (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decisions 1 and 2).
//!
//! # Inside a writing, the reporter's pages
//!
//! A writing read from `xml_harvard` or `html_columbia` holds its words in its
//! paragraphs and footnotes: `opinion_<id>/page_<p>/paragraph_<n>` by the page of
//! the official reporter each paragraph starts on, and `opinion_<id>/footnote_<label>`
//! (decisions 3 and 4, #314). A writing read from any other field is one node
//! that holds all its words. See [`opinion`] and the `structure` reader.
//!
//! # The filing date is on the cluster
//!
//! An opinion record carries `date_created` and `date_modified`, and neither is
//! the date the court decided anything: they say when CourtListener ingested and
//! last touched the record. The court's date is `date_filed` on the cluster,
//! which is a second request. [`case_expression`] takes both kinds of record for
//! that reason, and there is no single-record form of it to take by mistake.
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
mod structure;

pub use client::CourtListenerClient;
pub use opinion::{
    CaseExpression, ClusterRecord, OpinionRecord, TextSource, WritingText, case_expression,
    work_id, writing_path,
};

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
