# Structural paths locate provisions; they do not identify them

Status: accepted

Legal sources often supply no usable identifier, so we derive a structural path from the hierarchy the parser found, such as `uscode/title_26/subtitle_F/chapter_61/subchapter_A/part_III/subpart_B/section_6041/subsection_d`. This works well against messy documents and we keep it. But a derived path is an address, and an address changes. `AmendingAction` already contains `Redesignate` and `Move` (`src/uslm/mod.rs`), which are exactly the operations that keep a provision alive and move it. A path also changes when our own parser improves.

We therefore give each provision a stable identity that is carried across versions, and we treat the structural path as a locator at one point in time. Links point at the identity and record the locator beside it.

## Consequences

Without this, three things break quietly. A `Move` reads as a delete plus an add, so the diff reports a change to law that did not change. Human-confirmed annotations, which point at path strings today (`ChangeAnnotation.paths`), silently attach to different text after a parser change, and the party who receives the file cannot detect it. And "is this the same provision as last year", which is the question a legal researcher actually asks, has no answer.

A locator is not only stale across versions; it can be ambiguous within one. The law sometimes numbers two provisions alike — `26 U.S.C. § 45X(d)(4)` is two paragraphs (4), which the Code footnotes as "So in original" and the official site renders in full — so one path names both. Code that treats a path as unique within an expression does not fail loudly. It picks one and discards the other, which is the confident false statement this decision exists to avoid.

Where a path names more than one provision, the document's order decides which is which. We inherit the order the source gives, pair provisions by position, and record the position explicitly rather than leaving it implicit in a parse. A parse keeps the order; a round trip through storage or a reserialized report is where it goes missing, and it goes missing silently. The alternatives to position are worse. Pairing by content similarity invents a judgement the source did not make, and treating a collision as an error refuses to hold law that genuinely exists.

Position pairing has one limit, which we accept for now. When the number of provisions at a path falls, it assumes the survivors are the leading ones. If the source instead drops the first and keeps the second, the diff reports a large text change plus a removal, rather than the removal that happened. The corpus has not shown this yet: at `26 U.S.C. § 6724(d)(2)`, where two subparagraphs (JJ) became one between 2025-07-18 and 2025-07-30, the survivor is the first and position pairing is correct. That is luck rather than design, because nothing in the pairing can tell the two apart. A stable provision identity is what fixes this. Position is what we have until then.

The cost is one more identifier to mint and carry. We already do this for amendments, where `amendment_id` is a content hash, so the mechanism is not new.

## Considered and rejected

**The path is the identity.** Simplest, and wrong in a way that produces confident false statements rather than errors.

**The path is the identity, plus an alias table of renames.** This puts the truth in a side table. A reader that does not consult it gets the wrong answer, and no other implementation of the format is obliged to consult it.
