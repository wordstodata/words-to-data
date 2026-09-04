# Structural paths locate provisions; they do not identify them

Status: accepted

Legal sources often supply no usable identifier, so we derive a structural path from the hierarchy the parser found, such as `uscode/title_26/subtitle_F/chapter_61/subchapter_A/part_III/subpart_B/section_6041/subsection_d`. This works well against messy documents and we keep it. But a derived path is an address, and an address changes. `AmendingAction` already contains `Redesignate` and `Move` (`src/uslm/mod.rs`), which are exactly the operations that keep a provision alive and move it. A path also changes when our own parser improves.

We therefore give each provision a stable identity that is carried across versions, and we treat the structural path as a locator at one point in time. Links point at the identity and record the locator beside it.

## Consequences

Without this, three things break quietly. A `Move` reads as a delete plus an add, so the diff reports a change to law that did not change. Human-confirmed annotations, which point at path strings today (`ChangeAnnotation.paths`), silently attach to different text after a parser change, and the party who receives the file cannot detect it. And "is this the same provision as last year", which is the question a legal researcher actually asks, has no answer.

The cost is one more identifier to mint and carry. We already do this for amendments, where `amendment_id` is a content hash, so the mechanism is not new.

## Considered and rejected

**The path is the identity.** Simplest, and wrong in a way that produces confident false statements rather than errors.

**The path is the identity, plus an alias table of renames.** This puts the truth in a side table. A reader that does not consult it gets the wrong answer, and no other implementation of the format is obliged to consult it.
