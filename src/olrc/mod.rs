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
//! and plain text when a row names two pages (`43, 44`).

use std::sync::LazyLock;

use regex::Regex;

/// Something went wrong reading a classification table.
#[derive(Debug, thiserror::Error)]
pub enum OlrcError {
    /// The page does not hold a table in the shape this reader knows.
    #[error("the page is not a classification table this reader knows: {0}")]
    Shape(String),
}

/// One row of a classification table, as the table writes it.
///
/// Every field is the text of its column with the padding removed. Nothing is
/// read into another form here, because the row is the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassificationRow {
    /// The title of the Code: `26`.
    pub title: String,
    /// The section of that title: `36B`.
    pub section: String,
    /// The kind of change, in the table's own words: `nt new`. Empty when the
    /// section is amended.
    pub description: String,
    /// The public law: `119-21`.
    pub public_law: String,
    /// The sections of the law the row names, as written: `71301(a), (b)`.
    pub law_sections: String,
    /// The page of the Statutes at Large: `321`, or `43, 44`.
    pub statutes_page: String,
}

/// One page of classifications: every row, in the order the page lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassificationTable {
    pub rows: Vec<ClassificationRow>,
}

/// Where each column of a row starts, in characters, up to the law's sections.
///
/// The page is fixed-width text, and a value can hold spaces (`nt new`,
/// `71301(a), (b)`), so a row is cut at these offsets and never split on
/// whitespace.
const COLUMNS: [usize; 5] = [0, 6, 19, 36, 45];

/// Where the Statutes page starts, when nothing before it runs long.
///
/// The law's sections can run past it: `7201(c)(1) "Subchapter I"` meets the
/// link to the Statutes page with no space between. So when a row carries the
/// link, the link is where the page starts, and this offset is used only for a
/// row that names two pages as plain text (`43, 44`).
const STATUTES_PAGE: usize = 70;

/// Any HTML tag, such as the link around a Statutes page.
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").expect("a valid pattern"));

impl ClassificationTable {
    /// Read every row out of one page of the OLRC's HTML.
    ///
    /// A page in another shape is refused rather than read as fewer rows: a
    /// column that moved would put a section number in the description, and no
    /// count would show it.
    pub fn parse(html: &str) -> Result<Self, OlrcError> {
        let start = html
            .find("<PRE>")
            .ok_or_else(|| OlrcError::Shape("no <PRE> block".to_string()))?;
        let end = html[start..]
            .find("</pre>")
            .map(|offset| start + offset)
            .ok_or_else(|| OlrcError::Shape("the <PRE> block does not close".to_string()))?;
        let block = &html[start..end];

        let mut lines = block.lines();
        lines
            .by_ref()
            .find(|line| line.starts_with("-------------"))
            .ok_or_else(|| OlrcError::Shape("no line of dashes above the rows".to_string()))?;

        let rows = lines
            .filter(|line| !line.trim().is_empty())
            .map(parse_row)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { rows })
    }
}

/// Cut one line of the table into its columns.
fn parse_row(line: &str) -> Result<ClassificationRow, OlrcError> {
    let chars: Vec<char> = line.chars().collect();

    // The character before each column is padding. A value that runs into it
    // means the columns are not where this reader expects them.
    for &start in &COLUMNS[1..] {
        if chars.get(start - 1).is_some_and(|c| *c != ' ') {
            return Err(OlrcError::Shape(format!(
                "a value crosses a column boundary: {line}"
            )));
        }
    }

    let cut = |from: usize, to: usize| -> String {
        let from = from.min(chars.len());
        let to = to.min(chars.len());
        chars[from..to].iter().collect::<String>().trim().to_string()
    };

    let tail: String = chars[COLUMNS[4].min(chars.len())..].iter().collect();
    let (law_sections, statutes_page) = match tail.find("<a") {
        Some(link) => (
            tail[..link].trim().to_string(),
            TAG.replace_all(&tail[link..], "").trim().to_string(),
        ),
        None => (
            cut(COLUMNS[4], STATUTES_PAGE),
            cut(STATUTES_PAGE, chars.len()),
        ),
    };

    Ok(ClassificationRow {
        title: cut(COLUMNS[0], COLUMNS[1]),
        section: cut(COLUMNS[1], COLUMNS[2]),
        description: cut(COLUMNS[2], COLUMNS[3]),
        public_law: cut(COLUMNS[3], COLUMNS[4]),
        law_sections,
        statutes_page,
    })
}
