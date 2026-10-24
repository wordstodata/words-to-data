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
//!
//! Two more lines are true and easy to misread (#340). A window that starts
//! before the opinion and ends after it holds the decision date, so a change in
//! it may be from before the decision; [`ChangeWindow::starts_before_opinion`]
//! says so. And with no window at all, [`SincePublication::printings_after`]
//! tells "no printing after the opinion" from "one printing, and nothing to
//! compare it with".
//!
//! # Below the section
//!
//! A citation link ends at the subdivision the opinion wrote, as it resolved in
//! the printing the link was made against (#316): `26 U.S.C. § 174(a)(1)` ends
//! at `…/section_174/subsection_a/paragraph_1`, and `26 U.S.C. § 174(e)`, which
//! no held printing has, ends at the section and names `(e)` in its payload.
//! A change window compares two other printings, so the subdivision is
//! resolved again in each of them when it is read, and nothing is written back
//! (#302). A change window is read at that subdivision where both
//! of its printings hold it, and at the whole section where they do not, and
//! [`ChangeWindow::at`] says which. The words of a subdivision can stay the
//! same while the chapeau above them changes what they mean, so a window read
//! at a subdivision also names every provision above it whose own words
//! changed ([`ChangeWindow::changed_above`]).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::citation::resolve::{Subdivision, resolve_subdivision, stub_heading};
use crate::dataset::{Coverage, DatasetError, ExpressionId, Scope, WorkId};
use crate::diff::TreeDiff;
use crate::document::{DocumentNode, TextContentField};
use crate::link::{Link, LinkKind, Target, VerificationState};
use crate::method::Method;
use crate::review::short_id;
use crate::storage::{DocumentReader, LinkReader};
use crate::uslm::path::covers_path;

/// Whether the dataset holds the opinion a citation link came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Citing {
    /// The dataset holds it, as one writing in one expression of a case.
    /// Everything the report says about it can be checked against the text in
    /// the file.
    Held {
        /// The case, as it read on the day the court filed it.
        expression: ExpressionId,
        /// The writing that cites: the link's subject, such as
        /// `judicial/case_122262/opinion_9434365`. Two writings of one case are
        /// two entries, so a dissent's citation is never read as the Court's.
        path: String,
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
                path,
                case_name,
                ..
            } => match case_name {
                Some(name) => format!("{name} ({path}, filed {})", expression.at),
                None => format!("{path}, filed {}", expression.at),
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeWindow {
    pub from: String,
    pub to: String,
    /// Where the change was looked for: the subdivision the opinion wrote when
    /// both printings hold it, and the whole section when they do not.
    pub at: String,
    /// True when the provision does not say the same thing on both dates.
    pub changed: bool,
    /// True when the window starts before the day the opinion was filed and
    /// ends after it. A change in such a window can be from before the decision
    /// or from after it, and the two printings cannot say which (#340).
    pub starts_before_opinion: bool,
    /// Every path at or under the provision whose text differs, in document
    /// order. Empty when nothing changed.
    pub changed_paths: Vec<String>,
    /// Every provision above [`ChangeWindow::at`], up to the section, whose own
    /// words differ: the chapeau of (a) above an (a)(1) that reads the same.
    /// The words of `at` can stay the same while their meaning changes, so a
    /// reader must not take `changed: false` alone as "means the same" (#340).
    /// Empty when the window is read at the whole section.
    pub changed_above: Vec<ChangedAbove>,
}

/// A provision above the one a window was read at, whose own words changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangedAbove {
    pub path: String,
    /// Which of its own text fields differ, such as its chapeau or its heading.
    pub fields: Vec<TextContentField>,
}

/// A period the dataset says nothing about.
///
/// From the day the opinion was filed to the first printing of the cited work
/// that the dataset holds. The provision may have been rewritten ten times in it,
/// and this dataset cannot see any of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UncoveredPeriod {
    pub from: String,
    pub to: String,
}

