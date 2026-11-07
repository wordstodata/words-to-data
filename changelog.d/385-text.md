### A new command, `text`, prints a Code provision as the law reads ([#385](https://github.com/wordstodata/words-to-data/issues/385))

`words_to_data text <dataset> <path> --at <date>` prints a Code provision and
its children at one date, in document order, indented by level. Each node
prints its number as the publisher prints it ("(a)", "§ 6041.") and its heading
on one line, then its chapeau, content, footnotes (marked `[footnote]`),
children, proviso and continuation. Before, the only way to read the words was
`search "" --path <path> --snippet 0 --all`, which printed one hit for each
field, with no numbers, and put the continuation of a subsection before its
paragraphs.

- The answer stops after 400 lines, and says "… N more node(s) not shown; give
  --depth or a deeper path". `--limit N` changes the limit. `--depth N` prints
  only N levels below the path, and the last level prints its number and
  heading only, so `--depth 1` on a chapter lists its sections.
- `--json` gives `{ expression, path, at, nodes_total, truncated, node }`, with
  `nodes_total` and `truncated` before the tree. `node` is a tree of `{ path,
  num, heading, chapeau, content, proviso, continuation, footnotes, children }`.
  An empty field is left out. The JSON holds the same nodes as the text, cut at
  the same limit.
- Code paths only. A court or a bill path is refused, with exit 1. A path that
  no expression holds at the date gets the notice `search` gives for that path
  and date, with exit 1.
- The library gives the same reader as `uslm::text::provision_text`.
- `docs/agents/working-a-dataset.md` now tells an agent to read a provision with
  `text`. sleuth-pi must copy the playbook again.

The command only reads. `SCHEMA_VERSION` and the method versions do not change,
and no dataset needs a rebuild.
