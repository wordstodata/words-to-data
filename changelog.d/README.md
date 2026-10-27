# Unreleased changes

Each PR that changes what a user or an agent sees adds one file here:
`changelog.d/<issue>-<short-topic>.md`, for example
`changelog.d/452-block-space-lost.md`.

- The file holds the entry as it will show under the release in
  `CHANGELOG.md`. Its first line is the `### ` heading, with the issue link:
  `### A short title ([#452](https://github.com/wordstodata/words-to-data/issues/452))`.
- Do not edit `CHANGELOG.md` in a PR. Two PRs that both add an entry at the
  top of one file conflict, and the merge queue removes the second one. Two
  PRs that each add their own file do not conflict.
- When a PR raises the version of a method, its entry says so:
  ``Method version: `address, window and quoted words@9` (was 8).`` The PR
  changes only the number in `Method::new(…)`. It does not add a history line
  to the doc comment of the method: `CHANGELOG.md` holds the history.
- To change an entry before the release, edit its file.

## The release

The release PR runs:

```
scripts/changelog-release <version>
```

The script moves all the files here into a new `## <version>` section of
`CHANGELOG.md`, below `## Unreleased`, in the order of their issue numbers,
and deletes them. Then write the summary and the `### Upgrade from …` section
by hand at the top of the new section, as `## 0.6.2` shows.