/// What this dataset can say about a provision since an opinion was filed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SincePublication {
    /// Whether the dataset carries the cited material at all.
    pub coverage: Coverage,
    /// Every consecutive pair of printings held whose later date falls after the
    /// opinion, oldest first.
    pub windows: Vec<ChangeWindow>,
    /// How many printings of the cited work the dataset holds that are dated
    /// after the opinion. With no window, this tells "holds none after it"
    /// from "holds one, and nothing to compare it with" (#340).
    pub printings_after: usize,
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

/// How a citation stands to the path a caller asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    /// The opinion cites the path asked about, or a provision under it.
    Within,
    /// The opinion cites a provision that holds the path asked about: the
    /// section with no subdivision, a subdivision above the path, or a
    /// subdivision the printing does not hold, so only the section is known.
    /// It is not "does not cite", and it is not "cites this path".
    Holds,
}

/// One provision one writing cites, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CitedProvision {
    /// Where the provision sits: a structural path, because a provision has no
    /// identity of its own yet (`docs/adr/0001-structural-paths-locate-not-identify.md`).
    /// The deepest provision the subdivision the court wrote leads to (#316).
    pub path: String,
    /// Each passage of the writing that cites the provision, in path order:
    /// one `judicial.cites` link each.
    pub places: Vec<CitedPlace>,
    /// Where the subdivision the opinion wrote leads in the latest printing
    /// held: the `(a)(1)` of `174(a)(1)`.
    #[serde(flatten)]
    pub subdivision: Subdivision,
    /// The heading of the cited section when the latest printing held prints it
    /// as a stub, such as `[Renumbered § 30]`, and `None` for a section of law
    /// (`crate::citation::resolve::stub_heading`). The court cited law that
    /// this printing no longer holds at that number (#310).
    pub stub_heading: Option<String>,
    /// How this citation stands to the path asked about.
    pub relation: Relation,
    pub since: SincePublication,
}

/// One passage that cites a provision: one `judicial.cites` link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CitedPlace {
    /// The link's id, shortened as a report prints it. `settle` takes it.
    pub link: String,
    /// The paragraph or footnote that holds the citation: the link's subject.
    /// For an opinion the dataset does not hold, its reference.
    pub passage: String,
    /// Each form the court wrote in the passage, once: `174(a)(1)`.
    pub written: Vec<String>,
    /// The written designations that lead to no provision in the printing the
    /// link was made against, such as `(e)` of `174(e)`. Empty when every one
    /// leads to a provision.
    pub unresolved: Vec<String>,
    /// The text the rule matched, from the link's evidence. This is what a
    /// reviewer checks the link against.
    pub citation_text: Option<String>,
    /// How far the *link* can be trusted — not the provision, and not the
    /// opinion's text.
    pub verification: VerificationState,
}

/// One opinion, and the provisions it cites that the caller asked about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CitingCase {
    pub citing: Citing,
    /// In path order, so two runs report alike.
    pub cites: Vec<CitedProvision>,
}

