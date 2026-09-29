//! Links that the current version of a batch method no longer makes (#185).
//!
//! A link's identity is its subject, its kind and its object
//! (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`). So
//! when a newer version of a method runs over a bill and a window:
//!
//! - a link that both versions make is one link, and the run re-stamps it with
//!   the newer version;
//! - a link that only the newer version makes is added;
//! - a link that **only the older version made** is kept, because evidence is
//!   never deleted (`docs/adr/0005-evidence-is-stored-once-and-never-deleted.md`).
//!   It keeps the older version, and nothing else marks it.
//!
//! **The rule.** Group the links of one batch method by bill and by window. In
//! a group whose links carry more than one version of the method, each link
//! below the group's newest version is **outdated**: that bill was re-run at
//! the newest version over that window, and the re-run did not make this link.
//!
//! **Batch methods only.** A batch method runs over every amendment of a bill,
//! so its silence about a link is an answer. An agent covers only the items it
//! chose, so a newer version of an agent method says nothing about the links it
//! did not make, and they are never outdated here.
//!
//! **By bill, not only by window.** A run for one bill never touches another
//! bill's links, so another bill's newer run must not make them outdated.
//!
//! **A blind spot, accepted.** A newer run that made no link at all for a bill
//! in a window leaves no newer version in the group, so nothing is flagged. It
//! misses a flag. It never gives a false one.
//!
//! **Nothing is stored.** This is derived every time it is asked for
//! (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//! Nothing here refutes, hides or deletes a link: a reviewer decides.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::dataset::DatasetError;
use crate::legislature::evidence_matching::evidence_method;
use crate::legislature::redesignation::reading_method;
use crate::link::{Link, LinkKind, Window, amendment_reference_parts};
use crate::method::Method;
use crate::storage::Storage;

/// One link the current version of its method no longer makes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutdatedLink {
    /// The link, by its full id.
    pub link_id: String,
    /// The path the link's subject names.
    pub path: String,
    /// The bill whose run made the link.
    pub bill_id: String,
    /// The work the window belongs to.
    pub work: String,
    /// The window the link was made over.
    pub window: Window,
    /// The method, at the version that made the link.
    pub made_by: Method,
    /// The same method, at the newest version that ran over this bill and
    /// window.
    pub remade_by: Method,
}

/// Every outdated link of `bill`, or of every bill when `bill` is `None`, in
/// the order of their ids.
pub fn outdated_links<S: Storage>(
    dataset: &S,
    bill: Option<&str>,
) -> Result<Vec<OutdatedLink>, DatasetError> {
    let mut links = dataset.links_by_kind(LinkKind::AMENDED_BY)?;
    links.extend(dataset.links_by_kind(LinkKind::REDESIGNATED_AS)?);
    Ok(outdated_among(&links)
        .into_iter()
        .filter(|row| bill.is_none_or(|bill| bill == row.bill_id))
        .collect())
}

/// Whether `link` is outdated, and if it is, which versions say so.
///
/// `None` for a link that names no bill: no batch run made it.
pub fn outdated_link<S: Storage>(
    dataset: &S,
    link: &Link,
) -> Result<Option<OutdatedLink>, DatasetError> {
    let Some(bill_id) = bill_of(link) else {
        return Ok(None);
    };
    let link_id = link.id();
    Ok(outdated_links(dataset, Some(&bill_id))?
        .into_iter()
        .find(|row| row.link_id == link_id))
}

/// The outdated links among `links`, in the order of their ids.
fn outdated_among(links: &[Link]) -> Vec<OutdatedLink> {
    let batch = batch_method_names();
    let mut groups: BTreeMap<Group, Vec<(&Link, &Method)>> = BTreeMap::new();
    for link in links {
        let Some(method) = &link.provenance.method else {
            continue;
        };
        if !batch.contains(&method.name) {
            continue;
        }
        let (Some(bill_id), Some(work), Some(window)) =
            (bill_of(link), link.subject.work(), link.subject.window())
        else {
            continue;
        };
        let group = Group {
            method: method.name.clone(),
            bill_id,
            work: work.to_string(),
            window,
        };
        groups.entry(group).or_default().push((link, method));
    }

    let mut outdated = Vec::new();
    for (group, members) in groups {
        let Some(newest) = members.iter().map(|(_, method)| method.version).max() else {
            continue;
        };
        for (link, method) in members {
            if method.version < newest {
                outdated.push(OutdatedLink {
                    link_id: link.id(),
                    path: link.subject.path().unwrap_or_default().to_string(),
                    bill_id: group.bill_id.clone(),
                    work: group.work.clone(),
                    window: group.window.clone(),
                    made_by: method.clone(),
                    remade_by: Method::new(group.method.clone(), newest),
                });
            }
        }
    }
    outdated.sort_by(|left, right| left.link_id.cmp(&right.link_id));
    outdated
}

/// One method's links for one bill in one window.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Group {
    method: String,
    bill_id: String,
    work: String,
    window: Window,
}

/// The methods that run over every amendment of a bill: the evidence matcher
/// and the renumbering step. An agent's method is never one of them.
fn batch_method_names() -> [String; 2] {
    [evidence_method().name, reading_method().name]
}

/// The bill whose run made a link: the bill its amendment reference names, or
/// for a renumbering the bill that stated it.
fn bill_of(link: &Link) -> Option<String> {
    if let Some((bill_id, _)) = amendment_reference_parts(&link.object.name()) {
        return Some(bill_id.to_string());
    }
    link.payload
        .as_ref()
        .and_then(|payload| payload.value.get("bill_id"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}
