### `path` and `settle --explain` say when a citation names an edition the dataset does not hold ([#414](https://github.com/wordstodata/words-to-data/issues/414))

*Boechler, P. C. v. Commissioner* (2022) cites "26 U. S. C. §6404(g)(1) (1994
ed., Supp. II)". The rule the court applied is now § 6404(h)(1). In the
printings held, § 6404(g)(1) is "Suspension", a different rule. `cases-citing`
said so (#449, #470). `path` and `settle --explain` showed the edition only
inside the quoted words, and gave no warning.

Now `path` prints a line under each citation link that names an edition, and
`settle --explain` prints the same line before the evidence:

```
the opinion cites the Code of 1994, written "(1994 ed., Supp. II)" after the citation. This dataset holds no printing of title 26 from 1994, so that law is out of scope. A section number can be used again, so the provision now at that number can be a different provision from the one the court read.
```

When the dataset holds a printing from the year of the edition, the line names
those printings and gives no warning. The three commands use one wording.

`path --json` gives `editions` on each citation entry that names one:
`[{ "written": "1994 ed., Supp. II", "year": "1994", "printings": [] }]`. An
entry that names no edition has no `editions` field, so it is as before. The
output of `cases-citing` does not change.