/// Every opinion in this dataset that cites a provision at or under `path`, or
/// a provision that holds it.
///
/// A link starts at the passage that holds the citation and ends at the
/// subdivision the opinion wrote (#316). The subdivision is resolved again here,
/// at read time, in the latest printing held ([`CitedProvision::subdivision`]). Each citation then says how
/// it stands to `path` ([`Relation`]): it cites `path` or a provision under it,
/// or it cites a provision above `path` — the section alone, a subdivision that
/// holds `path`, or a subdivision the printing does not hold. A citation whose
/// subdivision resolves to another branch, such as (a) when (e) is asked
/// about, is left out: that opinion does not cite `path`.
///
/// In the order the opinions are named, so two runs agree. Each writing is one
/// entry, with one [`CitedProvision`] for each provision it cites, and each
/// passage that cites that provision is one of its [`CitedPlace`]s: one
/// `judicial.cites` link each. So a reader asked about a section gets each
/// writing once, grouped by the provisions in the section it cites.
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

    // Every printing of each cited section, read once. Nine opinions cite § 174,
    // and reading its title nine times over is the cost of not keeping it.
    let mut printings: BTreeMap<String, Vec<(ExpressionId, DocumentNode)>> = BTreeMap::new();

    let mut cases: Vec<CitingCase> = Vec::new();
    for link in links {
        let Target::Node(cited_path) = &link.object else {
            continue;
        };
        if !covers_path(path, cited_path) && !covers_path(cited_path, path) {
            continue;
        }
        let Some((section, written)) = section_and_written(cited_path) else {
            continue;
        };
        if !printings.contains_key(&section) {
            printings.insert(section.clone(), reader.find_nodes(&section)?);
        }
        let held = &printings[&section];

        let latest = held
            .iter()
            .max_by(|a, b| a.0.at.cmp(&b.0.at))
            .map(|(_, section)| section);
        let place = place_of(&link);
        let subdivision = match latest.map(|section| resolve_subdivision(section, &written)) {
            // The link names the section, and a place names what the court
            // wrote below it that leads nowhere.
            Some(Subdivision::Section) | None => match place.unresolved.first() {
                Some(unresolved) => Subdivision::Unresolved {
                    unresolved: unresolved.clone(),
                },
                None => Subdivision::Section,
            },
            Some(subdivision) => subdivision,
        };
        let Some(relation) = relation_to(&subdivision, &section, path) else {
            continue;
        };

        let citing = citing_of(reader, &link)?;
        let case = match cases.iter_mut().position(|case| case.citing == citing) {
            Some(index) => &mut cases[index],
            None => {
                cases.push(CitingCase {
                    citing,
                    cites: Vec::new(),
                });
                cases.last_mut().expect("just pushed")
            }
        };
        match case
            .cites
            .iter_mut()
            .find(|cited| cited.path == *cited_path)
        {
            Some(cited) => cited.places.push(place),
            None => case.cites.push(CitedProvision {
                since: since_publication(
                    reader,
                    &scope,
                    &section,
                    held,
                    &written,
                    case.citing.at(),
                )?,
                path: cited_path.clone(),
                places: vec![place],
                subdivision,
                stub_heading: latest.and_then(stub_heading),
                relation,
            }),
        }
    }

    for case in &mut cases {
        case.cites.sort_by(|a, b| a.path.cmp(&b.path));
        for cited in &mut case.cites {
            cited.places.sort_by(|a, b| a.passage.cmp(&b.passage));
        }
    }
    Ok(cases)
}

/// The section a cited path is in, and the path below it written as a court
/// writes a citation: `section_174` and `174(a)(1)` for
/// `…/section_174/subsection_a/paragraph_1`. `None` for a path in no section.
///
/// A link ends at the subdivision it resolved to in one printing (#316), and a
/// change window compares two others. Written again as a citation, the
/// subdivision is resolved in each printing the way the court's own words are,
/// so a subdivision added or removed between two printings is said, not
/// guessed.
fn section_and_written(cited_path: &str) -> Option<(String, String)> {
    let segments: Vec<&str> = cited_path.split('/').collect();
    let at = segments
        .iter()
        .position(|segment| segment.starts_with("section_"))?;
    let number = segments[at].strip_prefix("section_")?;
    let below: String = segments[at + 1..]
        .iter()
        .filter_map(|segment| segment.split_once('_'))
        .map(|(_, designation)| format!("({designation})"))
        .collect();
    Some((segments[..=at].join("/"), format!("{number}{below}")))
}

/// One link, as the place a passage cites a provision.
fn place_of(link: &Link) -> CitedPlace {
    let value = link.payload.as_ref().map(|payload| &payload.value);
    let places = value
        .and_then(|value| value["occurrences"].as_array())
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut written: Vec<String> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    for place in places {
        if let Some(form) = place["written"].as_str()
            && !written.iter().any(|known| known == form)
        {
            written.push(form.to_string());
        }
        if let Some(part) = place["unresolved"].as_str()
            && !unresolved.iter().any(|known| known == part)
        {
            unresolved.push(part.to_string());
        }
    }
    CitedPlace {
        link: short_id(&link.id()).to_string(),
        passage: link.subject.name(),
        written,
        unresolved,
        citation_text: link
            .provenance
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.reasoning.clone()),
        verification: link.provenance.verification,
    }
}

