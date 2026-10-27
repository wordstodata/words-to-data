### A citation that names an edition of the Code says so ([#449](https://github.com/wordstodata/words-to-data/issues/449))

The *Windsor* dissent of Justice Scalia cites "8 U.S.C. § 1105a(a) (1976
ed.)", the old rule on judicial review of a deportation order. The number was
used again: in the printings held, § 1105a(a) is "Employment authorization for
battered spouses". The link b546b4d422d9 went to the current § 1105a(a), and
`cases-citing` called the provision "unchanged".

Now the citation reader reads the edition in the brackets after a citation:
`(1976 ed.)`, `(1994 ed., Supp. II)`, or the year alone, `(1988)`. The link
does not change (ADR 0014, decision 7: an older citation is resolved against
the earliest printing held and says so). Each place of the link records the
edition (`edition`, with `written` and `year`), and the evidence ends with it.
`cases-citing` says which edition the opinion cites and whether the dataset
holds a printing from its year. When it holds none, `cases-citing` gives no
window, because the provision now at that number can be a different provision.
`add-opinions` prints how many citations name an edition.

#### Measured on the opinions of `evals/opinions.txt`

Added to two copies of one fresh build of the Code alone (the five release
points of `scripts/build-eval`, offline), by the build before this change and
by the build after it:

- **`judicial.cites`** stays at 441 links and 488 places. No id, subject or
  object changes.
- 8 citations name an edition. 2 of them make no link before or after: *Boeing*
  cites §§ 921, 923 and § 926(a)(2) (1988 ed.), which the title does not hold.
- The other 6 are 6 links, and each now records its edition. Each was checked
  against the printing held:
  - b546b4d422d9, *Windsor*, § 1105a(a) (1976 ed.): the number was used again.
  - ca4da73d0e0e, *Boechler*, § 6404(g)(1) (1994 ed., Supp. II): the number
    was used again. The Court cites the Tax Court's jurisdiction to review a
    denial of interest abatement, which is now (h). Now (g) is "Suspension of interest and certain
    penalties where Secretary fails to contact taxpayer". Before, `cases-citing`
    gave 4 windows "unchanged" for it.
  - 06f7127fb946, *Windsor*, § 1186a(b)(1) (2006 ed. and Supp. V); 47f06288c582,
    § 162 (1988); df752f930dfe and 8e619d761691, *United Stationers*, § 41
    (1986) and § 41(d)(1) (1986): the same provision as now. The link is
    right, and `cases-citing` gives no window for it, because the dataset holds
    no printing of that edition.
- The 303 links of the U.S.C. patterns change their method version. Of them,
  only the 6 above change their payload and evidence. The 138 links read as
  bare references do not change.

#### Breaking changes for agents

- Method version: `reporters-db laws.json U.S.C. patterns@7` (was 6). A dataset
  built with 0.6.2 keeps the old links until you run `add-opinions` again. The
  ids do not change.
- `cases-citing --json`: each place has a new field `editions`, and `since` has
  a new field `edition_not_held`. When `edition_not_held` is set, `windows` is
  empty.
