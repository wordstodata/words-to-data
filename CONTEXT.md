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
_Avoid_: Corpus, collection

**Database**:
Where a Dataset is kept between runs and changed in place.
_Avoid_: Store, backend, working copy

**W2D file**:
A Dataset serialized to one file, to carry it to another party. A serialization, and not a place a Dataset lives (`docs/adr/0011-a-dataset-lives-in-a-database-json-is-serialization.md`).
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

**Document node**:
One piece of a document, with its children, as a Dataset stores it. It carries a Structural path, a Node type, a date, up to five text fields, and where its text came from. It carries nothing that belongs to one class of document: those facts sit beside it in a Class payload. An Expression holds the root of a tree of them.

A whole document is one node when nothing has taken it apart, which is ordinary rather than degenerate: a court opinion is stored as a single node, and its structure is in its text.
_Avoid_: Element, USLM element

**Node type**:
What kind of thing a Document node is, as an open namespaced string: `uscode.section`, `bill.section`, `judicial.opinion`. The namespace names the document class and the local half names the kind. It is open, so another party adds a document class without our permission, and a type this build has never seen is carried rather than dropped. The core reads the type — two nodes of different types are not one provision across two dates — and owns none of the vocabulary.

Three namespaces exist: `uscode`, `bill` and `judicial`. A Bill is `bill.*` and never `uscode.*`, because a bill is not part of the US Code. The root node of each class says what the whole document is: `uscode.document` for a US Code file, `bill.public_law` for a Bill that has been enacted, `judicial.opinion` for a court opinion. The two are not named alike on purpose — for the US Code, title-versus-appendix is already in the path and in the child's own type, while for a Bill nothing else says whether it is enacted, and a reader needs that in order to report it.

A segment of a Structural path uses almost always the same word, and the two document roots are the exception: a path segment is frozen, because moving one renames a Provision, while a type is an interface a person reads.
_Avoid_: Element type, document type, tag

**Class payload**:
The facts about a Document node that only its class understands: for the US Code an element's number and the publisher's identifier for it, for a court opinion the case name, the author and the reporters' citations. The core stores it, hands it back unchanged, and never reads it. Nothing a reader needs in order to report a node may live here, which is why how a node's text was obtained is recorded as Provenance instead. The same thing a Kind payload is for a Link, one level down; see Kind payload.
_Avoid_: Metadata, extra, blob

**Provision**:
A unit of law that stays the same thing across versions, even when its text changes or it moves to a new address. Its identity does not depend on its location. A Document node is how one is stored; a Provision is what one is.

**A provision has no identity of its own, and will not be given one.** Nothing in the code mints or carries one. A provision is reached by a Structural path plus a position within one Expression, which locates it and does not identify it.

"Is this the same provision as last year" is answered another way: by walking Redesignations. A provision has nothing stable to hash — its text changes, which is the point of tracking it, and its location changes, which is why identity was wanted — so #93 recorded the movement as an edge instead of minting an id. `docs/adr/0001-structural-paths-locate-not-identify.md` records the identity it recommended and the note that replaced it.

Read this entry as what a Provision is; read Structural path for how one is located, and Redesignation, under Statements about the law, for how two locations are known to be one provision.
_Avoid_: Section, node, element

**Structural path**:
The address of a Document node, derived from the hierarchy that the parser found. It locates a Provision at one point in time. It does not identify one. Two provisions can share one path: the law sometimes numbers two provisions alike, and the document records both. A path and a position together locate one provision within one Expression. They still do not identify it.

A segment is `<kind>_<number>`, and the kind is almost always the local half of the node's Node type, so `uscode.section` and `section_174` cannot disagree about what an element is called. The first segment names the document class: `uscode/title_26`, `judicial/opinion_2812209`.

**A path segment never changes, even where the Node type beside it does.** A Bill's root sits at `publiclawdocument_119-21` while its type reads `bill.public_law`: the type was renamed for a reader, and the path was not, because a root path is a Work id that a Link can point at.

Independence from the source document holds only where an element carries a number. A container that groups a body of law usually carries none: the Federal Rules sit in a `courtRules` element with no number of its own. Such a container takes its segment from the publisher instead — first from the `identifier` attribute, reduced to its last segment, and where there is no identifier, from the heading, which is the only name the publisher gives it:

```
uscode/appendix_28a/level_Civil/title_I/level_1
uscode/appendix_11a/level_federal-rules-of-bankruptcy-procedure
```

