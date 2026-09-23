# Links live in the core, with namespaced kinds

Status: accepted. Implemented, with one term below that describes intent rather than code.

## A note on the word for a node (#147)

The variant this document and `docs/adr/0004` call `Target::Provision` is now **`Target::Node`**. `CONTEXT.md` defines a Provision as a unit of law that stays the same thing across versions, and a court opinion is neither, so every stored opinion link said in the core's own vocabulary that an opinion is a provision. The shape was right — the variant carries a path, which is what is needed — and the stored word was false, and a target is serialized into every W2D file, so the falsehood travelled.

This is a **rename inside the closed set**, not a widening of it. The set below is "ours, so it widens when the core needs to say something new"; nothing new is said here and no variant is added, so this ADR needs this note rather than a new decision. Read `Target::Provision` anywhere below as `Target::Node`. The rename changes `subject_json` and `object_json` on every stored link in both forms, so it rode the schema break of #182 rather than taking one of its own.

A second same-shaped variant for a document was considered and rejected: two variants of one shape invite the wrong one being picked.

## A note on "a provision identity"

The shape section says the object of a link may be "a provision identity". There is no provision identity in the code. `Target::Node` carries a **path**, which `docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md` states plainly — "a provision holds a path" — and which `docs/adr/0001-structural-paths-locate-not-identify.md` now flags as unbuilt. Read "provision identity" in this document as the thing a link is meant to point at; read ADR 0004 for what it points at today. #93 closes the difference.

Everything else in this ADR is built: one `Link` type in the core with a namespaced kind, `legislature.amended_by` as the first kind, the open kind string, and a `Declaration` that lists the namespaces a reader should expect (`Scope::declares_namespace`).

The core data model is document-class-neutral: identity, hierarchy, text, dates, and provenance. Bills, sponsors, members, and votes move to a legislature extension, and courts and opinion types form a judicial extension. A reader might expect `ChangeAnnotation`, which connects a change to the amendment that caused it, to move into the legislature extension with everything else about amendments. It does not.

> **Two of those five were not neutral when this was written.** Identity, dates and provenance were. Hierarchy and text were a `USLMElement`, named after one publisher's XML schema, and every node in the tree had to declare a `DocumentType` of US Code or bill. `docs/adr/0006-a-document-node-is-class-neutral.md` closed that, on this ADR's own rule: the node's type became an open namespaced string, and the facts only one class understands moved into a payload the core never reads. The sentence above is now true of the code (#129).

Instead the core holds one `Link` type: subject, namespaced kind, object, provenance. `ChangeAnnotation` becomes the kind `legislature.amended_by`. A citation becomes `judicial.cites`. A third party's enrichment becomes `westlaw.headnote`. The kind is a namespaced string, not an enum we control.

## Why

The links are the product. A reader that does not know an extension must still see that a link exists, read its subject and object, report its verification state, and preserve it when it writes the file again. If each extension owned its own link tables, a reader that does not know Westlaw would see nothing at all, and could not even report that something was there. Silent loss is the one failure a portable format cannot have.

Keeping the kind open, rather than an enum, is what lets another party add a link type without our permission. That is the condition for the format becoming a shared language instead of our library.

## The shape of a link

**A link points at a closed set of things.** The object is a provision identity (**not built — a path today, see the note above and #93**), an expression, a document, a change (a provision as it read across two dates), or an external reference (a URI with display text). An extension type, such as an amendment, is reached as an external reference into that extension's namespace. The set is closed but it is ours, so it widens when the core needs to say something new: the change target was added because a link whose subject is a bare provision cannot say *when* the provision was amended (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`). This keeps one property that matters: a reader who does not know the legislature extension can still report "this change was caused by something, and here is its name", and a reader can still tell a reference inside the file from a reference to a web page, because only the first can be checked.

**Links are directed and stored once.** The reverse reading, "what did this amendment change", is a query. Two records for one fact can disagree, and after an edit one of them will.

**A namespace is a free string.** We publish a convention and register the kinds we define. The scope declares which namespaces a dataset uses, so a reader knows what it is about to meet. A namespace that names a third party, such as `westlaw.`, should be used by that party, or with a source note in the provenance. A central registry would rebuild the gatekeeper this decision removes.

## Consequences

We cannot validate the meaning of a kind we do not own. We validate the shape of a link, and the namespace convention, and nothing more.

Every link carries provenance, so a machine-made link is never presented as a fact from a source. This is the same rule that governs the rest of the data model.
