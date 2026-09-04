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
A statement that connects one provision to something else. It carries a subject, a namespaced kind, an object, and its provenance. Every reader can read a link, even a reader that does not know the kind.
_Avoid_: Relation, edge, reference

**Change annotation**:
The link of kind `legislature.amended_by`. It connects one change in a diff to the amendment that caused it.
_Avoid_: Match, mapping, label

**Extension**:
A named set of facts that only some datasets carry, such as the legislature facts (bill, sponsor, vote) or the judicial facts (court, opinion type). The core carries no extension concept.
_Avoid_: Plugin, module, add-on

**Provenance**:
The record of where one statement came from: its source, the method that produced it, its evidence, and its verification state.
_Avoid_: Lineage, history, audit

**Provision**:
A unit of law that stays the same thing across versions, even when its text changes or it moves to a new address. Its identity does not depend on its location.
_Avoid_: Section, node, element

**Structural path**:
The address of an element, derived from the hierarchy that the parser found. It does not depend on the source document to supply an identifier. It locates a provision at one point in time. It does not identify one.
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
The statement of what a Dataset covers. A Dataset declares the scope it intends, and reports the scope it holds. A reader uses the scope to answer "out of scope" instead of "not found".
_Avoid_: Coverage, extent, contents

**Verification state**:
The trust level of one statement in a Dataset: `Asserted` by a source, `MachineSuggested`, `HumanConfirmed`, or `Disputed`. Every statement that a machine made carries this state and its evidence.
_Avoid_: Confidence, score, accuracy