/// Every window of consecutive printings this dataset holds, and whether the
/// provision at `path` changed in it, with no opinion to measure from. The
/// coverage says whether the dataset carries the provision at all.
///
/// This is what a reader can still say about a provision that no opinion
/// cites: "no opinion cites it" is not "it did not change" (#340).
pub fn changes_held<R: DocumentReader + ?Sized>(
    reader: &R,
    path: &str,
) -> Result<SincePublication, DatasetError> {
    let scope = Scope::derive(reader)?;
    let held = reader.find_nodes(path)?;
    since_publication(reader, &scope, path, &held, "", None)
}

/// How a citation of the section at `cited_path` stands to the path `asked`.
///
/// A citation to `26 U.S.C. § 174` resolves to the section, and a caller asking
/// about the section means the whole of it, so a link into a subsection counts.
/// A caller asking about a path below the section is answered from the
/// subdivision the opinion wrote. `None` means the citation does not reach the
/// path asked about.
fn relation_to(subdivision: &Subdivision, cited_path: &str, asked: &str) -> Option<Relation> {
    if covers_path(asked, cited_path) {
        return Some(Relation::Within);
    }
    match subdivision {
        Subdivision::Resolved { resolved_path: at } if covers_path(asked, at) => {
            Some(Relation::Within)
        }
        Subdivision::Resolved { resolved_path: at } if covers_path(at, asked) => {
            Some(Relation::Holds)
        }
        // Another branch of the section, such as (a) when (e) is asked about.
        Subdivision::Resolved { .. } => None,
        Subdivision::Section | Subdivision::Unresolved { .. } => Some(Relation::Holds),
    }
}

