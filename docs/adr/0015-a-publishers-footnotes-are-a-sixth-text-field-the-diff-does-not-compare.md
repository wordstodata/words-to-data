# A publisher's footnotes are a sixth text field, and the diff does not compare it

Status: accepted. It adds a sixth text field to the five that `docs/adr/0006-a-document-node-is-class-neutral.md` names; the other rules of 0006 stand.

The OLRC prints editorial footnotes in the Code: "So in original. The comma probably should not appear.", "See References in Text note below.". USLM puts the mark and the footnote text inside the text elements of the provision. The parser read every text below an element, so the footnotes were in the words of the law. The OLRC numbers its footnotes again in each printing, so `diff` reported a footnote that only got a new number as a change to the law, and `coverage` put it in the work queue (#416). In title 26 and title 42, four windows held 51 such changes.

A footnote is not the law, and it is useful: it tells a reader that the words are "so in original", or where to find a reference. A reader must still find it with `search`.

## The decision

**A node has a sixth core text field, `footnotes`.** It holds the text of each footnote that the publisher printed in the words of the node, in the order of the marks, one footnote on each line, without the publisher's number. The marks and the footnote text go out of the five fields of words.

- **Search reads it.** A hit names the field `footnotes`, so a reader knows that the words are the publisher's and not the law's.
- **The diff does not compare it, and `coverage` does not count it.** A changed footnote is not a change to the law.
- **The readers of the law's words do not read it**: the evidence matcher, the address reader and the renumbering reader.

The field is core, not a class fact, because search reads it. ADR 0006 says that nothing a reader needs to report a node can live in a payload that the core does not read. The field is not one class's word either: a court and an agency print footnotes too.

A second mark for a footnote has no text beside it, because the publisher prints the text at the first mark. The field of that node holds the text from the first mark, so each provision that the footnote is about says so.

## Consequences

`SCHEMA_VERSION` goes to 11. The search index `element_index` gets a column `footnotes`, and the compact form gets the field. A dataset at 10 is refused by name. Datasets are built again, never migrated.

The place of a mark in the words is not kept. A reader that must know which comma "The comma probably should not appear" is about reads the publisher's text.

## Considered and rejected

**Take out the marks, and keep the footnote text in the words.** No schema change, but the publisher's editorial text stays in the law's words, and a footnote that the OLRC adds or removes is still a change.

**A footnote node under the provision, `…/footnote_<n>`.** No path moves and search finds the footnote. But the OLRC adds and removes footnotes in each window, so the diff reports added and removed nodes in place of changed words, and the work queue does not get smaller (title 26, 2025-07-18 to 2025-07-30: 1,052 to 1,050).

**A footnote node that `diff` and `coverage` do not report.** Then the core must know that one node type annotates the law, and the core does not own the vocabulary of node types (ADR 0006).

**The footnote in the USLM payload only.** Search does not read a payload, so a search for "So in original" finds nothing, and what a node says must not live only in its payload.
