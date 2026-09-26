# A review is its own link, and a reader reports the record

Status: accepted. Not yet built — #227 builds it.

A dataset can hold a link that is wrong, and until now it had no way to say so.

The mechanism to say it was already there and complete. `VerificationState` carries `Asserted`, `MachineSuggested`, `HumanConfirmed`, `Disputed` and `Refuted`. `Link::from_annotation` maps every `AnnotationStatus` onto one of them. `LinkWriter::add_link` keeps a human-touched provenance alive so re-running the pipeline cannot quietly destroy a review (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`). `inspect` refuses to follow a `Refuted` step, and `path` prints each link's trust in words.

What was missing was a door. `match-amendments` is the only command that mentions `AnnotationStatus`, and it mentions it once, to hard-code `Pending`. Nothing could set a status on a link that already existed, so every stored link read *machine suggested, unconfirmed* for ever.

The cost of that was measured on #223. A dataset matched over both of its windows held 842 annotations in `2025-07-18 -> 2025-07-30` and 169 in `2025-07-30 -> 2025-08-14`. Of the links naming a section in their own amending text, 5 of 114 named a different section than the path they were filed under in the first window, and 7 of 24 — 29% — in the second. That second window cannot hold `119-hr-1`'s changes at all: #218 measured that every one of them appears by `2025-07-30`. Matching it anyway filed `Section 7701(a) is amended by adding at the end` under §48E, twice. `contradictions` surfaced 13 duplicated and 20 disagreeing subjects, and `validate` still exited 0.

Roughly fifty links were known to be wrong, and the file could not carry that knowledge.

## The decision

**A review is its own link. The link it reviews is never touched.**

An objection is a thing someone said, and `docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md` makes a thing someone said a record. Expressing it by rewriting the link argued with would destroy the argued-with statement, which is also a record. So a review is a new link, with its own source, evidence, reasoning and time.

This is what makes a bad review safe. Several reviewers may argue about one link and **every argument survives**, so a poor suggestion is *visible* rather than destructive. A review that rewrote the link's own state would let one weak reviewer overwrite a good judgement, and the loss would be invisible.

### The identity is the reviewer, not the verdict

A link is identified by its subject, its kind and its object, hashed (`docs/adr/0004`). A review is therefore shaped so that two reviewers of one link are two records:

| part | value | why |
| --- | --- | --- |
| subject | the reviewed link's own subject, copied | A **locator**. `links_for_path` and `links_for_pair` already read the promoted subject columns, so a review comes back beside the link it reviews with no new query and no new column. |
| kind | `review.confirmed`, `review.refuted`, `review.disputed` | Hashed, so a reviewer who changes their mind leaves **both** records. In the payload it would overwrite the earlier one, because a payload is not hashed. |
| object | `External { reference: "review.argument:<link-id>:<arguer>" }` | The **identity**. It names the link reviewed and who reviewed it. |

The reviewer must sit in one of the three hashed parts, and this is the part that is surprising without the context. Provenance is deliberately not hashed, so a reviewer named only there cannot separate two reviews. Two reviewers writing the same subject, kind and object mint one id — and `add_link`, finding a stored human-touched provenance, returns `Ok(())` and drops the incoming link. **The rule that protects a review would silently eat the second review.** A shape that looks obviously right is therefore wrong, and it fails in the worst available way: quietly, and only when two people bother to disagree.

The mirrored subject repeats what the link id in the object already implies. That redundancy is deliberate and one-directional, on the same terms `docs/adr/0004` sets for the promoted columns: **the reference is the identity, the subject is an index.** Nothing reads the subject as the truth about which link is under review.

### The newest record wins, and the program ranks nobody

Writing a verdict is mechanical, and anyone can do it. So no verdict is reserved to a kind of reviewer: an agent may refute and a human may dispute. `provenance.source` records *who* — `human:jesse`, `model:local` — and how much a given reviewer is trusted is a matter of who is allowed to run the command, not a rule compiled into the program.

Among the reviews naming one link, the newest by `provenance.timestamp` is the one a reader reports. A reviewer overrides by publishing over it. A review record must therefore always carry a timestamp, and one without a timestamp is refused, because it cannot be ordered.

This is the same restraint `docs/adr/0010` applies to a model and #179's decision 17 applies to a window: the machinery narrows and records, and it does not mint a judgement nobody made.

### A reader reports the record, not a collapsed state

`VerificationState` is untouched by this decision, and the door never writes it.

- `path` prints the winning review with its source and its date: *machine suggested, confirmed by `human:jesse` on 2026-09-26*.
- `inspect` gains one question, which is the only boolean it needs: **is the newest review of this link a refutation.** It keeps its existing refusal of a stored `Refuted`, because the mapping from `AnnotationStatus::Rejected` is still live and still true.

This is why no new verification state was needed. A human confirming and a machine confirming are genuinely different, and the difference was never a *state* — it is *who said it*, which is provenance. Reading the record keeps that difference without widening the enum.

## Considered and rejected

- **`Target::Link(id)`, a fifth target variant.** The cleanest identity by some margin: a review would point at a link directly rather than naming it inside a string. It moves the stored format, so `SCHEMA_VERSION` goes up, and it needs a new promoted column and a new `LinkReader` query, because no promoted column holds a link id. Revisit it when a break is being taken for other reasons, as the `Target::Node` rename of #147 rode the break of #182 rather than buying one of its own.
- **A mirror with no reviewer in the identity.** Collapses, as above.
- **Reserving `Refuted` to a human.** Rejected because "human" is a poor proxy for "trustworthy", and a stronger agent must be able to refute a weaker one definitively.
- **Declared supersession**, where a review names the review it overrules. It gives a definitive win on the record, and it is a precedence rule in the program, which is the thing being kept out.
- **A configured trust order** over reviewers. It expresses the real intent and brings a policy file, precedence rules and its own failure modes.
- **A per-link delete, or a relabel.** `LinkWriter` has one method, `add_link`, and `docs/adr/0005-evidence-is-stored-once-and-never-deleted.md` is append-only by name: *"An orphan is not garbage. It is a record that something was said."* A relabelled review is a different link anyway, because the kind is hashed, so relabelling means adding and deleting. Overriding replaces the reading, never the record.
- **A `MachineConfirmed` state.** An old reader meets `machine_confirmed` and cannot parse it, so one word would cost a release window.

## Consequences

**`SCHEMA_VERSION` does not move.** No target variant, no verification state, no rename, and no change to `add_link`'s merge rule. This decision is expressed entirely in the kind, which `docs/adr/0002-links-live-in-the-core.md` made an open namespaced string precisely so a party could add a link type without permission. The `review` namespace is the first use of that freedom by this repo on its own behalf.

**`VerificationState::is_human_touched` is not exercised by this door.** It stays, and it stays correct, because the legacy annotation mapping still writes those states. But the protection it provides is no longer the thing protecting a review: a review is a separate link of a kind the pipeline never writes, so the pipeline cannot reach it.

**A review does not survive a build from scratch.** `build-dataset` creates a dataset with an empty links table, and a review is a link, so a full rebuild discards every resolution. `add-release-points` grows a dataset in place, so a review survives that. This is accepted for now and recorded as open on #179: a changed method is exactly what forces a full rebuild, and it is also exactly when a reviewer's work is most worth keeping.

**A wrong verdict is corrected by publishing over it, and there is no way to remove it.** That is the intended trade: the record of a mistake is itself a record.

**Deriving a contested reading is left open.** Where one reviewer confirms and another refutes, a reader today reports the newer one. Pointers between reviews would let a *disputed* reading be derived from the pair instead. The records already hold everything such a rule would read, so this can be added later without touching what is stored. It is deliberately not built: one agent and one human working together do not need it.
