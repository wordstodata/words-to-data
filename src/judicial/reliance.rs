//! Which cases construing a provision are now unreliable, and how far we can say.
//!
//! This is statutory research run backwards. The usual question is "what
//! controls this point"; this one is "Congress has amended this provision — which
//! cases construing the old text can no longer be relied on?" It is the question
//! #53 is a pass condition for:
//!
//! > which of the ten opinions cite 26 USC 174, and did that provision change
//! > after the opinion was published?
//!
//! # Two answers, and neither may be "not found"
//!
//! The first half is a link query: every `judicial.cites` link whose object sits
//! at or under the provision asked about. The link carries the verification state
//! and the matched text as evidence, so a reader sees that a rule proposed it and
//! what the rule read ([`crate::citation`]).
//!
//! The second half is the part it is easy to get quietly wrong. A dataset holding
//! two release points of the U.S. Code can prove what changed **between those two
//! dates** and nothing else. *Snow v. Commissioner* was filed in 1974 and § 174
//! has been rewritten since, most recently in 2025; a dataset holding July 2025
//! can show the July change and knows nothing whatever about the fifty-one years
//! before it. Reporting only "changed: yes" would be true and misleading, and
//! reporting "no change found" for the earlier period would be a lie about the
//! law.
//!
//! So [`SincePublication`] reports both: the windows the dataset covers, each
//! with whether the provision changed in it, **and** the period between the
//! opinion and the first printing held, which is out of scope. A reader can then
//! tell "this provision did not change" from "we were not looking".

use crate::dataset::{Coverage, DatasetError, ExpressionId, Scope, WorkId};
use crate::diff::TreeDiff;
use crate::document::DocumentNode;
use crate::link::{Link, LinkKind, Target, VerificationState};
use crate::method::Method;
use crate::storage::{DocumentReader, LinkReader};
use crate::uslm::path::covers_path;

/// Whether the dataset holds the opinion a citation link came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Citing {
    /// The dataset holds it, as one expression of one work. Everything the
    /// report says about it can be checked against the text in the file.
    Held {
        expression: ExpressionId,
        case_name: Option<String>,
        /// Where the opinion's text came from, from the node's own provenance:
        /// the field it was taken from, how it was obtained, and how far it can
        /// be trusted. `None` means the producer recorded nothing.
        text_source: Option<String>,
        /// How it was obtained, and which version of that reading. The version
        /// tells a reader whether this build would read the same words again
        /// (`crate::method::Method`).
        text_method: Option<Method>,
        text_verification: Option<VerificationState>,
    },
    /// The link names an opinion outside the dataset. The citation is still
    /// reported, and nothing about the opinion's own text can be.
    NotHeld { reference: String, display: String },
}

impl Citing {
    /// How to name this opinion in a report.
    pub fn display(&self) -> String {
        match self {
            Self::Held {
                expression,
                case_name,
                ..
            } => match case_name {
                Some(name) => format!("{name} ({expression})"),
                None => expression.to_string(),
            },
            Self::NotHeld { display, .. } => display.clone(),
        }
    }

    /// The date the citing opinion is dated, when the dataset holds it.
    fn at(&self) -> Option<&str> {
        match self {
            Self::Held { expression, .. } => Some(&expression.at),
            Self::NotHeld { .. } => None,
        }
    }
}

/// Two printings of one work, and whether the provision differs between them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeWindow {
    pub from: String,
    pub to: String,
    /// True when the provision does not say the same thing on both dates.
    pub changed: bool,
    /// Every path at or under the provision whose text differs, in document
    /// order. Empty when nothing changed.
    pub changed_paths: Vec<String>,
}

/// A period the dataset says nothing about.
///
/// From the day the opinion was filed to the first printing of the cited work
/// that the dataset holds. The provision may have been rewritten ten times in it,
/// and this dataset cannot see any of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncoveredPeriod {
    pub from: String,
    pub to: String,
}

/// What this dataset can say about a provision since an opinion was filed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SincePublication {
    /// Whether the dataset carries the cited material at all.
    pub coverage: Coverage,
    /// Every consecutive pair of printings held whose later date falls after the
    /// opinion, oldest first.
    pub windows: Vec<ChangeWindow>,
    /// The period between the opinion and the first printing held, when there is
    /// one. `None` means the dataset holds a printing from on or before the day
    /// the opinion was filed, so nothing is missing at that end.
    pub uncovered: Option<UncoveredPeriod>,
}

