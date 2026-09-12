# Words To Data

The language of this project. `words_to_data` turns legal documents into data structures that a person or an agent can compare, link, and send to another party.

**What it is for.** The goal is to find the authority that controls a legal question. Nothing here does that yet. What exists today is the first step: finding what changed between two versions of a law, and which instruction in a bill caused each change.

**How to read this.** Every entry names one thing and lists the words to avoid for it. The sections build on each other, so read them in order: a term is defined before it is used, except where an entry names its own parts just before defining them, or where two concepts define each other and the first says "see". Where an entry describes something the code does not do yet, it says so in bold and names the issue.

## The task

**Legislative change tracking**:
Finding what changed between two versions of a law, and which instruction in a bill caused each change. This is the capability that exists today.
_Avoid_: Diffing, legal diff, change detection

## The dataset

**Dataset**:
A collection of legal documents and the statements that connect them (see Link, below). It has no fixed size: one title, or all of federal and state law.
_Avoid_: Corpus, database, collection

**W2D file**:
One portable file that carries a Dataset to another party.
_Avoid_: Export, dump, archive

**Scope**:
The statement of what a Dataset covers. A Dataset declares the scope it intends, and reports the scope it holds. A reader uses the scope to answer "out of scope" instead of "not found". The difference between the two halves is a Gap.
_Avoid_: Coverage, extent, contents

**Gap**:
Material a Dataset declared, did not exclude, and does not hold. A gap says the build did not do what it said it would, so it is a fault in the Dataset rather than a statement about the law. This is why it is a separate answer from "out of scope": one means incomplete, the other means out of its lane.
_Avoid_: Missing, hole, omission

**Exclusion**:
A hole a producer states, together with the reason for it. An excluded path is answered rather than merely absent, so it is not a Gap. The reason is part of the statement: a hole with no reason cannot be told apart from an oversight, which is the ambiguity the Scope exists to remove.
_Avoid_: Skip, filter, ignore

## The law

**Work**:
A legal document, or a part of one, treated as a concept with no date attached. Title 26 section 174 is one work, from 1954 until today.
_Avoid_: Document, section

**Expression**:
One work as it read on one date. A version of the law, not a version of a file.
_Avoid_: Version snapshot, revision, edition

**Provision**:
A unit of law that stays the same thing across versions, even when its text changes or it moves to a new address. Its identity does not depend on its location.

**The identity this definition needs does not exist yet.** Nothing in the code mints or carries one. A provision is reached by a Structural path plus a position within one Expression, which locates it and does not identify it, so "is this the same provision as last year" — the question a researcher actually asks — has no answer today. `docs/adr/0001-structural-paths-locate-not-identify.md` records the decision to mint an identity, and #93 is the work. Read this entry as what a Provision is meant to be, and Structural path as what the code has.
_Avoid_: Section, node, element

**Structural path**:
The address of an element, derived from the hierarchy that the parser found. It locates a Provision at one point in time. It does not identify one. Two provisions can share one path: the law sometimes numbers two provisions alike, and the document records both. A path and a position together locate one provision within one Expression. They still do not identify it.

Independence from the source document holds only where an element carries a number. A container that groups a body of law usually carries none: the Federal Rules sit in a `courtRules` element with no number of its own. Such a container takes its segment from the publisher instead — first from the `identifier` attribute, reduced to its last segment, and where there is no identifier, from the heading, which is the only name the publisher gives it:

```
uscode/appendix_28a/level_Civil/title_I/level_1
uscode/appendix_11a/level_federal-rules-of-bankruptcy-procedure
```

**This is common, not marginal.** 118 containers per release point carry no identifier and take their segment from a heading, and a dataset built from the two committed release points holds **6,962** paths at or below one, across thirteen works. Per release point: 2,363 in the title 11 appendix, 240 in the title 28 appendix, 396 in title 29, 143 in title 38, 132 in title 25, 99 in title 19 and 77 in title 12. Those segments are readable, but they are only as stable as the publisher's wording. Where the publisher rewords a heading, every path below it moves.
_Avoid_: Path, breadcrumb

**USLM ID**:
The official identifier of an element, as published in the source document. Many legal documents supply none, which is why a Structural path is derived rather than read.
_Avoid_: ID, identifier, reference

**Diff**:
The set of differences between two Expressions of one Work, in the shape of the document hierarchy.
_Avoid_: Delta, comparison, change set

## Bills

**Bill**:
A legislative instrument that changes existing law. It carries the instructions that do the changing, and once enacted it is published as a public law.
_Avoid_: Act, statute, law

**Amendment**:
An instruction in a Bill that tells a reader how to change existing law.
_Avoid_: Edit, modification, revision

## The legislature

These are facts about the people and the votes behind a Bill. Only some datasets carry them; see Extension.

**Chamber**:
One house of a legislature: Senate or House.
_Avoid_: Body, house, branch

**Member**:
A person who sits in a legislature, in one Chamber. The person does not change. Almost everything the source says about them is true only of a date.
_Avoid_: Legislator, representative

