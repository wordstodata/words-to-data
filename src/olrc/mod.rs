//! The Office of Law Revision Counsel's classification tables (#247).
//!
//! The OLRC publishes, for each session of Congress, which section of the U.S.
//! Code each section of a public law was classified to, and the kind of change:
//! <https://usc-cdn.house.gov/classification/tables.shtml>. A classification is
//! a statement by an authority, so it is stored as a **record**
//! (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//!
//! # The shape of a page
//!
//! HTML only. The table is one `<PRE>` block of fixed-width text, with a header
//! and a line of dashes above the first row:
//!
//! ```text
//!  U. S. Code
//! Title Section      Description      Pub. L.  Sec.                  139 Stat.
//! -------------      -----------      -------------                  ---------
//! 26    36B                           119-21   71301(a), (b)            321
//! ```
//!
//! The Statutes page is a link in the page (`<a href="/statviewer.htm?…">321</a>`)
//! and plain text when a row names two pages (`43, 44`). The column of the
//! law's sections can run into it with no space between:
//! `7201(c)(1) "Subchapter I"<a href=…>`.
//!
//! # The two sort orders hold the same rows
//!
//! Each session is published twice: in public law order (`tbl119pl_1st.htm`) and
//! in Code order (`tbl119cd_1st.htm`). They were checked against each other once,
//! on 2026-09-27, for the 119th Congress, 1st session, both pages "prepared"
//! January 23, 2026. Each holds 3,049 rows, and read as a multiset of
//! (title, section, description, public law, sections of the law, Statutes page)
//! the two are equal: no row is in one and not the other, and neither page
//! repeats a row. Only the order and the words of the header differ. So one
//! order is read, the public law order, because it groups the rows of a law
//! together.
//!
//! # The Description column
//!
//! The page's own legend gives the words, and a description is built from them:
//!
//! | word | meaning, per the legend |
//! | --- | --- |
//! | *(blank)* | the section is amended |
//! | `nt` | note. Alone, the note is amended |
//! | `nt [tbl]` | note [table] |
//! | `prec` | preceding: what stands before the section, such as the heading of the part or chapter it opens |
//! | `new` | a new section or a new note |
//! | `gen amd` | the section or note is generally amended |
//! | `omitted` | the section is omitted |
//! | `repealed` | the section or note is repealed |
//! | `ed chg` | an editorial classification change, which the OLRC lists in its Editorial Classification Change Table |
//! | `tr to T/S` | transferred to section S of title T |
//! | `tr fr T/S` | transferred from section S of title T |
//!
//! `nts` is not in the legend. It is read as the plural of `nt`: notes.
//!
//! Every description the 119th Congress, 1st session table holds, with its
//! count out of 3,049 rows:
//!
//! | description | rows | of them, Pub. L. 119-21 |
//! | --- | --- | --- |
//! | *(blank)* | 1,278 | 364 |
//! | `nt new` | 582 | 175 |
//! | `new` | 367 | 45 |
//! | `prec` | 169 | 27 |
//! | `repealed` | 159 | 2 |
//! | `nt` | 144 | 10 |
//! | `nt repealed` | 73 | |
//! | `nt prec new` | 54 | |
//! | `tr to T/S` | 43 | 3 |
//! | `tr fr T/S` | 43 | 3 |
//! | `prec new` | 39 | 3 |
//! | `nt prec repealed` | 25 | |
//! | `gen amd` | 21 | 2 |
//! | `nt prec` | 21 | |
//! | `nt ed chg` | 11 | |
//! | `nt [tbl]` | 7 | 1 |
//! | `nts repealed` | 4 | |
//! | `nts ed chg` | 3 | |
//! | `prec repealed` | 1 | |
//! | `nt omitted` | 1 | |
//! | `nt gen amd` | 1 | |
//! | `nt  repealed` | 1 | |
//! | `nt prec ed chg` | 1 | |
//! | `nts prec ed chg` | 1 | |
//!
//! `nt  repealed` is written with two spaces, as the OLRC printed it. A
//! description is stored as the table writes it, and never read into another
//! form, because it is the record.
//!
//! # What a row becomes
//!
//! One `olrc.classified_from` link for each section of the law the row names
//! ([`ClassificationRow::named_law_sections`]):
//!
//! - **subject:** the Code section, as the structural path the dataset holds it
//!   at. A section the dataset holds at two paths, over the dates it holds, is
//!   two links.
//! - **object:** `olrc.classification:<public law>:<section of the law>`, such
//!   as `olrc.classification:119-21:71301(a)` ([`classification_reference`]).
//! - **provenance:** `Asserted`, with the source `olrc:<the page's URL>`.
//! - **payload:** `{"descriptions": [...]}` in the `olrc` namespace. A list,
//!   because two rows can state one link: 70116(a)(2) of Pub. L. 119-21 both
//!   amended 26 U.S.C. 25B and added a note to it ([`classify`]).
//!
//! A note, and a heading that precedes a section, are recorded against the
//! section the row names: the table places them there and no lower, and the
//! description says which it is.
//!
//! A row that names a section the dataset cannot address states no link and is
//! reported with a [`SkipReason`]: the title is not held (out of scope, never
//! "not found"), the title was declared and is missing, or the title holds no
//! section with that number at any date the dataset holds. A structural path
//! carries the chapters and parts between a title and a section, so it cannot
//! be made up for a section the dataset does not hold.
//!
//! # The limits
//!
//! **A table resolves to a section, not below it** (#126). A row says `26 36B`,
//! not which subsection moved.
//!
//! **Absence from a table is not absence of a change.** In the OLRC's words,
//! "the tables only include those provisions of law that have been classified
//! to the Code".
//!
//! **Only the 119th Congress and later.** Earlier tables are published elsewhere
//! and are out of scope for #247.

use crate::dataset::DatasetError;
use crate::storage::DocumentReader;
use crate::uslm::ElementType;

pub mod client;
pub mod links;
pub mod table;

pub use client::{OlrcClient, TablePage};
pub use links::{Classified, SkipReason, Skipped, classification_reference, classify};
pub use table::{ClassificationRow, ClassificationTable};

/// Something went wrong reading a classification table.
#[derive(Debug, thiserror::Error)]
pub enum OlrcError {
    /// The page does not hold a table in the shape this reader knows.
    #[error("the page is not a classification table this reader knows: {0}")]
    Shape(String),
    /// A Congress has two sessions, and a table is published for each.
    #[error("there is no table for session {0}; a Congress has sessions 1 and 2")]
    Session(u32),
    /// An offline client was asked for a page the cache does not hold.
    #[error("{page} is not in the cache at {directory}, and this run may not fetch it")]
    Offline { page: String, directory: String },
    /// The request failed.
    #[error("fetching a classification table failed: {0}")]
    Http(String),
    /// The cache could not be written.
    #[error("the cache could not be written: {0}")]
    Io(#[from] std::io::Error),
}

/// The number of every public law a dataset holds, such as `119-21`.
///
/// A public law is stored as a work named by its number,
/// `publiclawdocument_119-21` (#196), and that number is the join key into a
/// classification table. Only work names are read.
pub fn held_public_laws<R: DocumentReader + ?Sized>(
    reader: &R,
) -> Result<Vec<String>, DatasetError> {
    let prefix = format!("{}_", ElementType::PublicLawDocument.path_segment_name());
    Ok(reader
        .works()?
        .iter()
        .filter_map(|work| work.as_str().strip_prefix(&prefix).map(str::to_string))
        .collect())
}
