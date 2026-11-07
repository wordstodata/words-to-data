### `residue` always gives the count of the outdated links ([#447](https://github.com/wordstodata/words-to-data/issues/447))

`residue` printed the section "Links the current method no longer makes" only
when a link was outdated, and only at the end. In `--json`, the `outdated` key
came after all the rows. For 119-hr-1 the JSON is approximately 261,000
characters, so a reader that cuts the output could not tell "no outdated link"
from "the output was cut".

Now:

- The text output always ends with the section. When no link is outdated, it
  prints `Links the current method no longer makes (0)`.
- The summary at the top has a new line with the same count:
  `0 link(s) the current method no longer makes: review work, listed at the end`.
- `--json` has a new key `outdated_count`. `outdated_count` and `outdated` now
  come before `rows`. The keys and the rows do not change otherwise.
- `residue --help` names the section and says that the output always gives its
  count.
- `docs/agents/working-a-dataset.md` says how to read the count when the output
  is cut.
