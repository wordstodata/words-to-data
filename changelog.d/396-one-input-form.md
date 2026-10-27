### One input has one name and one form in every command ([#396](https://github.com/wordstodata/words-to-data/issues/396))

- **Every command that takes a bill reads the public law it became.**
  `residue --bill`, `votes`, `amendment-addresses --bill`, `annotations --bill`,
  `section-agreement --bill`, `redesignation-report --bill`, `redesignations
  --bill` and `link-amendment --bill` now take `Pub. L. 119-21`, `PL 119-21` and
  `119-21`, as `show-bill` does since [#297](https://github.com/wordstodata/words-to-data/issues/297). Before, they refused the law, and the
  refusal did not name `119-hr-1`. A law that the dataset holds no bill for is
  refused in the same words in each command, and the refusal now names each
  held public law and its bill: `Pub. L. 119-21 (119-hr-1)`.
- **`redesignation-report` and `redesignations` take `--bill`**, as every other
  command does. `--bill-id` is the old name. It stays as a hidden alias for one
  release.
- **`coverage` takes `--all` and `--limit`**, as the other list commands do.
  Human output now stops at 20 paths (before: 10). `--json` gives every path,
  unless `--limit` is given. `--list` is the old name of `--all`. It stays as a
  hidden alias for one release.
- **`cases-citing --cites` reads a citation with no section marker.**
  `26 USC 174`, `26 U.S.C. 174` and `26 U.S.C. § 174` give the same answer.
  Before, the two forms without `§` were refused. Bill text writes
  `26 USC 6213`. The reader of court text does not change: there a number with
  no marker can be a page or a year, so it is still not read.
- **Rust library:** one new function, `citation::usc::find_typed`. Nothing is
  removed or changed. No stored data and no method version changes.
