# Structural paths locate provisions; they do not identify them

Status: accepted. **The identity this ADR recommends was not built, and will not be — see Implementation status.**

## Implementation status

The paragraph below says "we therefore give each provision a stable identity", in the settled tense. **We have not, and #93 decided not to.** No provision identity is minted or carried anywhere in the code, and none is planned. `Target::Provision` names a provision by path, and `WorkId` holds a structural path for the same reason (`docs/adr/0003-storage-is-keyed-by-work.md` records that debt too).

**Do not build the identity this ADR recommends.** A provision has nothing stable to hash. Its text changes, which is the point of tracking it, and its location changes, which is why identity was wanted. A minted id would move whenever a newly added bill revealed an earlier redesignation, which is the exact defect `docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md` rejected UUIDs for: "it changes on every rebuild, so nothing outside the file can point at a link."

**What was built instead is an edge.** A redesignation is a record — one `Link` of kind `legislature.redesignated_as`, whose subject is the provision as it was and whose object is the provision as it became, each naming a change so the edge carries the dates it was observed between. Continuity is then a **projection**: "is this the same provision as last year" is answered by walking those links, forwards and backwards, and nothing stores the chain (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`, `LinkReader::provision_history`).

An edge is preferred because it is additive. Adding a bill later adds an edge, where minting an identity would rewrite one, so nothing that already points somewhere breaks. #93 therefore needed no `SCHEMA_VERSION` bump and no rebuild, and `Target` did not change.

What was already implemented is the second half of this ADR: the path is treated as a locator, a path may name more than one provision, and the position is recorded explicitly rather than left implicit. The Consequences section below is accurate about that, except where it says identity is the fix; see the note there.

So this ADR records a problem statement that still holds and a remedy that was replaced. Read every statement about the path as a locator as fact, and every statement about a minted identity as a proposal that #93 answered another way.

### A gap this ADR did not anticipate

A derived path was meant to owe the source document nothing. It does, where an element carries a number. Where an element carries **no** number — which is the usual case for a container that groups a body of law — the segment has to come from the source. It used to fall back to the element's XML id, and **14,484** paths in the dataset read like this, of which 1,736 sat in nine ordinary titles:

```
uscode/appendix_28a/level_id2e47c0a6-b17c-11ef-b971-e82c9e4f66ce/title_I/level_1
```

#115 replaced that with the publisher's own name for the container: the `identifier` attribute reduced to its last segment, and where there is none, the heading. No uuid segment remains in a dataset built from the committed release points, and Rule 1 of the Federal Rules of Civil Procedure is now at `uscode/appendix_28a/level_Civil/title_I/level_1`.

The dependence is smaller but it did not go away, and where the segment comes from a heading it took a new shape. **6,962** paths across the two committed release points sit at or below a container named by its heading, in thirteen works, the largest groups being the title 11 appendix, title 29, the title 28 appendix and title 38. A heading is stable against a sibling being inserted above the container, which is why it was preferred to numbering by position, but it is not stable against the publisher rewording it. That is the same class of movement this ADR already describes for `Redesignate` and `Move`, and the same answer applies: a locator moves, and only a minted identity does not.

Legal sources often supply no usable identifier, so we derive a structural path from the hierarchy the parser found, such as `uscode/title_26/subtitle_F/chapter_61/subchapter_A/part_III/subpart_B/section_6041/subsection_d`. This works well against messy documents and we keep it. But a derived path is an address, and an address changes. `AmendingAction` already contains `Redesignate` and `Move` (`src/uslm/mod.rs`), which are exactly the operations that keep a provision alive and move it. A path also changes when our own parser improves.

We therefore give each provision a stable identity that is carried across versions, and we treat the structural path as a locator at one point in time. Links point at the identity and record the locator beside it.

> **Not built, and superseded.** The identity in that paragraph does not exist in the code, and #93 decided against minting one; a link points at a path, and a *second* link says where that path moved to. The second half — the path as a locator — is built. See Implementation status above.

## Consequences

Without this, three things break quietly. A `Move` reads as a delete plus an add, so the diff reports a change to law that did not change. Human-confirmed annotations, which point at path strings today (`ChangeAnnotation.paths`), silently attach to different text after a parser change, and the party who receives the file cannot detect it. And "is this the same provision as last year", which is the question a legal researcher actually asks, has no answer.

> **The first and third are answered, by links rather than by identity.** A redesignation is stored as a `legislature.redesignated_as` link, the diff consults those links before it pairs by position, and "is this the same provision as last year" is walked out of them (#93). The second — an annotation attaching to different text after a parser change — is not: a link still names a path, and a parser change still moves it silently.

A locator is not only stale across versions; it can be ambiguous within one. The law sometimes numbers two provisions alike — `26 U.S.C. § 45X(d)(4)` is two paragraphs (4), which the Code footnotes as "So in original" and the official site renders in full — so one path names both. Code that treats a path as unique within an expression does not fail loudly. It picks one and discards the other, which is the confident false statement this decision exists to avoid.

Where a path names more than one provision, the document's order decides which is which. We inherit the order the source gives, pair provisions by position, and record the position explicitly rather than leaving it implicit in a parse. A parse keeps the order; a round trip through storage or a reserialized report is where it goes missing, and it goes missing silently. The alternatives to position are worse. Pairing by content similarity invents a judgement the source did not make, and treating a collision as an error refuses to hold law that genuinely exists.

Position pairing has one limit. When the number of provisions at a path falls, it assumes the survivors are the leading ones. If the source instead drops the first and keeps the second, the diff reports a large text change plus a removal, rather than the removal that happened.

> **The corpus does show this, and this ADR looked for the wrong shape.** It searched for a swap of two provisions *sharing one path*, found none, and concluded the case was hypothetical. The real shape is a renumbering: `119-hr-1` struck `26 U.S.C. § 898(c)(2)` and renumbered (3) as (2), so paragraph (2) after the bill is paragraph (3) before it, and the diff reported (2) as rewritten and gaining children while (3) read as simply gone. Both statements are false. The corpus holds 57 `redesignate` actions.
>
> **What fixes it is the redesignation link, not an identity.** Where a link says a provision was renumbered, the diff pairs across the renumbering and reports the displaced provision as removed (`TreeDiff::from_nodes_with`, `TreeDiff::moved`). Where no link is known, position is still what we have, which is why the `§ 6724(d)(2)` case below keeps position pairing.

The corpus's other example is genuinely unresolvable, and keeps position pairing. At `26 U.S.C. § 6724(d)(2)`, two subparagraphs (JJ) became one between 2025-07-18 and 2025-07-30. The survivor is the first, so position pairing happens to be right, and nothing in the source says which of the two it is — no bill states a redesignation there, so there is no link to consult and no honest answer beyond position.

The cost of the edge, rather than an identifier, is that continuity has to be walked rather than read. That is a projection, computed on demand, and it is the price of the property that matters: adding a bill later adds an edge instead of rewriting an identity (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).

## Considered and rejected

**The path is the identity.** Simplest, and wrong in a way that produces confident false statements rather than errors.

**The path is the identity, plus an alias table of renames.** This puts the truth in a side table. A reader that does not consult it gets the wrong answer, and no other implementation of the format is obliged to consult it.

> **A redesignation link is not that table, and the difference is the point.** An alias table is a private index beside the data. A link is the data: `docs/adr/0002-links-live-in-the-core.md` requires every reader to see a link, read its subject and object, report its verification state and preserve it when it writes the file again, whether or not it understands the kind. So a reader that does not walk `legislature.redesignated_as` still *sees* it and can say a renumbering was recorded. It gets a coarser diff, not a confident wrong answer, and it can tell which it has.
>
> The link also carries provenance naming the bill that said so, which an alias table has no place for. A rename with no source is an assertion nobody can check.
