# Evidence is stored once, and never deleted

Status: accepted

`CONTEXT.md` defines provenance as the source, the method, the evidence, and the verification state of a statement. Three of those survived an LLM extraction. The reasoning did too — the model's own explanation is parsed out of its reply and kept — but the reply itself did not. `classify` and `extract_changes` both used the raw text inside an error message and then dropped it.

That left a machine's claim uncheckable. A party receiving a W2D file could see that a statement was `MachineSuggested`, but not what the machine said, so "never lie" rested on a label rather than on anything they could inspect. It also meant no recorded reply existed to test the parser against, and a reply reconstructed from stored results is well-formed by construction, so it proves nothing about what models really send.

We therefore keep the verbatim reply. This ADR records how, because the choices are expensive to change once datasets carry them.

## A reply is stored once, under the hash of its own text

One reply produces many statements. The first recorded sweep wrote 1,237 replies behind 893 links, and a single reply routinely answers for several annotations at once.

So the reply is content-addressed and referenced, not copied. Putting the text on each statement would be wrong before it was wasteful: it would say each statement had its own reply, which is false. The same rule already names amendments and links, so the mechanism is not new, and it means a rebuild that restates a fact does not accumulate copies of the evidence behind it.

The reference lives in `Provenance.evidence`, which becomes a type rather than a string: the maker's reasoning, the reply's id, the model, and a hash of the prompt. Evidence is one of the four parts of provenance and deserves a shape, rather than one string with three loose relatives. It also keeps the degraded case readable — a statement made before replies were recorded still carries its reasoning in the same field.

## The prompt is hashed, not stored

A verifier wants to see what the model was asked as well as what it answered, so storing the prompt is tempting. We store its hash instead.

The prompt is built from material the dataset already holds — the amending text, the candidate diffs — so keeping it duplicates the file's own contents. It is also the larger of the two by some margin: a user prompt carries an amendment plus every candidate, while a reply carries an answer.

The hash still answers the question that decides whether a reply can be trusted: is the prompt behind it the one this build produces now? The case it does not cover is a build whose prompt construction changed. You then know the hash differs but cannot see the old text. We accept that; the alternative pays for every prompt to serve the rare audit.

## Evidence is append-only

A statement can be superseded. A link is identified by what it says, so restating a fact replaces the link and can leave its old reply referenced by nothing.

Those replies are kept. Deleting one because the statement it supported was superseded destroys the trail this decision exists to create, and it interacts badly with the rule in `docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md` that a human verdict survives a machine re-write: the superseded machine reply is how you would audit that a human's judgement was the one that stood. An orphan is not garbage. It is a record that something was said.

## Consequences

The evidence store is **core**, not part of an extension. A reader that does not know the legislature must still be able to see what a claim was based on, or the verification state it can read is a label with nothing behind it.

`BillAmendment` gains a provenance. `extract-changes` had none at all: the amending text is parsed from the bill and is a fact from a source, but the word-level changes are a model's reading of it, and nothing said so. Attaching evidence there without a verification state would have read as corroboration for a machine's guess, which is worse than recording nothing.

A dataset grows by the size of its replies. The first sweep added 1,210 KB against a 1.9 GB dataset, so the proportion is small, and it is not optional: evidence you can switch off is evidence you will not have on the run that mattered.

**A reply is only recorded when it parses.** Both commands still log a parse failure and drop the reply, so the malformed output this decision most wants to capture is the one thing it does not. That is a gap in the implementation rather than in this decision, and it is tracked separately.

## Considered and rejected

**The reply inline on every statement.** Simplest, and wrong about what a reply is: it claims each statement had its own. It also stored the text once per link rather than once per reply.

**A sidecar file outside the dataset.** Cheap, and it defeats the purpose. Evidence that does not travel in the W2D file cannot be checked by the party the file was sent to, which is the only party that needs it.

**Storing the prompt alongside the reply.** The most complete record, and it duplicates data the dataset already holds while roughly multiplying the storage cost of evidence. Revisit if prompt construction ever changes often enough that the hash stops being informative.

**Reference-counting replies and deleting the orphans.** Keeps the store tidy. It also throws away the record of a claim that was later revised, which is exactly the history an auditor asks for.