**Party affiliation**:
The party a Member held, with the years the source gives for it. A Member carries a history of these and never one undated party, because the affiliation is dated in the same way that an Expression is one Work on one date: the person is the Work, the affiliation on a day is the Expression. A vote carries a date, so the party reported beside a vote is the party held on that day. The source gives years and not days, and the year of a change belongs to two affiliations, so a date in that year is answered as unresolved. An unresolved affiliation is a different answer from a known one, and every reader can tell them apart.
_Avoid_: Current party, the member's party

**Roll call**:
A recorded vote of a Chamber on one question, holding each Member's position, the question as the Chamber put it, the result, and the date.

**Only House roll calls can be held.** The source publishes a House vote resource and no Senate equivalent, so a Senate vote is not merely absent from a Dataset — it cannot be represented by the types at all. An answer about how a Bill was voted on therefore covers one Chamber, and must say so rather than offer one Chamber as the whole story. A bill that passed both chambers will show its House concurrence and nothing of the Senate. #112 explores whether a Senate source exists.
_Avoid_: Vote, division, tally

**Vote position**:
How one Member answered one Roll call. The party reported beside it is the Party affiliation the Member held on the Roll call's date, never the one they hold now.
_Avoid_: Ballot, choice, stance

**Sponsor**:
The Member who introduced a Bill. Held as a bioguide id, so a Sponsor resolves to a Member and to their Party affiliation on any date. The field that carries it is named `sponsor` and reads like a person's name; it is not one (#104).
_Avoid_: Author, introducer, proposer

**Cosponsor**:
A Member who added their name to a Bill after its Sponsor, with the date they did it and whether they later withdrew. A withdrawal is recorded rather than removed: that support was given and then taken back is itself a fact about the bill, and deleting the record would state that the support never existed.
_Avoid_: Co-author, supporter, backer

## Trust in a statement

Every statement a machine made carries all of this, and all of it is core rather than owned by one extension. So a reader can always say who made a statement and how far it can be trusted.

**Provenance**:
The record of where one statement came from: its source, the method that produced it, when it was made, its Evidence, its Verification state, and the two numbers below.
_Avoid_: Lineage, history, audit

**Verification state**:
The trust level of one statement: `Asserted` by a source, `MachineSuggested`, `HumanConfirmed`, `Disputed`, or `Refuted`. `Disputed` means someone objects and it is unsettled; `Refuted` means it was checked and found wrong, which is settled.
_Avoid_: Confidence, score, accuracy

**Evidence**:
What a statement was based on: the maker's reasoning in their own words, and the verbatim reply that produced it. A machine's claim is only checkable if the party receiving the Dataset can see what the machine actually said, so evidence is what stops a Verification state being a label with nothing behind it.
_Avoid_: Proof, justification, backing

**Model reply**:
The text a model returned, kept exactly as it arrived. One reply usually makes several statements, so it is stored once and referred to, never copied onto each. It is never deleted, even when the statement it supported has been superseded: an unreferenced reply is a record that something was said. A reply that no parser could read makes no statement, so the Dataset does not carry it at all.
_Avoid_: Response, output, completion

**Corroboration**:
A deterministic measurement that supports a statement, carrying the method used, the figure, and the parts the figure was built from. A party receiving the Dataset can recompute it from the same texts and get the same answer, which is what makes it evidence rather than a claim, and the one number in a Provenance a reader may reasonably rely on.

It does not raise the Verification state. A machine's proposal that scores well is still a machine's proposal; corroboration says where a human reviewer should look first, and nothing more.
_Avoid_: Score, confidence, match strength

**Raw score**:
A number a model reported about its own work. It is kept as diagnostic data and nothing more. It is not a probability, it must never be presented as one, and nobody can check it: the model asserted it about itself, and running the model again may give a different number.
_Avoid_: Confidence, probability, certainty

## Statements about the law

**Link**:
A statement that connects one Provision to something else. It carries a subject, a namespaced kind, an object, and its Provenance. Every reader can read a link, even a reader that does not know the kind. Its identity comes from its subject, its kind, and its object, so restating the same fact updates one link rather than making a second one.
_Avoid_: Relation, edge, reference

**Kind payload**:
The facts about a Link that only the extension defining its kind understands, such as the amending action behind a link that records a change. The core stores it, hands it back unchanged, and never reads it. Nothing a reader needs in order to report a link may live here.
_Avoid_: Extra, metadata, blob

**Change annotation**:
The Link of kind `legislature.amended_by`. It connects one change in a Diff to the Amendment that caused it.
_Avoid_: Match, mapping, label

**Extension**:
A named set of facts that only some datasets carry, such as the legislature facts (Bill, Sponsor, Roll call) or the judicial facts (court, opinion type). The core data model carries no extension concept: a Link's kind is an open namespaced string, and the facts only one kind understands sit in a Kind payload the core stores and never reads.

The storage traits hold the same line. The core `Storage` trait requires documents, links and evidence, and no extension, so a backend that will only ever hold court opinions implements it without writing a word about bills. Code that needs legislature facts asks for them — `S: Storage + LegislatureReader` — and a dataset arriving from another party is asked at run time, through `Storage::legislature`, which answers `None` for "not a concept here" rather than an empty list (#127).
_Avoid_: Plugin, module, add-on