**This is common, not marginal.** 118 containers per release point carry no identifier and take their segment from a heading, and a dataset built from the two committed release points holds **6,962** paths at or below one, across thirteen works. Per release point: 2,363 in the title 11 appendix, 240 in the title 28 appendix, 396 in title 29, 143 in title 38, 132 in title 25, 99 in title 19 and 77 in title 12. Those segments are readable, but they are only as stable as the publisher's wording. Where the publisher rewords a heading, every path below it moves.
_Avoid_: Path, breadcrumb

**USLM ID**:
The official identifier of a Document node, as published in the source document. Many legal documents supply none, which is why a Structural path is derived rather than read. It belongs to one publisher's schema, so it sits in the `uscode` Class payload rather than in a core field.
_Avoid_: ID, identifier, reference

**Diff**:
The set of differences between two Expressions of one Work, in the shape of the document hierarchy. A child is reported as changed, added, removed, or **moved**. Moved is what a Redesignation buys: where one is known the diff pairs across the renumbering, and where none is known it pairs by position, which is all two documents say on their own.
_Avoid_: Delta, comparison, change set

**Uncovered period**:
A stretch of time in which a Dataset holds no Expression of a Work, so it can say nothing about what changed in it. It is the Scope in time rather than in space: "out of scope" for a period instead of for a Structural path.

It is the answer a question spanning dates needs. A dataset holding title 26 at two release points a fortnight apart can prove what section 174 did in that fortnight and nothing else, and a case construing the section in 1974 sits fifty-one uncovered years before the earlier of them. Reporting only the change that is visible would be true and misleading.

