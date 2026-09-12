# Structural paths locate provisions; they do not identify them

Status: accepted. **Partly implemented — see Implementation status.**

## Implementation status

The paragraph below says "we therefore give each provision a stable identity", in the settled tense. **We have not.** No provision identity is minted or carried anywhere in the code. `Target::Provision` names a provision by path, and `WorkId` holds a structural path for the same reason (`docs/adr/0003-storage-is-keyed-by-work.md` records that debt too).

What is implemented is the second half: the path is treated as a locator, a path may name more than one provision, and the position is recorded explicitly rather than left implicit. The Consequences section below is accurate about that, and says so directly — "Position is what we have until then."

So this ADR records an accepted decision and a partly-built one. #93 is the work, and it is queued for the release window after `SCHEMA_VERSION` 7. Until then, every statement here about identity should be read as intent, and every statement about position as fact.

### A gap this ADR did not anticipate

A derived path was meant to owe the source document nothing. It does, where an element carries a number. Where an element carries **no** number — which is the usual case for a container that groups a body of law — the segment has to come from the source. It used to fall back to the element's XML id, and **14,484** paths in the dataset read like this, of which 1,736 sat in nine ordinary titles:

```
uscode/appendix_28a/level_id2e47c0a6-b17c-11ef-b971-e82c9e4f66ce/title_I/level_1
```

#115 replaced that with the publisher's own name for the container: the `identifier` attribute reduced to its last segment, and where there is none, the heading. No uuid segment remains in a dataset built from the committed release points, and Rule 1 of the Federal Rules of Civil Procedure is now at `uscode/appendix_28a/level_Civil/title_I/level_1`.

The dependence is smaller but it did not go away, and where the segment comes from a heading it took a new shape. **6,962** paths across the two committed release points sit at or below a container named by its heading, in thirteen works, the largest groups being the title 11 appendix, title 29, the title 28 appendix and title 38. A heading is stable against a sibling being inserted above the container, which is why it was preferred to numbering by position, but it is not stable against the publisher rewording it. That is the same class of movement this ADR already describes for `Redesignate` and `Move`, and the same answer applies: a locator moves, and only a minted identity does not.

Legal sources often supply no usable identifier, so we derive a structural path from the hierarchy the parser found, such as `uscode/title_26/subtitle_F/chapter_61/subchapter_A/part_III/subpart_B/section_6041/subsection_d`. This works well against messy documents and we keep it. But a derived path is an address, and an address changes. `AmendingAction` already contains `Redesignate` and `Move` (`src/uslm/mod.rs`), which are exactly the operations that keep a provision alive and move it. A path also changes when our own parser improves.

We therefore give each provision a stable identity that is carried across versions, and we treat the structural path as a locator at one point in time. Links point at the identity and record the locator beside it.

> **Not built.** The identity in that paragraph does not exist in the code; a link points at a path. The second half — the path as a locator — is built. See Implementation status above, and #93.

## Consequences

Without this, three things break quietly. A `Move` reads as a delete plus an add, so the diff reports a change to law that did not change. Human-confirmed annotations, which point at path strings today (`ChangeAnnotation.paths`), silently attach to different text after a parser change, and the party who receives the file cannot detect it. And "is this the same provision as last year", which is the question a legal researcher actually asks, has no answer.

A locator is not only stale across versions; it can be ambiguous within one. The law sometimes numbers two provisions alike — `26 U.S.C. § 45X(d)(4)` is two paragraphs (4), which the Code footnotes as "So in original" and the official site renders in full — so one path names both. Code that treats a path as unique within an expression does not fail loudly. It picks one and discards the other, which is the confident false statement this decision exists to avoid.

Where a path names more than one provision, the document's order decides which is which. We inherit the order the source gives, pair provisions by position, and record the position explicitly rather than leaving it implicit in a parse. A parse keeps the order; a round trip through storage or a reserialized report is where it goes missing, and it goes missing silently. The alternatives to position are worse. Pairing by content similarity invents a judgement the source did not make, and treating a collision as an error refuses to hold law that genuinely exists.

Position pairing has one limit, which we accept for now. When the number of provisions at a path falls, it assumes the survivors are the leading ones. If the source instead drops the first and keeps the second, the diff reports a large text change plus a removal, rather than the removal that happened. The corpus has not shown this yet: at `26 U.S.C. § 6724(d)(2)`, where two subparagraphs (JJ) became one between 2025-07-18 and 2025-07-30, the survivor is the first and position pairing is correct. That is luck rather than design, because nothing in the pairing can tell the two apart. A stable provision identity is what fixes this. Position is what we have until then.

The cost is one more identifier to mint and carry. We already do this for amendments, where `amendment_id` is a content hash, so the mechanism is not new.

## Considered and rejected

**The path is the identity.** Simplest, and wrong in a way that produces confident false statements rather than errors.

**The path is the identity, plus an alias table of renames.** This puts the truth in a side table. A reader that does not consult it gets the wrong answer, and no other implementation of the format is obliged to consult it.
