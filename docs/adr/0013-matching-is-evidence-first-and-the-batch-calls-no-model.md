# Matching is evidence-first, and the batch calls no model

Status: accepted. Not yet built — the tickets under #179 build it.

The removal in **What this removes** was made in #252.

Elimination in stage 4 gives an amendment only a change its own words can have made (#274): a change in a unit its words name, and of a kind its action makes. A strike brings no new words, and an addition removes no provision.

The window rule in stage 3 applies to renumbering too (#172): the redesignation step records a statement in the first window after the law's enactment in which the text under the statement's container changed, and names a later such window for review.

Matching asks which change to the US Code each amendment of a public law made. Until now it was answered by content. `extract-changes` asked a model for the words each amendment removes and adds. `score-amendments` scored those words against every changed path in a title with a precision-weighted F1 and dropped scores at or below a cutoff. `match-amendments` asked a model to choose among what was left. `scan_for_mentions` added paths whose section a regex found anywhere in the amendment's text.

The method was chosen arbitrarily, and #179 said so from the start. On 2026-09-27 it was measured against the maintainer's real dataset, which holds `119-hr-1` (Public Law 119-21, 603 amendments), and it failed in two ways.

**It left real changes unmatched.** 168 of 603 amendments, 28%, have no link. A spike gave an agent the CLI alone and 20 of those amendments. It resolved all 20 — 15 found, 2 unchanged, 3 in material the dataset does not hold — in about 45 tool calls. Every found row checked by hand was right.

**It matched confidently to the wrong provision, and each error was the same error.** Content is drawn to what an amendment *mentions*, and an amendment mentions the provisions it cross-references:

| the amendment says | the link points at | why |
| --- | --- | --- |
| `Section 5000A(d)(3) … is amended` | §36B | the inserted words cite §36B |
| `Section 1371(d)(1) … is amended` | §50(a)(5) | the struck words quote §50(a)(5) |
| `Section 7701(a) is amended` | §48E | |
| a new §174A | §263(a)(1)(B), at confidence 0.85 | a sentence inside §174A names §263 |

The last one hid the single change that mattered most to the question it was asked about, and it also left §263's real amendment unmatched, so neither gap looked like a gap.

## The evidence was already in hand

Three sources say where an amendment acts, and none of them was the matcher's input.

- **The publisher's markup.** A bill's `<ref href="/us/usc/t42/s1396a/a/10/A/i/VIII">` resolves each citation exactly, and the parser keeps it as `UscReference`. The redesignation step already reads it through `stored_section_under_amendment`, which climbs the enclosing clauses so that *"(A) by striking"* inherits the section its parent names. Run over every amendment of `119-hr-1`, it named a section for **495 of 603 (82%)** in one second, and it was right on every hand-checked row it answered.
- **The Office of Law Revision Counsel's classification tables.** For each section of a public law they state the Code section it was classified to and the kind of change: new, note, amended, repealed. They resolve to a section and no lower, and absence from a table is not absence of a change. They cover the markup's blind spots — a new section, and a note the dataset does not hold.
- **The diff, in the window the law lands in.** The law's own `<approvedDate>` is stored as the date of its expression (`publiclawdocument_119-21@2025-07-04`). In the first window after it, 274 of the 495 addressed amendments have exactly one change under their address, and 239 of those are claimed by no other amendment. In the second window, 406 of the 495 have no change at all.

## The decision

**Matching resolves the address from evidence first, finds the change under it in the window the law lands in, and calls no model. What it cannot resolve is left for an agent, with the reason it stopped.**

1. **Classify.** Each row of an OLRC classification table is stored as a link: the Code section as subject, `olrc.classified_from` as kind, `olrc.classification:<public law>:<section>` as object, `Asserted`, with the kind of change in its payload. A published classification is a statement by an authority, so it is a **record** (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`), and a link in a namespace of its own is where `docs/adr/0002-links-live-in-the-core.md` puts one. A new link kind is not a stored-format break.
2. **Address.** The markup resolver answers for every instruction, not only renumbering: section, the designations below it, and the scope phrases (*"in paragraph (2)"*) that narrow it. The OLRC link corroborates it or contradicts it, and a contradiction is reported.
3. **Window.** The first window after the law's enactment date in which the address actually changed. A release point's publication name (`Online@119-27not21`) is a hint and is not trusted on its own. A later window where the same address also changed is a candidate for review, never a second link.
4. **Resolve.** One change under the address is linked. Several are told apart by the words the markup quotes — `<quotedText>` struck and inserted, `<quotedContent>` enacted — which a change's before and after text either contain or do not. The changes under one section are assigned together, so one amendment does not take another's change. A provision edited in place can carry the edits of several amendments, and each that shows words of its own in it is one of its causes (#259). Words that join clauses — "and", "or", "the" — decide nothing alone unless they are all a change struck or inserted (#259). Word overlap only ranks what the quoted words leave tied. A link points at the changed path; the address is its evidence.
5. **Residue.** Every amendment the batch did not link, with the stage and the reason it stopped, is **derived** on demand and never stored: it goes false the moment someone links it.

**An agent works the residue through a door that cannot invent a path.** A command records an `amended_by` link from an amendment id, one or more paths, a window and a written reason. It refuses any path that did not change in that window. The link is `MachineSuggested`, its source names the agent, and its reasoning is its evidence, so the review loop of `docs/adr/0012` covers it unchanged. This keeps `docs/adr/0010-two-readers-one-resolver-a-model-never-writes-a-path.md` whole: the agent chooses among changes the diff produced, and the command enforces it.

**Only public laws are matched.** A bill that has not been enacted changes nothing, and predicting what a pending bill would change is a different problem, deferred.

## What this removes

`extract-changes`, `score-amendments`, `match-amendments` and `scan_for_mentions`, once the maintainer has compared the new route with the old on `119-hr-1`. The comparison is information for that judgement, not a gate. A dataset rebuilt afterwards holds links from the new method only; a file built before keeps its old links, because evidence is never deleted (`docs/adr/0005-evidence-is-stored-once-and-never-deleted.md`).

Three readers of "which section does this amendment name" existed at once — `scan_for_mentions`, the prose reader in `section-agreement`, and the markup resolver — and they disagreed. One remains.

## Considered and rejected

- **Keep a model for the ambiguous cases.** Every confirmed error came from a model reading content, and a batch with no model is reproducible. The quoted words and the one-change-one-cause assignment cover most of what a model was guessing at, and an agent covers the rest with reasoning a reviewer can read.
- **Keep the old pipeline as a fallback.** It keeps the question of which reader is authoritative, which is how three readers came to disagree.
- **Ask a model to extract the cited section.** A model supplies a codification from memory where the text gives none, and a wrong section that exists passes any check that it exists. The publisher has already resolved the citation; the markup is read instead.
- **Sweep every window.** The generalized sweep answered a question the law's own date and the diff already answer, and matching `119-hr-1` against a window after its changes landed produced the false links #218 measured.
- **Fetch OLRC tables at match time and store nothing.** A classification is a statement by a party, and the dataset keeps what was said.
