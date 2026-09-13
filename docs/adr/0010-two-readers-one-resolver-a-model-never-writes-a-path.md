# Two readers, one resolver: a model never writes a path

Status: accepted. Built in part — see ADR 0009, **Order of work**.

A bill states its renumberings in English: "by redesignating subparagraphs (H) through (U) as subparagraphs (I) through (V), respectively". A rule reader turns those words into two structural paths. Over `119-hr-1` it places 44 of the 57 statements the bill makes, as 89 links, and reports the other 13 with a reason each.

Those 13 are not noise. Three name a *part* rather than a section, and the resolver starts at a section. Four rely on the bill's own "Amendment of 1986 Code" convention with nothing in the markup to confirm the title. Three amend a table of sections. One writes "subparagraphs (R) through (V)" on one side and "paragraphs (S) through (W)" on the other, so no series enumerates. A stricter regex will not read them; reading them is a language problem.

So there are two readers. This ADR is about what each is allowed to do.

## The decision

**The model reads words. The resolver makes paths. The model never writes a path.**

A model is asked about one clause and returns a payload in the same shape the rule reader produces: the section it read, the container steps below it, and the pairs of old and new designations. That payload goes into the same `resolve` the rule reader's output goes into, and is subject to the same guards: every container step must match exactly one child, the old path must exist in the earlier expression, the new path must exist in the later one, and the counts on the two sides must agree. A payload that fails a guard becomes an unplaced statement, with the model named as the reader that failed.

**The model reads only the residue.** It is asked about the statements the rule reader could not place. Where the rules place a statement, they are cheaper, reproducible, and already checked against both documents.

**A deterministic reading is corroborated by the words at both ends, and this is the stronger half of the guarantee.** Take the M → N step of the run quoted above:

```text
subparagraph_M   2025-07-18 -> 2025-07-30
  content: "Section 1396a(ff) … disregard of certain property …"
        -> "Section 1396a(bb) … Federally-qualified health centers …"

subparagraph_N   2025-07-18 -> 2025-07-30
  content: "Paragraphs (2), (16), and (17) of section 1396b(i) …"
        -> "Section 1396a(ff) … disregard of certain property …"
```

Old M and new N hold one sentence, character for character. That identity is recorded as `Corroboration`: method, the figure over the node's own words and over its subtree, and the parts in `detail`. A party holding the same two release points recomputes it and gets the same answer, which is what `CONTEXT.md` requires of corroboration and what a `raw_score` can never be.

Note what the same example says about existence alone: `subparagraph_M` and `subparagraph_N` both exist on **both** dates, because the whole run shifted by one letter. In a shift run the paths prove nearly nothing. The words are what tell a correct reading from a misread.

**Neither reader raises the verification state.** Every redesignation link is `MachineSuggested`. The bill asserted the *renumbering*; it did not assert our two paths, and the paths are the statement the link makes. `CONTEXT.md` is explicit that corroboration says where a reviewer should look first and nothing more.

**A low figure does not refuse the link.** A bill often renumbers and rewrites in one breath, so different words are not evidence against a renumbering. `MachineSuggested` plus a low figure is the honest record: the bill said this, and the words do not back it up.

**A rule reading is derived; a model reading is stored.** The asymmetry is the point. Once the bill is a document (ADR 0009) a rule reading is reproducible from the dataset, so storing it would restate what the file already holds. A model reading is not reproducible — run it again and it may differ — so it is stored with the reply that produced it, or `Evidence` is a label with nothing behind it. `BillAmendment.changes` plus its provenance is already this pattern.

## Consequences

**One code path makes every path in the dataset.** A misreading by either reader fails the same four guards, and a path that reaches a link was checked against real law whichever reader proposed it.

**The two readers cannot disagree,** because they never read the same statement. If that changes — a maintainer re-running the model over everything — `VerificationState::Disputed` is the word for the result, and nothing here needs to change to allow it.

**The model pass stays a separate command.** It costs money, so it must be re-runnable without rebuilding, and its replies are cached beside the dataset as `extract-changes` does (#123 records what happens without a cache).

**A reviewer gets a queue rather than a pile.** 89 links sorted by corroboration puts the doubtful handful at the top. The report that does this stays narrow on purpose: the same "what this build could not use" hole exists for declined citations (#140, 51,721 per release) and for dropped parser elements (#110), and a surface that must also hold fifty thousand rows is a different design. Get it right at 13 rows first.

## Considered and rejected

**Let the model return paths.** Shorter, and it makes the model's output unfalsifiable in the one way that matters: a path is the statement the link makes, so a model that writes paths writes links, and our only check would be that the string looks like a path.

**Run the model over everything, with the rules as a check.** Doubles the spend to measure agreement on the 77 percent the rules already place with both ends verified.

**Refuse a link when the two ends' words differ.** Tempting after the M → N example, and wrong: it would delete the record of what the bill said whenever a bill renumbered and rewrote at once, which is common. The figure is recorded so a human can judge; it is not a gate.

**Treat a fully guarded rule reading as `Asserted`.** The bill is a source and it did say so. Rejected: it would make a parser's output indistinguishable from a publisher's fact, and the resolution — which section, which container, which path — is ours, not the bill's.