/// Who the citing opinion is, and whether this dataset holds it.
fn citing_of<R: DocumentReader + ?Sized>(reader: &R, link: &Link) -> Result<Citing, DatasetError> {
    let path = match &link.subject {
        Target::Node(path) => path,
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

    // The writing that holds the passage: `judicial/case_<cluster>/opinion_<id>`.
    // Two writings of one case are two entries, and two passages of one
    // writing are one.
    let path = &super::writing_of(path).to_string();
    let found = reader.find_nodes(path)?;
    let Some((expression, node)) = found.into_iter().next() else {
        return Ok(Citing::NotHeld {
            reference: path.clone(),
            display: format!("an opinion at {path}, which this dataset does not hold"),
        });
    };

    let case_name = case_name_of(reader, &expression)?;
    let provenance = node.data.provenance.as_ref();
    Ok(Citing::Held {
        expression,
        path: path.clone(),
        case_name: case_name.or_else(|| node.data.heading.as_deref().map(str::to_string)),
        text_source: provenance.map(|provenance| provenance.source.clone()),
        text_method: provenance.and_then(|provenance| provenance.method.clone()),
        text_verification: provenance.map(|provenance| provenance.verification),
    })
}

/// The name of the case an expression is, from the case root's facts or its
/// heading. `None` when the root carries neither.
fn case_name_of<R: DocumentReader + ?Sized>(
    reader: &R,
    expression: &ExpressionId,
) -> Result<Option<String>, DatasetError> {
    let root = reader
        .find_nodes(expression.work.as_str())?
        .into_iter()
        .find(|(id, _)| id == expression)
        .map(|(_, root)| root);
    Ok(root.and_then(|root| {
        super::CaseFacts::of(&root.data)
            .map(|facts| facts.case_name)
            .or_else(|| root.data.heading.as_deref().map(str::to_string))
    }))
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
    held: &[(ExpressionId, DocumentNode)],
    written: &str,
    opinion_date: Option<&str>,
) -> Result<SincePublication, DatasetError> {
    let coverage = scope.covers(path);
    let Some(work) = work_holding(scope, path) else {
        return Ok(SincePublication {
            coverage,
            windows: Vec::new(),
            printings_after: 0,
            uncovered: None,
        });
    };

    let mut dates: Vec<String> = reader
        .expressions(&work)?
        .into_iter()
        .map(|info| info.id.at)
        .collect();
    dates.sort();

    let mut windows = Vec::new();
    for pair in dates.windows(2) {
        let (from, to) = (&pair[0], &pair[1]);
        // A window that closes before the opinion was filed says nothing about
        // what happened after it.
        if opinion_date.is_some_and(|filed| to.as_str() <= filed) {
            continue;
        }
        let Changes {
            at,
            changed_paths,
            changed_above,
        } = changed_between(held, &work, path, written, from, to);
        windows.push(ChangeWindow {
            from: from.clone(),
            to: to.clone(),
            at,
            changed: !changed_paths.is_empty(),
            starts_before_opinion: opinion_date.is_some_and(|filed| from.as_str() < filed),
            changed_paths,
            changed_above,
        });
    }

    let uncovered = opinion_date.zip(dates.first()).and_then(|(filed, first)| {
        (first.as_str() > filed).then(|| UncoveredPeriod {
            from: filed.to_string(),
            to: first.clone(),
        })
    });

    let printings_after = dates
        .iter()
        .filter(|date| opinion_date.is_none_or(|filed| date.as_str() > filed))
        .count();

    Ok(SincePublication {
        coverage,
        windows,
        printings_after,
        uncovered,
    })
}

/// The work that holds `path`, out of the works the dataset carries.
///
/// Read from the scope rather than built from the path, so this stays
/// class-neutral: `uscode/title_26/.../section_174` belongs to
/// `uscode/title_26`, `judicial/case_109019/opinion_109019` belongs to
/// `judicial/case_109019`, and nothing here needs to know which rule made
/// either.
fn work_holding(scope: &Scope, path: &str) -> Option<WorkId> {
    scope
        .works()
        .find(|work| covers_path(work.as_str(), path))
        .cloned()
}

/// What changed in one window, read at one path.
struct Changes {
    at: String,
    changed_paths: Vec<String>,
    changed_above: Vec<ChangedAbove>,
}

/// Where to look for a change, every path there whose text differs between
/// the two printings, and every provision above it whose own words differ.
///
/// The subdivision the opinion wrote is resolved in each printing, because a
/// subdivision can be added or removed between them. Where both printings hold
/// it at one path, the change is read there; otherwise the whole section is
/// read, and the path returned says so.
///
/// The date is deliberately not compared: two printings of one provision always
/// differ in it, and comparing whole nodes would report every provision as
/// changed. [`TreeDiff`] compares the five text fields and the children, which
/// is the codebase's one definition of "this provision changed".
fn changed_between(
    held: &[(ExpressionId, DocumentNode)],
    work: &WorkId,
    section: &str,
    written: &str,
    from: &str,
    to: &str,
) -> Changes {
    let at = |date: &str| {
        held.iter()
            .find(|(id, _)| &id.work == work && id.at == date)
            .map(|(_, node)| node)
    };
    let (Some(before), Some(after)) = (at(from), at(to)) else {
        // The provision is not at this path on both dates. Added or removed
        // rather than changed, and neither is this function's question.
        return Changes {
            at: section.to_string(),
            changed_paths: Vec::new(),
            changed_above: Vec::new(),
        };
    };
    let whole_section = TreeDiff::from_nodes(before, after);

    let (before, after) = match (
        resolve_subdivision(before, written),
        resolve_subdivision(after, written),
    ) {
        (
            Subdivision::Resolved { resolved_path: was },
            Subdivision::Resolved { resolved_path: is },
        ) if was == is => match (before.find(&was), after.find(&is)) {
            (Some(was), Some(is)) => (was, is),
            _ => (before, after),
        },
        _ => (before, after),
    };

    let at = before.data.path.to_string();
    let mut changed_paths = Vec::new();
    collect_changed(&TreeDiff::from_nodes(before, after), &mut changed_paths);
    let mut changed_above = Vec::new();
    collect_changed_above(&whole_section, &at, &mut changed_above);
    Changes {
        at,
        changed_paths,
        changed_above,
    }
}

/// The provisions strictly above `at` whose own text fields differ, from the
/// section down.
fn collect_changed_above(diff: &TreeDiff, at: &str, into: &mut Vec<ChangedAbove>) {
    if diff.root_path == at || !covers_path(&diff.root_path, at) {
        return;
    }
    if !diff.changes.is_empty() {
        into.push(ChangedAbove {
            path: diff.root_path.clone(),
            fields: diff
                .changes
                .iter()
                .map(|change| change.field_name)
                .collect(),
        });
    }
    for child in &diff.child_diffs {
        collect_changed_above(child, at, into);
    }
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
