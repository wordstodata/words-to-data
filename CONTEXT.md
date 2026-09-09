# Words To Data

The language of this project. `words_to_data` turns legal documents into data structures that a person or an agent can compare, link, and send to another party.

## Language

**Legal research**:
The task of finding the authority that controls a legal question. This is the goal of the project.
_Avoid_: Search, lookup

**Legislative change tracking**:
The task of finding what changed between two versions of a law, and which amendment caused each change. This is the capability that exists today, and the first step toward legal research.
_Avoid_: Diffing, legal diff, change detection

**Work**:
A legal document or provision as a concept, with no date attached. Title 26 section 174 is one work, from 1954 until today.
_Avoid_: Document, section

**Expression**:
One work as it read on one date. A version of the law, not a version of a file.
_Avoid_: Version snapshot, revision, edition

**Diff**:
The set of differences between two expressions of one work, in the shape of the document hierarchy.
_Avoid_: Delta, comparison, change set

**Amendment**:
An instruction in a bill that tells a reader how to change existing law.
_Avoid_: Edit, modification, revision

**Link**:
A statement that connects one provision to something else. It carries a subject, a namespaced kind, an object, and its provenance. Every reader can read a link, even a reader that does not know the kind. Its identity comes from its subject, its kind, and its object, so restating the same fact updates one link rather than making a second one.
_Avoid_: Relation, edge, reference

**Kind payload**:
The facts about a link that only the extension defining its kind understands, such as the amending action behind a change annotation. The core stores it, hands it back unchanged, and never reads it. Nothing a reader needs in order to report a link may live here.
_Avoid_: Extra, metadata, blob

**Change annotation**:
The link of kind `legislature.amended_by`. It connects one change in a diff to the amendment that caused it.
_Avoid_: Match, mapping, label

**Extension**:
A named set of facts that only some datasets carry, such as the legislature facts (bill, sponsor, vote) or the judicial facts (court, opinion type). The core carries no extension concept.
_Avoid_: Plugin, module, add-on

**Provenance**:
The record of where one statement came from: its source, the method that produced it, when it was made, its evidence, and its verification state.
_Avoid_: Lineage, history, audit

**Evidence**:
What a statement was based on: the maker's reasoning in their own words, and the verbatim reply that produced it. A machine's claim is only checkable if the party receiving the Dataset can see what the machine actually said, so evidence is what stops a verification state being a label with nothing behind it.
_Avoid_: Proof, justification, backing

**Model reply**:
The text a model returned, kept exactly as it arrived. One reply usually makes several statements, so it is stored once and referred to, never copied onto each. It is never deleted, even when the statement it supported has been superseded: an unreferenced reply is a record that something was said. A reply that no parser could read makes no statement, so the Dataset does not carry it at all.
_Avoid_: Response, output, completion

**Provision**:
A unit of law that stays the same thing across versions, even when its text changes or it moves to a new address. Its identity does not depend on its location.
_Avoid_: Section, node, element

**Structural path**:
The address of an element, derived from the hierarchy that the parser found. It does not depend on the source document to supply an identifier. It locates a provision at one point in time. It does not identify one. Two provisions can share one path: the law sometimes numbers two provisions alike, and the document records both. A path and a position together locate one provision within one expression. They still do not identify it.
_Avoid_: Path, breadcrumb

**USLM ID**:
The official identifier of an element, as published in the source document. Many legal documents supply none.
_Avoid_: ID, identifier, reference

## The dataset

**Dataset**:
A collection of legal documents and the links between them. It has no fixed size: one title, or all of federal and state law.
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

**Verification state**:
The trust level of one statement in a Dataset: `Asserted` by a source, `MachineSuggested`, `HumanConfirmed`, `Disputed`, or `Refuted`. Every statement that a machine made carries this state and its evidence. `Disputed` means someone objects and it is unsettled; `Refuted` means it was checked and found wrong, which is settled.
_Avoid_: Confidence, score, accuracy
