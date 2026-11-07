### A Statutes at Large cite of a Revenue Code names the edition of the Code ([#482](https://github.com/wordstodata/words-to-data/issues/482))

*Commissioner v. Glenshaw Glass Co.* (1955) writes "53 Stat. 9, 53 Stat. 574,
26 U. S. C. § 22 (a)". Volume 53 of the Statutes at Large holds the Internal
Revenue Code of 1939, and § 22(a) of that Code is the gross-income rule. The
citation reader did not read the Stat. cite, so link f8070c43d47e named the
current § 22(a), the credit for the elderly, with no edition.

Now a Statutes at Large cite of a Revenue Code right before a title 26
citation names an edition, the same as "(1976 ed.)" after a citation (#449):

- `53 Stat.` (the Internal Revenue Code of 1939): year 1939.
- `68A Stat.` (the Internal Revenue Code of 1954): year 1954.

Other volumes, and a citation of another title, do not change. An edition in
brackets after the citation comes first when a citation has both.

- `usc::Edition` has a new field `before`, true for a Stat. cite. Its
  `written` is the Stat. cite as the court wrote it: `53 Stat. 574`. In the
  payload of a link, `edition.before` is present only when it is true, so the
  payload of a link with an edition in brackets does not change.
- The evidence of the link gives the cite after the words read:
  `26 U. S. C. § 22 (a) (after 53 Stat. 574)`.
- `cases-citing` says where the court wrote the edition:

```
the opinion cites the Code of 1939, written "53 Stat. 574" before the citation. This dataset holds no printing of title 26 from 1939, so that law is out of scope.
no window is given: the opinion cites the Code of 1939, written "53 Stat. 574" before the citation, and this dataset holds no printing of title 26 from that year. A section number can be used again, so the provision now at that number can be a different provision from the one the court read.
```

`CitedEdition` has the same new field `before`, so each edition in
`cases-citing --json` has a new key `before`. No schema change. To get the
edition on the links of a dataset, run `add-opinions` again.

Method version: `reporters-db laws.json U.S.C. patterns@8` (was 7).
