# A document node is class-neutral, and its type is an open string

Status: accepted

`docs/adr/0002-links-live-in-the-core.md` claimed the core data model was document-class-neutral in "identity, hierarchy, text, dates, and provenance". Three of those five were. Hierarchy and text were not: the tree an expression carried was a `USLMElement`, named after one publisher's XML schema, and every node in it had to declare a `DocumentType` from a closed set of two — US Code or bill — and an `ElementType` from twenty-two USLM legislative words.

A court opinion has no honest place in that. Measured against the real cached record for *Obergefell v. Hodges*, eleven fields the source supplies had nowhere to live, and three fields the model demanded had no answer: `number_value`, `element_type`, `document_type`. Storing one meant asserting it was a US Code document. That is the failure class these ADRs exist to prevent — not an error, but a confident wrong answer in a file sent to another party, which nothing downstream can detect.

So the node becomes class-neutral. This is not "add support for court opinions": it demotes USLM from *the* document model to *a* document class, and the US Code becomes one tenant of a tree it used to be the shape of. This ADR records the parts that are expensive to change once datasets carry them.

## The core keeps what any reader needs to report a node, and nothing else

A `DocumentNode` carries a structural path, a type, a date, the five text fields, its children, and where its text came from. That is what it takes to locate a provision, search it, diff it, quote it, and say how far it can be trusted — for a statute, a regulation or an opinion alike.

Everything else moves out. An element's number, the publisher's rendering of that number, its USLM identifier, its XML uuid, its source credits and its document type are facts a USLM reader understands and nobody else does.

## Facts that only one class understands live in a payload the core never reads

The node therefore carries a namespaced payload, on exactly the contract `docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md` already gave a link's kind payload: the core stores it, hands it back unchanged, and never interprets it. `uscode` carries `UslmFacts`; `judicial` carries `OpinionFacts`. A `westlaw` payload would be carried without our writing a line.

The same two rules keep it from becoming a dumping ground. The core never reads it. And **nothing needed to report a node may live there**: a reader that cannot open the payload must still be able to say where the node is, what kind of thing it is, what it says, and how its text was obtained.

The payload is held as JSON **text**, not as a parsed value. Both reasons follow from "hands it back unchanged": text comes back byte for byte, while a value tree reorders an object's keys on the way through, and a corpus of a million and a half nodes cannot afford a map per node. The core is equally blind either way.

## `extracted_by_ocr` is provenance, not a judicial fact

It looks like one. A CourtListener opinion carries it, no other document class has it, and it would sit quietly in the `judicial` payload beside the case name.

It goes in `Provenance` instead, as a `method` — `ocr` or `text_layer`. It does not say anything about a court; it says how the text was obtained. A reader deciding whether to rely on a passage must not have to open an extension payload to learn that a machine read it off a scan, and #53 requires mixed provenance precisely so that the verification state differs between records. This is the same exception a timestamp already is: every statement has a when, and every text has a how.

**A node carries provenance only where it differs from its neighbours'.** Every node of a US Code release point came from the same publisher by the same method, so recording it on each would state one fact 57,391 times over per title. A USLM node therefore carries none, exactly as it carried none before. An opinion is one node, and it is the case that needs it.

## The node's type is a namespaced open string, not an enum

This is the decision inside the decision. `element_type` could not simply move into the payload, because the core reads it: the diff refuses to pair two nodes of different types, and a reader reports what a node is.

So it becomes a `NodeType`, a namespaced open string, exactly as a `LinkKind` already is: `uscode.section`, `public_law.section`, `judicial.opinion`. The core reads the string and owns none of the vocabulary, which is the property ADR 0002 exists to protect — a third party adds a document class without our permission, and a type this build has never seen is stored and read back unchanged rather than dropped.

**The namespace replaced `DocumentType` rather than joining it.** Adding a `CourtOpinion` variant would have repeated the fault one size larger. Making the document type a second open string would have put a class declaration in two places, which can disagree; after an edit one of them will. The namespace *is* the class declaration. What remained of `DocumentType` — which USC type, which bill — is a USLM fact and went into the `uscode` payload with the rest.

A public law is `public_law.*`, not `uscode.*`, although both come out of the same USLM parser with the same words below the root. A public law is not part of the US Code, and nothing stored may say that it is.

**The local half is the publisher's element name, and it is also the path segment.** `uscode.section` and `section_174` take their word from one list, `ElementType::local_name`, so a path and a type can never disagree about what an element is called. That list is spelled out rather than derived from Rust's `Debug`, because it is published: it is in every path and every node type in every dataset, and renaming a Rust variant must not rename a provision. The cost is two warts that will now live forever — `uscode.uscodedocument` and `public_law.publiclawdocument` — and one list is worth more than two pretty names.

**`ElementType` stays a closed enum**, inside the USLM module. It is not a stored vocabulary: it is the list of XML tag names this parser knows, and a tag it does not know is dropped rather than stored. What is stored is the open string it maps to.

## Consequences

`SCHEMA_VERSION` goes to 9. Every field of every node changes shape, in both on-disk forms, so a dataset at 8 is refused by name rather than half-read. Datasets are rebuilt, never migrated.

`Expression.element` becomes `Expression.root`. It is the root of a tree of nodes, and "element" is USLM's word for a node, which `CONTEXT.md` already told readers to avoid for a provision.

`DocumentReader::find_element` and `has_element` become `find_nodes` and `has_node`. The SQLite search index keeps its `element_type` column under the name `node_type`, holding the string the producer wrote.

The corpus is measurably unchanged. A dataset built from the two committed release points holds the same node counts, the same paths — checked as a digest of every path in document order, taken on the build before this change — and the same diff. The cost is 3.5% on the compact file and 5.2% on the SQLite file for title 26, and about a fifth more parse time.

**A class's reader can read its own payload; the core still cannot.** `SectionPaths` indexes US Code sections by USLM identifier, and it now reads that identifier out of the `uscode` payload. That is allowed, and it is the line ADR 0004 draws: US Code citation code is not the core. What would not be allowed is the diff, the search index or the path generator doing it.

## Considered and rejected

**Add a `CourtOpinion` variant to `DocumentType`, and opinion words to `ElementType`.** Cheapest, and it repeats the fault at one greater size: the next class needs our permission, and the enum grows a wing per publisher.

**Make the whole node a trait, so a backend stores whatever implements it.** Most flexible, and worst for a portable file format, which must carry a concrete shape. A W2D file is the product.

**Keep `uslm_id` and `number_value` as optional core fields.** They would be `None` and `""` for an opinion, which is not a lie. But a core field named after one publisher's schema tells every future reader that the core knows about that publisher, and the next class would add its own two beside them.

**Put the node type in the payload too, and pair the diff on the path alone.** Then two different things at one path across two dates would be diffed as one provision changing. The type is the only thing that says they are the same kind of thing.

**Record USLM provenance on every node.** Honest and uniform, and it costs a heap allocation and a copy of one sentence per node across a million and a half nodes to say what one line in the dataset's metadata says once.