impl SincePublication {
    /// Whether the provision changed in a window this dataset covers.
    ///
    /// `false` does not mean it did not change. Read [`SincePublication::uncovered`]
    /// before saying anything about that.
    pub fn changed_in_a_covered_window(&self) -> bool {
        self.windows.iter().any(|window| window.changed)
    }
}

/// One provision one opinion cites, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedProvision {
    /// Where the provision sits: a structural path, because a provision has no
    /// identity of its own yet (`docs/adr/0001-structural-paths-locate-not-identify.md`).
    pub path: String,
    /// The text the rule matched, from the link's evidence. This is what a
    /// reviewer checks the link against.
    pub citation_text: Option<String>,
    /// How far the *link* can be trusted — not the provision, and not the
    /// opinion's text.
    pub verification: VerificationState,
    pub since: SincePublication,
}

/// One opinion, and the provisions it cites that the caller asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitingCase {
    pub citing: Citing,
    /// In path order, so two runs report alike.
    pub cites: Vec<CitedProvision>,
}

/// Every opinion in this dataset that cites a provision at or under `path`.
///
/// In the order the opinions are named, so two runs agree. An opinion that cites
/// the provision twice is one entry with two [`CitedProvision`]s, because two
/// citations to one provision are two statements about one thing.
///
/// An empty answer means no `judicial.cites` link in the dataset names this
/// material. Whether that is because no case cites it or because the dataset
/// holds no opinions is a question for the scope, not for this list.
pub fn cases_citing<R: DocumentReader + LinkReader + ?Sized>(
    reader: &R,
    path: &str,
) -> Result<Vec<CitingCase>, DatasetError> {
    let scope = Scope::derive(reader)?;
    let links = reader.links_by_kind(LinkKind::CITES)?;

    let mut cases: Vec<CitingCase> = Vec::new();
    for link in links {
        let Some(cited_path) = provision_at_or_under(&link.object, path) else {
            continue;
        };

        let citing = citing_of(reader, &link)?;
        let cited = CitedProvision {
            since: since_publication(reader, &scope, &cited_path, citing.at())?,
            path: cited_path,
            citation_text: link
                .provenance
                .evidence
                .as_ref()
                .and_then(|evidence| evidence.reasoning.clone()),
            verification: link.provenance.verification,
        };

        match cases.iter_mut().find(|case| case.citing == citing) {
            Some(case) => case.cites.push(cited),
            None => cases.push(CitingCase {
                citing,
                cites: vec![cited],
            }),
        }
    }

    for case in &mut cases {
        case.cites.sort_by(|a, b| a.path.cmp(&b.path));
    }
    Ok(cases)
}

/// The path a link's object names, when it sits at or under `wanted`.
///
/// A citation to `26 U.S.C. § 174` resolves to the section, and a caller asking
/// about the section means the whole of it, so a link into a subsection counts.
fn provision_at_or_under(object: &Target, wanted: &str) -> Option<String> {
    match object {
        Target::Provision(path) if covers_path(wanted, path) => Some(path.clone()),
        _ => None,
    }
}

/// Who the citing opinion is, and whether this dataset holds it.
fn citing_of<R: DocumentReader + ?Sized>(reader: &R, link: &Link) -> Result<Citing, DatasetError> {
    let path = match &link.subject {
        Target::Provision(path) => path,
        Target::External { reference, display } => {
            return Ok(Citing::NotHeld {
                reference: reference.clone(),
                display: display.clone(),
            });
        }
        // A citation's subject is the writing that made it. Neither of the two
        // remaining shapes can be one: an expression is a whole printing and a
        // change is a provision across two dates.
        other => {
            return Ok(Citing::NotHeld {
                reference: format!("{other:?}"),
                display: "a citing document this build cannot name".to_string(),
            });
        }
    };

    let found = reader.find_nodes(path)?;
    let Some((expression, node)) = found.into_iter().next() else {
        return Ok(Citing::NotHeld {
            reference: path.clone(),
            display: format!("an opinion at {path}, which this dataset does not hold"),
        });
    };

    let facts = super::OpinionFacts::of(&node.data);
    let provenance = node.data.provenance.as_ref();
    Ok(Citing::Held {
        expression,
        case_name: facts
            .map(|facts| facts.case_name)
            .or_else(|| node.data.heading.as_deref().map(str::to_string)),
        text_source: provenance.map(|provenance| provenance.source.clone()),
        text_method: provenance.and_then(|provenance| provenance.method.clone()),
        text_verification: provenance.map(|provenance| provenance.verification),
    })
}

