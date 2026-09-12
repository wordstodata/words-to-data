# A record is what was said; everything else is derived

Status: accepted

`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md` states a rule about one table:

> The promoted columns are a denormalised index of the value, which is two places holding one fact, and ADR 0002 warns against exactly that. It is deliberate here and it is one-directional: the JSON is the record, the columns are derived from it on write, and nothing reads a promoted column as the truth.

That rule is right and its scope is too small. The same pattern runs through the whole model, unnamed, and where it is unnamed it goes wrong in a specific way: a change to a *derivation* is indistinguishable from a change to the *format*, so both are announced with a schema version and a reader cannot tell which happened.

We therefore name three categories, and one rule per category.

## The three categories

**A record is something a party said.** A link, its provenance, a model reply, a declared scope, a bill's amending text. It is not recomputable from anything else in the dataset, because it is a statement rather than a consequence. Losing it loses information.

**An index is a derivation we store because recomputing it is too slow.** The promoted link columns, `element_index`, a structural path in a stored node. It is recomputable from the records, and where the two disagree the record wins. Storing it is a performance decision and nothing else.

**A projection is a derivation we do not store at all.** `ChangeAnnotation`, which `docs/adr/0004` turned into a projection out of links. `Scope.held`, computed by `Scope::derive`. A provision's continuity, which `#93` answers by walking redesignation links rather than by minting an identity.

A fourth thing looks like an index and is not. `Corroboration` is reproducible by a party holding the same texts — that is what makes it evidence rather than a claim — and it is stored anyway, because the point is for a receiver to check our arithmetic rather than take it on trust. **A derivation stored so it can be disputed is a record.** The test is not "could this be recomputed" but "is it something we are asserting".

## The rule

Records are the truth. An index is written from records and never read as the truth. A projection is computed on demand. Nothing derived may be the only copy of anything.

## What naming this buys

It separates two things that a schema version currently conflates. Today this repo has exactly one signal for "your dataset is out of date", and it fires for three different reasons:

| Change | What actually broke | Honest signal |
| --- | --- | --- |
| `#129`, the node type | the **format**. An old reader cannot parse the file at all | refuse the file |
| `#115`, readable container paths | an **index**. The file parses; its stored paths are stale | rebuild, tooling unaffected |
| `#113`, the appendix parser | a **record**. The source parse changed, so the dataset holds different facts | rebuild, tooling unaffected |

All three took a `SCHEMA_VERSION` bump or wanted one. Only the first deserved it. The second and third produce a file an old reader can read perfectly — it is simply stale — and refusing it overstates the problem while saying nothing about the actual cause.

`docs/agents/triage-labels.md` has the matching gap: it defines `breaking-changes` as a schema change, a serialized-type change, or a CLI JSON change. `#113` was none of those and still forced every user to rebuild, so the label could not describe it.

## Consequences

**A derivation change stops being a format break.** It still forces a rebuild, and that has to be visible, but the reader gets "rebuild and carry on" instead of a refusal that implies its tooling is obsolete. What signals a stale index rather than an unreadable format is left open deliberately — it is a smaller decision than this one and it should be made when the first case arrives rather than invented here.

**A new derivation is cheap to add and cheap to change.** `#93` is the first issue decided under this rule: continuity became a projection over links instead of a stored identity, which turned a breaking change with a schema bump into an additive one with neither. That is the rule paying for itself on its first use.

**The burden lands on review.** Nothing in the compiler distinguishes these categories; a field is a field. So the question "is this a record, an index, or a projection" belongs in review of any change that stores something new, and the answer belongs in a comment where the field is declared. `docs/adr/0004` already does this for the promoted columns and is the model to copy.

**Storing an index is still allowed and often right.** This is not an argument for computing everything on read. `Scope::derive` exists because "asking what a dataset holds no longer costs the text of everything it holds", and `element_index` exists because `#109` showed that answering a path query by loading documents took three minutes. The rule governs which copy is authoritative, not whether a second copy may exist.

## Considered and rejected

**Leave it implicit.** The rule was already written down once, in `docs/adr/0004`, scoped to one table. It did not generalise on its own: within a week, three changes of three different kinds were all announced with the same signal, and a fourth (`#93`) was about to be designed as a stored identity before the distinction was noticed. An unnamed rule is one that gets rediscovered.

**Enforce it in the type system.** A newtype per category, so a projection cannot be persisted and an index cannot be read as truth. Attractive, and too heavy for the benefit: the categories are about *meaning*, and the same `String` is a record in one field and an index in the next. It would add ceremony to every field and still rely on a human choosing the right wrapper.

**Fold it into `docs/adr/0004`.** That ADR is about links. This rule governs nodes, scope, annotations and continuity as well, and burying a model-wide rule inside a decision about one table is how it came to be too small in the first place.

**Compute everything on read, and store only records.** The purest answer, and it is refuted by measurement in this repo: `#109` cut `validate` from three minutes to two seconds precisely by reading a stored index instead of deriving the answer from records.
