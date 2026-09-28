# Working a dataset: answering, resolving, reviewing

**Playbook version 1.** When you record a link or a verdict by following this
playbook, name it as the method `resolve-residue@1`. Raise the version when this
document changes what an agent concludes, not when it only changes its wording.

This document is for any agent — any model, any harness — that is given a
`words_to_data` dataset and the `words_to_data` command. It tells you how to do
three jobs with the command alone:

1. **Answer a question** from the dataset.
2. **Resolve the residue**: the amendments the batch could not link.
3. **Review links**: confirm or refute what a machine suggested.

## Rules that apply to every job

- **Use the command, not the file.** Do not open the SQLite file with another
  tool, and do not read the source code to find an answer. The commands are the
  contract; if a question needs something no command gives, that is a finding —
  write it down in your report.
- **A wrong link is worse than no link.** A missing link stays on a work list
  where someone will find it. A wrong link looks finished. When the evidence is
  not clear, record nothing and say why in your report.
- **Every claim needs evidence you have seen.** Quote the words at both ends of
  a change, or the words of the amendment, from command output. Do not rely on
  what you remember about a law.
- **Say what came from outside the dataset.** If you use general knowledge (for
  example, which party a member belongs to), label it as such.
- **Bound your output.** Listing commands stop at 20 rows by default and say how
  many they did not show. Use `--path`, `--work`, `--at` and `--bill` to narrow;
  use `--json` when you need every row.
- **Never write to a dataset you were not told you may change.** `settle` with
  `--verdict`, and `link-amendment`, write. Everything else only reads.

## The vocabulary you need

- A **work** is one document at every date the dataset holds, such as
  `uscode/title_26`. An **expression** is one work at one date, written
  `uscode/title_26@2025-07-30`.
- A **path** locates a provision: `uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174A/subsection_d/paragraph_3`.
- A **window** is two adjacent dates of one work. A change happens in a window.
- A **public law** is stored as a document too, for example
  `publiclawdocument_119-21@2025-07-04`; its date is its enactment date. A bill's
  id, such as `119-hr-1`, names the same law.
- An **amendment** is one instruction of a law ("Section 174 is amended by
  striking … and inserting …"). Each has a long hexadecimal id. Readers print
  its first twelve characters; `link-amendment` needs the **full** id, which
  `residue --json` and `show-bill --json` print.
- A **link** is a stored statement. `legislature.amended_by` says "this change to
  the Code was made by this amendment". Each link has a short id that `settle`
  accepts. A link's **source** says who made it: `rule:evidence_matching` (the
  batch), `agent:<name>`, `human:<name>`.
- A **review** is its own record, never an edit. The newest review of a link wins.

## Orient first

```
words_to_data info <dataset>                     # what it holds, links by kind, review state, methods run
words_to_data expressions <dataset>              # every work and date
words_to_data bills <dataset>                    # the bills, and how many amendments each states
words_to_data validate <dataset>                 # faults, and steps nobody has run
words_to_data residue <dataset> --bill <bill>    # the amendments with no link, and why
```

## Job 1 — answer a question

The usual shape is: find the provisions, find what changed, find who changed it,
read the words.

1. **Find candidate provisions.** `search` looks for words in the text.
   ```
   words_to_data search <dataset> "software development" --work uscode/title_26 --at 2025-07-30
   ```
   Search the date **after** the change you care about, then the date **before**,
   and compare: a provision present after and absent before was added.
2. **See what changed.** `diff` lists the paths that changed between two dates of
   one work; `path` shows one provision and the words that changed on it.
   ```
   words_to_data diff <dataset> --from uscode/title_26@2025-07-18 --to uscode/title_26@2025-07-30 --path <path>
   words_to_data path <dataset> <path> --from uscode/title_26@2025-07-18 --to uscode/title_26@2025-07-30
   ```
   For a provision that is **new** in the window, `path` prints the words added.