/// What the dataset can say about `path` since `opinion_date`.
///
/// `opinion_date` is `None` when the dataset does not hold the citing opinion.
/// Then every window held is reported, and none is left out for being too early,
/// because there is no date to measure "too early" against.
fn since_publication<R: DocumentReader + ?Sized>(
    reader: &R,
    scope: &Scope,
    path: &str,
    opinion_date: Option<&str>,
) -> Result<SincePublication, DatasetError> {
    let coverage = scope.covers(path);
    let Some(work) = work_holding(scope, path) else {
        return Ok(SincePublication {
            coverage,
            windows: Vec::new(),
            uncovered: None,
        });
    };

    let mut dates: Vec<String> = reader
        .expressions(&work)?
        .into_iter()
        .map(|info| info.id.at)
        .collect();
    dates.sort();

    // Read the provision once per printing, from the index, rather than diffing
    // the whole work. A title of the U.S. Code holds tens of thousands of nodes
    // and the question is about one of them.
    let held: Vec<(ExpressionId, DocumentNode)> = reader.find_nodes(path)?;

    let mut windows = Vec::new();
    for pair in dates.windows(2) {
        let (from, to) = (&pair[0], &pair[1]);
        // A window that closes before the opinion was filed says nothing about
        // what happened after it.
        if opinion_date.is_some_and(|filed| to.as_str() <= filed) {
            continue;
        }
        let changed_paths = changed_between(&held, &work, from, to);
        windows.push(ChangeWindow {
            from: from.clone(),
            to: to.clone(),
            changed: !changed_paths.is_empty(),
            changed_paths,
        });
    }

    let uncovered = opinion_date.zip(dates.first()).and_then(|(filed, first)| {
        (first.as_str() > filed).then(|| UncoveredPeriod {
            from: filed.to_string(),
            to: first.clone(),
        })
    });

    Ok(SincePublication {
        coverage,
        windows,
        uncovered,
    })
}

/// The work that holds `path`, out of the works the dataset carries.
///
/// Read from the scope rather than built from the path, so this stays
/// class-neutral: `uscode/title_26/.../section_174` belongs to
/// `uscode/title_26` and `judicial/opinion_109019` is its own work, and nothing
/// here needs to know which rule made either.
fn work_holding(scope: &Scope, path: &str) -> Option<WorkId> {
    scope
        .works()
        .find(|work| covers_path(work.as_str(), path))
        .cloned()
}

/// Every path at or under `path` whose text differs between the two printings.
///
/// The date is deliberately not compared: two printings of one provision always
/// differ in it, and comparing whole nodes would report every provision as
/// changed. [`TreeDiff`] compares the five text fields and the children, which
/// is the codebase's one definition of "this provision changed".
fn changed_between(
    held: &[(ExpressionId, DocumentNode)],
    work: &WorkId,
    from: &str,
    to: &str,
) -> Vec<String> {
    let at = |date: &str| {
        held.iter()
            .find(|(id, _)| &id.work == work && id.at == date)
            .map(|(_, node)| node)
    };
    let (Some(before), Some(after)) = (at(from), at(to)) else {
        // The provision is not at this path on both dates. Added or removed
        // rather than changed, and neither is this function's question.
        return Vec::new();
    };

    let mut paths = Vec::new();
    collect_changed(&TreeDiff::from_nodes(before, after), &mut paths);
    paths
}

/// The paths a diff records a text change at, root first.
fn collect_changed(diff: &TreeDiff, into: &mut Vec<String>) {
    if !diff.changes.is_empty() {
        into.push(diff.root_path.clone());
    }
    for added in &diff.added {
        into.push(added.path.to_string());
    }
    for removed in &diff.removed {
        into.push(removed.path.to_string());
    }
    for child in &diff.child_diffs {
        collect_changed(child, into);
    }
}
