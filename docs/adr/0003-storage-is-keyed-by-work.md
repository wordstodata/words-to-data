# Storage is keyed by work, not by release date

Status: accepted

A dataset used to be a list of versions keyed by date, each holding one whole document tree. That is a global snapshot, and it encodes an assumption: every document in the dataset is republished on the same day. The US Code does work that way — every title comes out together on one release point — so the shape fit the only corpus we had.

It fits nothing else. Ten court opinions are ten documents with ten publication dates and nothing shared. A state code amends title by title. A regulation is republished on its own schedule. None of these has a date that describes the dataset rather than one document in it, so none could enter a dataset at all.

An expression is therefore `(work, date)` with its own tree. A **work** is the document as a concept, with no date; an **expression** is that work as it read on one date. Nothing groups expressions by date above that, so two documents published on unrelated days sit side by side without either pretending to share a cycle. A work with a single expression is the ordinary case, not a degenerate one — for a court opinion it is the only case there will ever be.

## Consequences

The release point stops being a stored thing and becomes what it always was: how the source publishes. A US Code release point of fifty-seven titles arrives as one merged tree and is stored as fifty-seven expressions, one per title.

Three questions become answerable that were not. "What documents does this dataset hold" is `works()`, and it no longer requires reading the text of everything held. "When was this document published" is `expressions(&work)`, and the answer belongs to that document rather than to the dataset. "What came before this" is the previous expression *of the same work* — under the old shape that question walked into a different document and answered confidently.

Two become properly refusable. A diff needs two expressions of one work; across two works it would compare unrelated documents and report the whole of each as changed, so it is an error rather than an answer. And an annotation is keyed by the pair of expressions it sits between, so a dataset holding two works can tell apart annotations that happen to share a date pair.

The cost is a schema break, on both the SQLite and the JSON form. Datasets are rebuilt rather than migrated, so the break has to be loud: both formats carry a schema version and refuse a file that does not match (`docs/adr/0002-links-live-in-the-core.md` names the failure this avoids — an empty answer that actually means "wrong schema"). The JSON form had no such guard before this change and now does.

## Considered and rejected

**Keep the global version list, and derive works from it.** This is what the previous slice did, and it is why this one exists. Works were discovered by asking which global snapshots happened to contain a path, so a document belonging to no snapshot could not be described. The view was honest about the data underneath; the data was the problem.

**Key on date, and let a version hold many documents.** A smaller change: keep `versions`, add a document column inside. But then the date still owns the row, and a document with no shared date needs an invented one. An invented date is a false statement about when the law read that way, and it is exactly the sort of confident wrong answer the rest of this model is built to avoid.

**Give a work a stable identity now, rather than a structural path.** `WorkId` holds a structural path today, which `docs/adr/0001-structural-paths-locate-not-identify.md` says is a locator and not an identity. That is a real debt, and it is deliberately not paid here: minting provision identity is its own piece of work, and doing it inside a storage break would make both harder to review. The field is private so the change costs no call site.
