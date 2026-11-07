//! What a reader says of an edition of the Code that a citation names (#414).
//!
//! `cases-citing`, `path` and `settle --explain` say it with these functions
//! alone, so the three commands use one wording.

use words_to_data::judicial::reliance::CitedEdition;

/// Said when the dataset holds no printing from the year of the edition cited.
pub const NUMBER_USED_AGAIN: &str = "A section number can be used again, so the provision now \
at that number can be a different provision from the one the court read.";

/// The opinion cites an edition: `the opinion cites the Code of 1986, written
/// "(1986)" after the citation`. The year comes first, and the words in the
/// brackets follow as the court wrote them, because they can be a year alone,
/// `1986`, and "the 1986 of the Code" is not English (#470). For title 26 a
/// year alone can name the Internal Revenue Code of that year, and "the Code
/// of 1986" stays true then too.
pub fn cites(edition: &CitedEdition) -> String {
    format!(
        "the opinion cites the Code of {}, written \"({})\" after the citation",
        edition.year, edition.written
    )
}

/// Whether this dataset holds a printing of `title` from the year of the
/// edition: `This dataset holds no printing of title 26 from 1994, so that law
/// is out of scope.`
pub fn held(edition: &CitedEdition, title: &str) -> String {
    let year = &edition.year;
    match edition.printings.as_slice() {
        [] => format!(
            "This dataset holds no printing of title {title} from {year}, so that law is \
             out of scope."
        ),
        dates => format!(
            "This dataset holds the printing(s) of {} from {year}.",
            dates.join(", ")
        ),
    }
}

/// The line `path` and `settle --explain` print for an edition a link names:
/// the edition, whether a printing from its year is held, and, when none is,
/// that the provision now at the number can be a different one.
pub fn warning(edition: &CitedEdition, cited_path: &str) -> String {
    let said = format!(
        "{}. {}",
        cites(edition),
        held(edition, title_of(cited_path))
    );
    match edition.printings.is_empty() {
        true => format!("{said} {NUMBER_USED_AGAIN}"),
        false => said,
    }
}

/// The number of the title a path of the Code is in: `26` for
/// `uscode/title_26/…`.
pub fn title_of(path: &str) -> &str {
    path.split('/')
        .nth(1)
        .and_then(|segment| segment.strip_prefix("title_"))
        .unwrap_or("?")
}