3. **Find which amendment made the change.**
   ```
   words_to_data annotations <dataset> --bill <bill> --path <path>
   ```
   Each row names the amendment and the ids of its links.
4. **Read how the link was made before you rely on it.**
   ```
   words_to_data settle <dataset> --link <id> --explain
   ```
   This prints the amendment's words, the words at both ends of the change, the
   OLRC's classification of the section, and **how the link was made** — the
   address and where it came from, the window, and how the change was chosen.
   A link chosen "by quoted words" under an address holding one change is strong.
   A link chosen "by elimination", or with an address "from the OLRC
   classification table", deserves a closer look.
5. **Read the words themselves.** `search` with `--path` and `--snippet 0` prints
   whole fields:
   ```
   words_to_data search <dataset> "" --path <path> --at 2025-07-30 --snippet 0
   ```
   An empty query matches every field under the path.
6. **Votes.** `votes <dataset> <bill>` gives the House roll calls on a bill,
   counted by party. No command names an individual member's vote yet; if the
   question needs one, say so, and label any party-level inference as such.
7. **If no link names a change you found**, the batch may have missed it. Look
   at `residue` for the bill: the amendment may be listed there with its
   candidate changes. That is Job 2. **An amendment can also be linked in part**
   — one of its changes linked, another not — and then `residue` does not list
   it. `coverage` lists every change in a window that no link claims:
   ```
   words_to_data coverage <dataset> --from <work@date> --to <work@date> --list
   ```
