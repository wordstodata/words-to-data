//! The `judicial.cites` links of one citation range, grouped as the court wrote
//! the range (#383).
//!
//! A range is more than one link. `26 U. S. C. §§ 991-997` is a link to each of
//! its two ends (#309), and `§§994(a)(1)-(3)` is a link to each of its members
//! (#366). Each of those links holds the place where the court wrote the range
//! (`CONTEXT.md`, "Citation"): the same passage, the same `start` and the same
//! `written` form. That place is what groups them.
//!
//! The group is a projection (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`):
//! it is read from the stored links each time, and nothing about it is stored.
//! `cases-citing` and `annotations` both call [`ranges_of`], so the rule is in
//! one place.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::citation::usc::{range_ends, subdivision_range};
use crate::link::Link;
use crate::review::short_id;

/// One range a passage wrote, and each provision it links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CitedRange {
    /// The passage that writes the range: the subject of each of its links.
    pub passage: String,
    /// Where the citation starts in the words of the passage, in bytes.
    pub start: usize,
    /// The range as the court wrote it: `991-997` or `994(a)(1)-(3)`.
    pub written: String,
    /// The words the rule matched: `26 U. S. C. §§ 991-997`.
    pub words: String,
    /// Each link of the range, in the order of the cited paths.
    pub members: Vec<RangeMember>,
}

/// One link of a range: the provision it cites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RangeMember {
    /// The link's id, shortened as a report prints it. `settle` takes it.
    pub link: String,
    /// The provision the link cites: its object.
    pub path: String,
}

impl CitedRange {
    /// Whether the link with the short id `link` is one of this range's.
    pub fn holds(&self, link: &str) -> bool {
        self.members.iter().any(|member| member.link == link)
    }
}

/// Every range that `links` write, in the order of the passage and the start.
///
/// The links of one range have the same subject, and each holds a place with
/// the same `start` and the same `written` form. That form must read as a range
/// of sections ([`range_ends`]) or of subdivisions ([`subdivision_range`]), and
/// at least two links must hold the place. So a section numbered with a hyphen,
/// such as `20 U.S.C. § 1078-1`, is one link and no range, and two citations of
/// one provision at two places are two places of one link, not a range.
pub fn ranges_of<'a>(links: impl IntoIterator<Item = &'a Link>) -> Vec<CitedRange> {
    let mut places: BTreeMap<(String, usize, String), CitedRange> = BTreeMap::new();
    for link in links {
        let Some(path) = link.object.path() else {
            continue;
        };
        let occurrences = link
            .payload
            .as_ref()
            .and_then(|payload| payload.value["occurrences"].as_array());
        for place in occurrences.into_iter().flatten() {
            let (Some(start), Some(written)) = (place["start"].as_u64(), place["written"].as_str())
            else {
                continue;
            };
            if range_ends(written).is_none() && subdivision_range(written).is_none() {
                continue;
            }
            let passage = link.subject.name();
            let start = start as usize;
            let range = places
                .entry((passage.clone(), start, written.to_string()))
                .or_insert_with(|| CitedRange {
                    passage,
                    start,
                    written: written.to_string(),
                    words: place["words"].as_str().unwrap_or_default().to_string(),
                    members: Vec::new(),
                });
            let id = link.id();
            let member = RangeMember {
                link: short_id(&id).to_string(),
                path: path.to_string(),
            };
            if !range.members.contains(&member) {
                range.members.push(member);
            }
        }
    }
    places
        .into_values()
        .filter(|range| range.members.len() > 1)
        .map(|mut range| {
            range.members.sort_by(|a, b| a.path.cmp(&b.path));
            range
        })
        .collect()
}
