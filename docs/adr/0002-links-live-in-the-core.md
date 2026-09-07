# Links live in the core, with namespaced kinds

Status: accepted

The core data model is document-class-neutral: identity, hierarchy, text, dates, and provenance. Bills, sponsors, members, and votes move to a legislature extension, and courts and opinion types will form a judicial extension. A reader might expect `ChangeAnnotation`, which connects a change to the amendment that caused it, to move into the legislature extension with everything else about amendments. It does not.

Instead the core holds one `Link` type: subject, namespaced kind, object, provenance. `ChangeAnnotation` becomes the kind `legislature.amended_by`. A citation becomes `judicial.cites`. A third party's enrichment becomes `westlaw.headnote`. The kind is a namespaced string, not an enum we control.

## Why

The links are the product. A reader that does not know an extension must still see that a link exists, read its subject and object, report its verification state, and preserve it when it writes the file again. If each extension owned its own link tables, a reader that does not know Westlaw would see nothing at all, and could not even report that something was there. Silent loss is the one failure a portable format cannot have.

Keeping the kind open, rather than an enum, is what lets another party add a link type without our permission. That is the condition for the format becoming a shared language instead of our library.

## The shape of a link

**A link points at a closed set of things.** The object is a provision identity, an expression, a document, a change (a provision as it read across two dates), or an external reference (a URI with display text). An extension type, such as an amendment, is reached as an external reference into that extension's namespace. The set is closed but it is ours, so it widens when the core needs to say something new: the change target was added because a link whose subject is a bare provision cannot say *when* the provision was amended (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`). This keeps one property that matters: a reader who does not know the legislature extension can still report "this change was caused by something, and here is its name", and a reader can still tell a reference inside the file from a reference to a web page, because only the first can be checked.

**Links are directed and stored once.** The reverse reading, "what did this amendment change", is a query. Two records for one fact can disagree, and after an edit one of them will.

**A namespace is a free string.** We publish a convention and register the kinds we define. The scope declares which namespaces a dataset uses, so a reader knows what it is about to meet. A namespace that names a third party, such as `westlaw.`, should be used by that party, or with a source note in the provenance. A central registry would rebuild the gatekeeper this decision removes.

## Consequences

We cannot validate the meaning of a kind we do not own. We validate the shape of a link, and the namespace convention, and nothing more.

Every link carries provenance, so a machine-made link is never presented as a fact from a source. This is the same rule that governs the rest of the data model.
