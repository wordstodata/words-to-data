### `residue --partial` lists an amendment that is linked in part ([#271](https://github.com/wordstodata/words-to-data/issues/271))

- **A new option, `residue --partial`, and a new category, `linked_in_part`.**
  With the option, `residue` also lists an amendment that a standing link
  names when a change under its address, in the window the matcher gives it,
  is claimed by no standing `amended_by` link. The row's `changes` are those
  unclaimed changes only. Before, only `coverage` showed such a change, and
  `coverage` does not name an amendment.
- **Without `--partial`, the output does not change**, as text and as
  `--json`. The default list is the work the playbook gives an agent, so these
  rows are not in it.
- The row says that each change is under the address and unclaimed. It does not
  say that the change is the amendment's: an agent reads the words and decides.
- A change whose link to the same amendment was refuted is not listed again.
- With `--partial`, the text output counts the rows as "linked in part" and
  lists them after the work. The first line still counts the amendments that
  no standing link names.
- On a copy of `eval.sqlite`, `--partial` adds 151 rows (489 unclaimed changes,
  272 different ones). The other counts do not change: 508 amendments that no
  standing link names, 231 of them work.
- **The stored format does not change** (`SCHEMA_VERSION` 10), and no method
  version changes: the batch makes the same links. The playbook stays at
  `resolve-residue@1`: Job 1 step 7 tells the agent about `--partial`.
- **Rust library:** `residue::unlinked_amendments` has a third argument,
  `residue::Rows` (`Unlinked` gives the list as before, `AlsoLinkedInPart`
  adds the new rows), and `residue::Category` has a new variant,
  `LinkedInPart`. In `evidence_matching::Linked`, the field
  `changes_under_address` (a count) is now `under_address`, the list of those
  changes.