8. **Effective dates and other words the Code does not carry** — "the amendments
   made by this section shall apply to…", transition rules, directions to an
   agency — are in the public law itself, which the dataset stores as its own
   work. Search it by work:
   ```
   words_to_data search <dataset> "shall apply to" --work publiclawdocument_119-21 --all
   ```
   Its paths name the law's own sections, for example
   `publiclawdocument_119-21/title_VII/subtitle_A/chapter_1/section_70302/subsection_e`;
   `--path publiclawdocument_119-21/title_VIII` narrows to one title of the law.

   **Not every section has an effective-date clause.** When there is none, the
   dates are usually inside the inserted text itself ("beginning on July 1,
   2026") — quote those. That an amendment with no clause takes effect on
   enactment is a general rule of law, not something the dataset states: label
   it as outside knowledge. Keep two things apart in your answer: **the Code
   shows it** (the text is printed at a date) and **it is in force** (the date
   inside or around the text). Beware the law's marginal notes: the stored text
   runs them into the words, so a hit such as "(F) Effective date.beginning on…"
   holds a margin label, not a clause.
   An amendment's section of the law is the `law_section` field of its
   `residue --json` row; `show-bill` does not print it.
9. **Related changes.** A new section is usually referred to elsewhere by the
   same law. After finding it, search the later date for "section N" to find
   its companions (a deduction's entry in section 63, its reporting rules, its
   math-error rule).

### Traps the playtests found

- **A later window can show changes that never happened.** Renumbering links
  are recorded over every window, and `diff` and `path` echo them, so a window
  after the law's changes may report a paragraph "moved" or "added" when the
  text at both dates is identical. `settle --explain` may then say the address
  "changed again". Before you report a change in a window, compare the whole
  text at both dates (`search "" --path <path> --at <date> --snippet 0 --json`
  for each date).
- **`diff` can miss the new children of a provision it reports as changed,**
  and `path` on that provision shows only its own fields. The whole-text
  comparison above is the final check.
- **`bills` "WITH CHANGES" and `show-bill`'s `change_count` read 0** even when
  hundreds of links exist. They count a stored field the current method does
  not write. Count links with `annotations` instead.
- **`info` "Roll calls"** counts every bill's roll calls, not one bill's.
- **A `residue` "quiet" reason names a whole section** ("nothing under
  /us/usc/t20/s1092 changed") when the check covered only the part the
  amendment addresses. Another part of that section may well have changed.
- **A link chosen `inside_placed_provision` can point at a sibling amendment's
  new paragraph.** When a link's changed words are not the amendment's own
  words, suspect this, and review it (Job 3).

## Job 2 — resolve the residue

`residue --bill <bill>` lists every amendment of a public law that no link
names. Each row has a **category** and, for work, the **stage** that stopped it:

| category | meaning | what to do |
| --- | --- | --- |
| `work` | the batch could not decide | resolve it (below) |
| `not_held` | it changes something the dataset does not hold (a table of sections, a note) | nothing — already explained |
| `quiet` | nothing under its address changed after enactment | nothing — not work |
| `reviewed: no link` | someone already concluded there is no link | nothing, unless you disagree |

For each `work` row, read the row in full first — `residue --bill <bill> --json`
gives its address (if any), window, candidate changes, its `law_section`, and
the OLRC rows for that section of the law.

**Then, before any stage recipe, read the effective date.** Many residue items
are not failures at all: the change takes effect after the newest date the
dataset holds. Find the effective-date words in the law's own text — usually the
last subsection of the same section of the law:

```
words_to_data search <dataset> "shall apply" --work publiclawdocument_119-21 --all --json
```

and keep the hits whose path holds that section (`…/section_70514/…`). If the
change takes effect after the newest release point, record **no link,
`not_yet_in_corpus`**, and quote the words. In the first playtest, 8 of the 22
items that were not linked were this.

### Stage: address

The batch could not tell which provision of the Code the amendment acts on.

- **"section N is a section of another law"** — the amendment names a section of
  an Act that the Code does not carry as its own section. Look at the OLRC rows
  in the residue row: if one classifies this section of the law to a Code
  section, and its description is not a note (`nt …`) or a heading (`prec`),
  that section is your candidate. If every row is a note, the change is in a note
  the dataset does not hold: record **no link, `not_held`**.
- **"no level above the clause says what it amends"** or **"no section named"** —
  the section is stated in an enclosing clause. Read the amendment's text in
  `show-bill <dataset> <bill>`, and the clauses near its section of the law
  (the residue row gives that as `law_section`, for example `70431(c)(1)`).
  Amendments sharing a law section are usually siblings.
- **A whole section repealed** ("Section X of Public Law Y (NN U.S.C. Z) is
  repealed") — check with `diff --path` that the Code section is removed in the
  window, and link the removed path.
- **A repeal that revives earlier text** ("… is repealed, and any provision of
  law amended … by that section is restored or revived as if that section had
  not been enacted") — the change is the old words coming back. Link only a
  change whose after-text you can show is the restored wording (compare the
  words before and after, and look for a sibling amendment that quotes them).
  If you cannot divide the revived changes between two such repeals, leave the
  items alone: the dataset does not hold the repealed law's own text.
- **"nothing says which title holds section N"** — find the title from the law's
  own words (an `(… U.S.C. …)` citation nearby, or a marginal note such as
  `26 USC 224`), or from an OLRC row.

When you have a candidate section, find the change: `diff` over the window with
`--path` on that section, then `path` on each changed provision. Link only a
change whose words match what the amendment says.

### Stage: window

The address resolved, but the Code holds no such provision in any window after
enactment. Usually the address was misread. Check the amendment's words against
the Code with `search` before anything else. If the provision genuinely takes
effect after the newest date the dataset holds, record **no link,
`not_yet_in_corpus`**, and quote the effective-date words.

### Stage: resolve

The address resolved and changes exist under it, but the batch could not choose.

- **"the words it quotes show in none of the changes"** — most often the Code
  **translates** what the bill quotes. A bill citing an Act writes "section
  3(u)(4)"; the Code prints "section 2012(u)(4) of this title". "This Act"
  becomes "this chapter". Search the candidate changes for the translated form.
  Also check whether a later amendment of the same law edited inside the words
  this one inserted.
- **"another amendment's words fit … as well as its own"** — first run
  `annotations --path` on each candidate change. Often the candidate has no
  link at all, or its words match only this amendment, and there is no real
  tie. When it is real, two amendments quote the same words (often "and" or "or"). Read both amendments and each
  candidate change; decide by what else each amendment says (its "in paragraph
  (2)" scope, the other words it inserts). If you cannot tell, record nothing.
- **"each change under its address is another amendment's"** — look at the
  links that took those changes (`annotations --path`); one of them may be wrong.
  That is a review (Job 3).

### Things that look wrong but are the data

- **Two provisions at one path.** When a law adds two paragraphs with the same
  number (the Code sometimes prints two "(4)"s), both sit at one path. A link
  to that path cannot say which one it means: say which in your reason.
- **`path` on a parent does not show its children's field changes.** Use
  `diff --path <parent>` to list the changed children, then `path --exact` on
  each.
- Re-run `residue` at the end. It is how you find the items you skipped.

### Recording a resolution

A link — only for paths that changed in the window; the command refuses others:

```
words_to_data link-amendment <dataset> --bill <bill> --amendment <id> \
  --from <work@date> --to <work@date> --path <path> [--path <path> …] \
  --source agent:<your-name> --method resolve-residue@1 \
  --reason "<what the amendment says, and the words you saw change>"
```

A conclusion that there is no link:

```
words_to_data link-amendment <dataset> --bill <bill> --amendment <id> \
  --no-link <not_held|not_yet_in_corpus|no_change|other> \
  --source agent:<your-name> --method resolve-residue@1 \
  --reason "<why, with the evidence>"
```

Write the reason for a reader who will check you. Quote the amendment and the
changed words. Your record is `MachineSuggested`, like every machine's, and a
person or another agent may refute it.

## Job 3 — review links

A link is never edited. You confirm it, refute it, or dispute it, and each is a
record of its own:

```
words_to_data settle <dataset> --link <id> --verdict <confirmed|refuted|disputed> \
  --reviewer agent:<your-name> --reason "<the evidence>"
```

To **correct** a wrong link, refute it and record the right one with
`link-amendment`.

- **A link that is right but incomplete** — the amendment also changed a path
  the link does not name — is **confirmed**, and you add the missing path with
  `link-amendment`.
- **A change with two causes** (two laws, or two amendments, each visibly
  responsible for part of the new words) is right for each cause whose words
  you can see in it.
- **If you refute an amendment's only link and find no right one**, the
  amendment leaves every work list — `residue` treats any link, even a refuted
  one, as linked. Until the tool changes, list that amendment under "open" in
  your report, with what you searched for, so a person can pick it up.

**What to review first.** `annotations <dataset> --bill <bill> --json` gives each
link a `links` entry with how it was made:

- `recorded.chosen` — `quoted_words`, `inside_placed_provision`, `renumbering` or
  `elimination`
- `recorded.address_source` — `markup` or `olrc`
- `recorded.changes_under_address` — how many changes the address held
- `causes` — how many amendments are linked to the same change

Start every review by comparing the amendment's quoted words with the words
that changed. A link whose changed words have nothing to do with the
amendment — a renumbered footnote marker, a punctuation change made by a
sibling — is the error this playbook most wants caught.

Review in this order: links chosen by **elimination**; links whose address came
from the **OLRC** table; links whose change has **more than one cause**; links
chosen by quoted words under an address holding **many** changes. Then take a
small sample of the strongest kind — quoted words, one change — so the report
can say how often the strong links are right rather than assume it.

A link you review is right when the amendment's words and the words that
changed agree: the struck text was there before and is gone after, the inserted
text is there after, and the provision is the one the amendment names.

## Report

End every run with a short report:

- what you were asked, and your answer with its evidence
- each record you wrote (link, no-link, review), with its id
- each item you left alone, and why
- anything the commands could not tell you — these are the tool's gaps