**The Scope cannot express this yet.** A Coverage is asked about a path, and answers `InScope` for a work it holds at any date at all, which is right and useless here. So the caller that needed the answer carries its own (`judicial::reliance`, #53), which means a second caller will word it differently. `docs/research/a-court-opinion-in-the-core.md` records why this is the more dangerous half of the mistake the Scope exists to prevent: nobody misreads "we do not hold title 42", and everybody misreads "this provision changed once".
_Avoid_: Missing dates, blind spot, unknown

## Bills

**Bill**:
A legislative instrument that changes existing law. It carries the instructions that do the changing, and once enacted it is published as a public law.

A Bill is a document, so it is a Work with an Expression, like a title of the Code or a court opinion. Its structure carries meaning that its words alone do not: an Amendment nested under "in subsection (a)--" is about a different provision from the same words outside it. A Dataset holds that structure: the Bill is a Work of its own, under the number its publisher gave it — `publiclawdocument_119-21` — with one Expression, dated the day the Bill says it was approved. It is published once, so there will only ever be one. The Dataset knows the same Bill by the id it was downloaded under, `119-hr-1`, and the two are joined by what the Bill says rather than by a name written down twice: every instruction in the document carries the Amendment hash minted under that id. `docs/adr/0009-a-source-is-parsed-once-a-bill-is-a-document.md` records the decision and the order of the work.
_Avoid_: Act, statute, law

**Amendment**:
An instruction in a Bill that tells a reader how to change existing law. It is identified by what it says, and located by where it sits in the Bill: the hash survives a rebuild, and the position moves whenever a publisher renumbers a title (`docs/adr/0001-structural-paths-locate-not-identify.md`). Both are held — the hash keys the Amendment, and the node at its path in the stored Bill carries the same hash, so one can be read from the other.

The words an Amendment **enacts** are not a Provision. They are quoted text, not law in force, and a Provision is a location an annotation can name, so they stay out of the hierarchy and travel in the Bill node's Class payload instead (#86).
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

A Document node carries one too, where it has something to say. There the method records **how the text was obtained** — read from the publisher's markup, from a text layer, or by a machine reading a scan. That is core rather than a Class payload fact, because a reader deciding whether to rely on a passage must not have to open a payload to learn that nobody has checked the words against the page. A node carries provenance only where it differs from its neighbours': every node of a US Code release point came from one publisher by one method, and recording that on each would state one fact a million times over.
_Avoid_: Lineage, history, audit

**Method**:
What produced a statement, named and given a version: `{name, version}`. The name says which reasoning it was; the version says which edition of that reasoning made this statement.

A person chooses the version, and raises it when the method's answers change. A build never raises it. A hash of the method's parameters was refused, because it moves on a cosmetic edit and a number that moves for no reason is a number everybody ignores (#179, decision 10).

Without the version, a method's name stays the same while its answers change underneath, and a reader cannot tell a statement this build would make again from one it would not. Comparing two methods that disagree is #184; declaring one superseded is #185, and it belongs in the Declaration.

A Corroboration names its method as a plain word, and that is deliberate. A corroboration is an arithmetic a receiver repeats for themselves, and the figure beside it is the check; if the arithmetic changes, the figure means something else and the method takes a new name, not a later version.
_Avoid_: Algorithm, strategy, technique, model

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

**Citation**:
The Link of kind `judicial.cites`. It says a court opinion cited a Provision: the subject is the opinion, the object is the Structural path the citation resolves to, and the text the rule matched travels with it as Evidence. It is always machine suggested — a pattern matched some words and no person has looked at it.

It is the statement this project exists to make and nobody publishes. CourtListener finds a U.S. Code citation with eyecite, fails to resolve it, and throws it away; what survives is display markup with no identifier. So the extractor is ours (#52).

The object stops at the section even where the opinion named a subsection, because a citation cannot be trusted at that depth — the published example `981(a)(l)(C)` has a lower-case L where the provision has a paragraph (1) — so the subsection is recorded as written rather than resolved.

Where the Dataset holds the citing opinion, the subject names the node, so a reader can follow the link to the words that made the citation. Where it does not, the subject says plainly that the citing document is outside the file and cannot be checked against it.
_Avoid_: Reference, cite, mention

**Redesignation**:
The Link of kind `legislature.redesignated_as`. It says a Provision was renumbered: the subject is the provision as it was, the object is the provision as it became, and each end names the change — a Work, a Structural path, and the two dates — so the edge says *when* the renumbering happened. A path is reused, so an edge with no dates would claim a renumbering held for all time.

A Bill states it in words: "redesignating paragraph (3) as paragraph (2)". Nothing in the US Code records it, which is why a Diff that pairs by position alone reads a renumbering as a rewrite of whichever provision now holds the number, plus the disappearance of the one that moved.

A Redesignation is read from a Bill by either of two readers, and both hand their reading to one resolver that makes the paths. A path a reader proposes must be there: the old one in the earlier Expression, the new one in the later. The words at those two ends are compared, and the measurement travels with the link as Corroboration — which is the stronger half of the check, because a Bill that shifts a whole run of provisions by one letter leaves every path on both sides in place and only the words say which reading is right.

A redesignation a Bill states and no reader can resolve to two paths is an Unplaced statement. It is **recorded**, never dropped. The tool's silence must not read as the corpus's silence.

A Diff reports a Redesignation's effect as **moved**, and that is the one place the word is ours to use: nothing relocated, the number changed. The publisher has no `move` action to confuse it with — its schema defines twelve amending actions and that is not one of them (`.out-of-scope/amending-action-move.md`). A provision that really is relocated is a different thing, and no reader reads one yet.
_Avoid_: Rename, alias, transfer

**Provision history**:
The Redesignations one Provision ran through, walked out of the links, oldest first. A projection: nothing stores it, so a Bill added later adds an edge rather than rewriting an identity, and nothing that already points somewhere breaks (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`). An empty history is an answer — the provision has always been where it is — and not a failure.
_Avoid_: Chain, lineage, ancestry

**Unplaced statement**:
Something a source states that a reader read and could not turn into a statement about the law. It carries the words, the reason, the reader that failed, and the path in the source document where the words sit, so a reviewer can open them.

It is not an Exclusion. An Exclusion says a Dataset does not hold some material, and here the material is held: the text is in hand, the Amendment is in hand, and what is missing is a Link we could not make. Recording one as an Exclusion would answer "out of scope" for a provision the Dataset holds. It is not a Gap either, because nothing was declared and then missed.

A reason is part of the statement, as it is for an Exclusion: a hole with no reason cannot be told apart from an oversight.

**It is a derivation, not a stored row.** The words and the path are already in the Dataset, because the Dataset holds a Bill as a Document, and the reason is reproduced by resolving the same statements against the same windows. A stored row would go stale: the same bill leaves 31 statements unplaced against title 26 alone and 17 against the whole Code, so a row written when the bill was loaded becomes false as soon as a release point is added. What makes a statement **placed** is a Link — so an unplaced statement is a statement with no Link, and both halves of the fact are read from the Dataset alone (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`, `docs/adr/0010-two-readers-one-resolver-a-model-never-writes-a-path.md`).
_Avoid_: Error, failure, skip, warning

**Window**:
Two neighbouring Expressions of one Work: the law as it read before, and as it read after. It is what a statement about change is checked against, and every step that reads change takes one — `score-amendments`, `match-amendments` and `redesignations` all name it with the same argument.

A window is made by loading a release point and is resolved by a separate step. Loading a Bill records nothing, because at that moment nobody knows which window matters and often the window is not held yet (#181).
_Avoid_: Version pair, range, period

**Unresolved window**:
A Bill and a Window where the bill states Redesignations, this build can place them in that window, and the Dataset holds no Link. It is work nobody has run, and it is named as the command that runs it.

It is not an Unplaced statement, and the difference is what makes the report worth reading. An unplaced statement is finished work with a reason: a reader read the words and no reader could turn them into two paths. An unresolved window is a step that has not run, and running it makes links. A report that could not tell the two apart would name the whole corpus and be ignored (#183).

It is a derivation, and nothing stores it. A Redesignation link carries the Work, both dates and the bill, so "has this bill been resolved against this window" is answered by the links the Dataset holds. A stored list of outstanding steps would go stale as soon as a link arrived by another route (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).

**It is not a Method run, and the two answer different questions.** An unresolved window is derived, per bill, from the links the Dataset holds. A method run is recorded, per window, and says which reasoning was applied there. Neither replaces the other: a method run does not say which bills were covered, and an unresolved window does not say which version of the reasoning ran.
_Avoid_: Todo, backlog, pending, dirty

**Contradiction**:
More than one Link about one subject. It is **computed** and never stored: the Links coexist, none is stamped, and none is rewritten (#179 decision 12). Two shapes, and they are kept apart because they are different facts.

A **duplication** is the same subject, the same object, and Links in more than one Window. One Method, run over two windows, placed one move twice. It says nothing is wrong with either link on its own; it says the Dataset holds one fact twice.

A **disagreement** is the same subject and a different object. The two Links cannot both be true.

Which Link to keep is a separate question, and it is open (#172). So nothing orders a contradiction by Corroboration: the figure is evidence for a reviewer, and on a measured corpus the false link scored higher in five pairs out of 64, worst case 0.22 against 0.71. A report that put the highest figure first would present the wrong Link first.

A count of contradictions is a **floor** and not a total, while two statements asserting one move still collapse into one stored Link (#220).
_Avoid_: Conflict, clash, error, duplicate

**Method run**:
A record that one Method, at one version, ran over one Window. It is how a Dataset says what has been done to it, so that a missing step no longer reads as a complete file (#182, #179 decision 11).

It records the **method**, not the command. "`redesignations` has run here" stays true for ever while the thing it means changes underneath. "This reasoning was applied to this window" is a fact an agent can act on.

It is stored, and it passes the test that decides what may be. "Method M at version V ran over window W" happened, and it stays true however much the Dataset grows. An Unplaced statement has the other shape — its count falls as titles are added — and is derived for that reason (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).

It carries no clock reading. The same method, at the same version, over the same window is one record, which is what keeps a rebuild idempotent. It sits in the Dataset's metadata, beside the Declaration, so both stored forms carry it without a table of its own.
_Avoid_: Log, history, audit trail, run id

**Extension**:
A named set of facts that only some datasets carry, such as the legislature facts (Bill, Sponsor, Roll call) or the judicial facts (case name, opinion type). The core data model carries no extension concept, and no document class either. A Link's kind is an open namespaced string, a Document node's type is an open namespaced string, and in both cases the facts only one namespace understands sit in a payload the core stores and never reads (Kind payload, Class payload).

**Two halves of this were not true until recently.** The core's identity, dates and provenance were always class-neutral; its hierarchy and text were one publisher's XML schema, and every node had to declare itself a US Code document or a bill. A court opinion could not be stored without that false claim. `docs/adr/0006-a-document-node-is-class-neutral.md` closed it, and the US Code is now one document class among others rather than the shape of the core (#129).

The storage traits hold the same line. The core `Storage` trait requires documents, links and evidence, and no extension, so a backend that will only ever hold court opinions implements it without writing a word about bills. Code that needs legislature facts asks for them — `S: Storage + LegislatureReader` — and a dataset arriving from another party is asked at run time, through `Storage::legislature`, which answers `None` for "not a concept here" rather than an empty list (#127).
_Avoid_: Plugin, module, add-on
