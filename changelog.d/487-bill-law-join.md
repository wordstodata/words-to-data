### A bill joins to its law also when the law states no amendment ([#487](https://github.com/wordstodata/words-to-data/issues/487))

`bills` and `show-bill` gave "not enacted / not held" and `"public_law": null`
for `119-hr-2215`, but the dataset held its law, Pub. L. 119-25. The join from
a bill to its law read only the amendments that the law states, and
Pub. L. 119-25 states no amendment.

- The parser now records on the root node of a public law the bill that the
  law enacted, as the publisher names it in the marginal note of the long
  title: `[H.R. 2215]`. The new field is `UslmFacts::enacted_bill`
  (`/us/bill/119/hr/2215`). `bill_parser::enacted_bill` gives it as a dataset
  bill id: `119-hr-2215`.
- `bill_document` and `bill_public_laws` join a bill to the law that names it,
  and else by the amendments, as before. So `bills`, `show-bill` and
  `show-bill <law>` find the law of a bill that states no amendment.
- The join does not read `document_type.bill_id`.
- **A dataset built before this change needs a rebuild** to get the new join.
  Its laws hold no note of their bill, so it gives the same answers as before:
  a bill that states amendments keeps its law, and a bill that states none
  stays "not enacted / not held". `add-bills` does not load a bill again that
  the dataset holds. The SQLite schema and `SCHEMA_VERSION` do not change.
