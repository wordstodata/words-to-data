# Dataset completeness

A Dataset does not record whether it is complete, and no reader may ask it to prove that it is.

## The maintainer's position

> Honestly we don't and will never know if the dataset is complete. And the dataset doesn't need to intrinsically prove it.

## Why this is out of scope

The model already speaks about exactly two things, and completeness is neither.

**What it holds.** `Scope::held` is derived from the contents: every work, with the dates it was published. `Coverage` answers a question about that, so a reader gets "out of scope" instead of "not found".

**What a producer said it would hold.** `Declaration` carries `intends`, `excludes` with a reason each, and a date range that is reported rather than checked. `Coverage::Gap` is the difference — declared, not excluded, not held — which is a fault in the build measured against a statement somebody made.

Both are statements we are in a position to make. Completeness is not. The material a Dataset is drawn from is published by third parties on their own schedule, and nothing in a file can establish that no further printing exists. A field claiming it would be an assertion we cannot support, and it would decay: a work recorded as holding two of forty-seven printings is wrong the moment a forty-eighth is published, and nothing in the Dataset would know.

## What was proposed, and why each was rejected

The request came from #146, which wanted a reader to tell a work published once — a court opinion — from a work whose other printings were not fetched.

**Record what the source offered.** The mirror index lists every release point it carries, and `build-dataset` reads it and keeps only what it fetched. Rejected on two grounds. The index is **our own mirror**, `https://wordstodata.com/mirror/uslm/index.json`, so the record would be a claim about a publisher's output made on the strength of our cache. And the number is stale as soon as it is written, for the reason above.

**A flag on the work**, or a property set at ingest. Rejected because it is an assertion we cannot support, whatever sets it. `docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md` draws the line between a record and a judgement, and this is a judgement wearing a record's clothes.

**Per document class** — an opinion is complete by construction, a US Code title never is. Rejected in #146's own words as "tempting and wrong: it hard-codes a fact about a publisher into a document class, and the next class will not fit". It also makes the core reason about the contents of a node-type string, which #129 made an open string precisely to prevent.

## The question it was supposed to answer, and what answers it instead

#146 claimed this distinction was what made #145 — the temporal coverage question — hard to answer. **That claim is wrong**, and the code shows it.

An opinion's expression is keyed by the day it was filed:

```rust
Expression { id: ExpressionId::new(work, date_filed), … }
```

So the question "can this Dataset tell me what the law was on date X" is answered by a single test — **do I hold an expression at or before X** — and that test gives the right answer for both kinds of work without knowing which kind it is:

| question | held | at or before? |
| --- | --- | --- |
| what did this 1974 opinion say | the opinion at 1974-05-13 | yes, covered |
| what did § 174 say in 1974 | title 26 at 2025-07-18 | no, not covered |

A work that is never republished needs no flag saying so. Nothing ever adds a second expression to it, and the test does not care.

So: **everything may have revisions, and opinions simply never get a second one.** Nothing to declare, nothing to derive, nothing to go stale.

## Prior requests

- #146 — "One expression cannot say whether the work was published once or that is all we fetched" (2026-09-12, closed 2026-09-22)
