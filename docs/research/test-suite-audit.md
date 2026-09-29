# Test suite audit: where it can shrink and speed up (#278)

> *This was written by AI on 2026-09-28, at the maintainer's request. It is a
> report. It deletes and changes no test. The maintainer decides what to cut.*

The question from #278: **where can the test suite become smaller and faster
without the loss of a behavior that it guards?**

## The answer in short

1. **Most of the time is spent in unoptimized code, not in too many tests.**
   The same suite, compiled at `opt-level = 1`, ran in 217 s. In the debug
   profile that CI uses, it ran in 928–931 s. That is 77% less time, and no
   test is removed. This is the largest single saving.
2. **The next saving is setup that each test does again.** Many files parse
   title 26 (55 MB) or title 7 (28.5 MB) in each test. One file,
   `path_redesignation_tests.rs`, already parses once per binary and clones
   the tree. With that pattern, a test that took 8–9 s takes about 0.5 s. If
   13 more files use the same pattern, the single-thread time falls by about
   730 s of 2,773 s.
3. **Redundant tests are real but they are few and cheap.** Mutation proves
   that 11 tests add nothing that another test does not catch. Three more
   tests guard nothing. Together they cost about 60 s of single-thread time.
4. **Consolidation cuts code, not time.** About 80 narrow test functions can
   become about 20 table-driven tests. The same cases run.
5. **The TDD rule needs a companion rule.** The rule adds one test for each
   cycle and never removes one. Section 6 gives the proposed text.

Estimated total, if the maintainer accepts every proven recommendation:

| | Now | After | Change |
|---|---:|---:|---:|
| `#[test]`/`#[rstest]` functions in `tests/` | 665 | about 575 | −14% |
| Debug, single thread, sum of all binaries | 2,773 s | about 1,900 s | −30% |
| Debug, parallel, sum of all binaries | 930 s | about 700 s | −25% (estimate) |
| The same, at `opt-level = 1` | 930 s | about 160 s | −83% (estimate; 217 s measured before the setup changes) |

These savings do not add in a straight line. Section 7 lists each
recommendation with its own estimate and its confidence.

---

## How I measured

- **Machine.** Linux, 48 cores, rustc 1.97.1 (stable). Load average was 2.4
  to 4.8 from other work during the runs.
- **Corpus.** I extracted `tests/test_data/test_files.tar.xz` first. Every
  test passed in every run below (exit 0). Two tests are `#[ignore]`.
- **Build.** I built once with `cargo test --no-run` (exit 0), so compile time
  is not in the timings. **The profile is `test`**, which inherits `dev`
  (unoptimized + debuginfo). CI uses the same profile.
- **Binary wall time, parallel.** For each file in `tests/`:
  `cargo test -q --test <name>`, timed with `date`, with cargo's own exit code
  recorded. I did this twice (run 1 and run 2). Script: `hacks/time_bins.sh`.
- **Per-test time, single thread.** Each test binary, run directly:
  `RUSTC_BOOTSTRAP=1 <binary> -Z unstable-options --report-time --test-threads=1`.
  `--report-time` is libtest's own per-test timer. It is unstable, so
  `RUSTC_BOOTSTRAP=1` lets the stable harness accept it. Nothing is rebuilt.
  With one thread, the first test that touches a `OnceLock` fixture pays for
  the fixture, and the tests after it do not. Script: `hacks/report_time.sh`.
- **Optimized builds.** `cargo test --release` (exit 0, clean build 55 s), and
  `CARGO_TARGET_DIR=hacks/target-o1 CARGO_PROFILE_DEV_OPT_LEVEL=1 cargo test`
  (exit 0, clean build 41 s). A clean debug build took 24 s.
- **CI.** The per-binary times from the CI logs of run 36466656826 (the push
  of `886d71f` to `main`), read from libtest's `finished in` line.

`hacks/` is ignored by git. The logs and scripts stay there and are not in
this PR.

---

## 1. Where the time goes

### Per binary

All times are in seconds. "1 thread" is the sum of `--report-time` for the
binary. "Parallel" is the range of run 1 and run 2. CI is one run on a GitHub
`ubuntu-latest` runner.

| Binary | Tests | Debug, 1 thread | Debug, parallel (2 runs) | CI (shard) | `opt-level = 1` | release |
|---|---:|---:|---:|---:|---:|---:|
| `residue_tests` | 16 | 564 | 142–144 | 443 (two) | 39.2 | 35.9 |
| `redesignation_tests` | 27 | 336 | 54–56 | 263 (one) | 9.1 | 7.7 |
| `review_tests` | 16 | 225 | 28 | 158 (rest) | 6.4 | 5.7 |
| `contradiction_tests` | 12 | 162 | 78–80 | 130 (rest) | 13.5 | 12.1 |
| `amendment_link_door_tests` | 15 | 159 | 42–43 | 129 (rest) | 7.0 | 6.2 |
| `renumbering_window_tests` | 11 | 114 | 19 | 83 (rest) | 3.3 | 3.0 |
| `cli_add_bills_tests` | 4 | 95 | 35–43 | 68 (rest) | 17.8 | 16.7 |
| `uncovered_amendment_tests` | 7 | 87 | 17 | 63 (rest) | 3.1 | 2.7 |
| `unplaced_statement_tests` | 9 | 86 | 56–59 | 103 (rest) | 7.4 | 6.1 |
| `duplicate_path_tests` | 13 | 85 | 11–12 | 62 (one) | 2.3 | 2.3 |
| `sqlite_tests` | 6 | 60 | 14 | 30 (four) | 3.1 | 2.9 |
| `link_decision_tests` | 11 | 59 | 39–40 | 71 (rest) | 6.2 | 5.4 |
| `evidence_resolve_tests` | 12 | 58 | 28–29 | 67 (rest) | 4.3 | 3.6 |
| `dataset_tests` | 13 | 54 | 8 | 27 (four) | 1.4 | 1.3 |
| `diff_tests` | 7 | 49 | 13 | 39 (three) | 2.1 | 1.8 |
| `evidence_matching_tests` | 11 | 44 | 37–39 | 51 (three) | 12.9 | 12.1 |
| `cli_olrc_tests` | 2 | 43 | 32–44 | 22 (rest) | 21.4 | 18.5 |
| `path_redesignation_tests` | 11 | 40 | 29–32 | 45 (three) | 10.2 | 10.1 |
| `website_example_tests` | 8 | 39 | 11 | 28 (one) | 1.7 | 1.5 |
| `unresolved_redesignation_tests` | 4 | 35 | 16 | 31 (rest) | 2.4 | 2.1 |
| `usc_citation_link_tests` | 10 | 32 | 10 | 17 (four) | 1.5 | 1.2 |
| `parser_tests` | 15 | 32 | 6–7 | 23 (rest) | 0.9 | 0.8 |
| `olrc_reader_tests` | 4 | 31 | 24–26 | 39 (rest) | 3.3 | 2.9 |
| `one_end_words_tests` | 6 | 26 | 14–16 | 29 (rest) | 2.2 | 1.9 |
| `no_regression_tests` | 23 | 25 | 10–11 | 14 (four) | 1.6 | 1.4 |
| **All 79 binaries** | 644 | **2,773** | **928–931** | **2,264** | **217** | **196** |

The other 54 binaries take 24 s or less each in one thread. The doc tests
take 8.7–8.9 s and the library unit tests 2.6 s.

Three facts come from this table:

- **The ten slowest binaries use 69% of the single-thread time** (1,914 s of
  2,773 s).
- **Debug code is 4–5 times slower than optimized code here.** The work is
  XML parsing, tree diffing and the evidence matcher, which are all
  CPU-bound. `opt-level = 1` gets 90% of the release speed-up, and its clean
  build took 41 s against 24 s for debug.
- **CI is CPU-bound.** Its sum (2,264 s) is close to the local single-thread
  sum, not to the local parallel sum. The `rest` shard ran 1,114 s of tests in
  a 1,320 s job, and it is the critical path of every PR. Shard `two`
  (`residue_tests` alone) ran 443 s.

### The slowest single tests, and what each one pays for

"Setup" means that the time goes to building a fixture. "Work" means that the
test itself runs much code, for example a command that runs the matcher.
Times are debug, one thread.

| Test | Time | Setup or work | What it pays for |
|---|---:|---|---|
| `residue_tests::should_leave_the_list_when_a_newer_review_confirms_the_refuted_link` | 108 s | both | It runs first, so it builds the `unlinked()` and `linked()` fixtures (the bill, titles 7 and 26 at three release points, the renumberings, the OLRC links, then `link-by-evidence`): about 60 s. Then it runs `residue` three times, at about 16 s each. |
| `residue_tests::*` (the other 15) | 16–66 s | work | Each `residue` run calls `match_by_evidence` over titles 7 and 26 again (`src/legislature/residue/mod.rs:157`). A test with one run takes 16.3–16.7 s. The binary runs `residue` about 29 times. |
| `unplaced_statement_tests::should_show_a_row_for_every_statement_when_it_reports_the_corpus` | 55 s | setup | Parses seven titles (5, 7, 10, 15, 20, 26, 42) at two release points. |
| `contradiction_tests::should_filter_rather_than_refuse…` and `…should_find_no_duplication_when_the_step…` | 54 s each | setup | Each is the first user of one of two fixtures. Both fixtures parse five titles at three release points (15 files). The two fixtures parse the same 15 files. |
| `redesignation_tests::should_count_statements_links_and_unplaced…` and `…should_resolve_most_of_the_corpus…` | 49–50 s each | setup | Both parse the same seven titles at two release points, one time each. |
| `amendment_link_door_tests::should_keep_both_records…` | 40 s | both | The shared fixture (the bill and title 26 at three release points) plus four runs of `link-amendment` or `settle`. Each run loads the saved JSON. |
| `link_decision_tests::should_carry_how_each_link_was_made…` | 35 s | setup | Builds the fixture and runs `link-by-evidence` once for the binary. |
| `cli_add_bills_tests::*` | 14–32 s each | work | Each test runs `build-dataset` over title 7 at three release points, one or two times. No test shares a build. |
| `path_redesignation_tests::should_answer_rather_than_panic_at_every_path…` | 29 s | work | Walks every path that a link names. The parse is shared, so the other ten tests take 0.5–3 s. |
| `review_tests::*` | 9–25 s each | setup | Each test parses title 26 at two release points again (about 8 s), and then runs one or more commands. No fixture is shared. |
| `redesignation_tests::*` (about 15 tests) | 8–10 s each | setup | The same: title 26 at two release points, parsed again in each test. |
| `renumbering_window_tests::*` | 7–17 s each | setup | Title 7 at three release points, parsed again in each test. |
| `utils_tests::test_load_uslm_folder` | 10 s | work | Parses all 57 files of one release point (659 MB). It is the only test of `load_uslm_folder`, and the whole release point is its point. Keep it. |

### The cost of a parse, measured

| Operation (debug, one thread) | Time | Where I read it |
|---|---:|---|
| Parse title 26 (55 MB) | about 4 s | `website_example_parse_usc_document`: 4.0 s |
| Parse title 7 (28.5 MB) | about 2.3 s | each `source_credit_tests` test: 2.3–2.4 s |
| Parse title 26 at two dates, load the bill, record renumberings | about 9 s | `review_tests::should_leave_two_records…`: 9.2 s |
| Clone the two parsed title 26 trees, record renumberings | about 0.5 s | `path_redesignation_tests`, the eight tests after the first: 0.50–0.59 s |
| Load a saved JSON of title 26 at two dates in a command | about 2.5 s | `one_end_words_tests`, each `path` test: 2.4–2.6 s |

`DocumentNode` is `Clone`. `Dataset` is not. So the pattern that works is:
parse each file once per binary in a `OnceLock`, clone the root for each
test, and build the dataset from the clones. `path_redesignation_tests.rs`
lines 38–72 already do this.

---

## 2. Redundancy

Each group names the test to keep and why. The rule is strict: the kept test
must fail for every fault that the dropped test catches. Where I say that, I
proved it by mutation (Appendix A). Where I did not prove it, the group is in
section 2.2 and is not yet a recommendation to cut.

### 2.1 Proved by mutation

| # | Drop | Keep | What each one asserts | Mutation |
|---|---|---|---|---|
| D1 | `date_tests.rs`, all 3 tests | `utils_tests.rs::test_valid_date_parsing`, `test_leap_year_feb_29`, `test_invalid_date_missing_components` | `utils::date_str_to_date` is a re-export of `date::date_str_to_date`. `test_date_str_to_date_valid` is a copy of `test_valid_date_parsing` (year 2025, month 7, day 18). The leap-year test asserts only `is_ok()`; the kept one also asserts 2024, 2, 29. The format test asserts `"2025-07"` is an error; the kept one asserts that and two more inputs. | M1a, M1b, M1c |
| D2 | `olrc_reader_tests::should_show_the_olrc_row_when_path_reads_a_section_the_olrc_classified` | `olrc_reader_tests::should_say_a_row_classifies_a_note_when_its_descriptions_are_all_notes` | Both run `path <dataset> <§ 174A>`. The dropped test asserts `contains("…§70302(a), new")`. The kept test asserts `contains("…§70302(a), new\n")`, which is longer, and also the note row. | M2 |
| D3 | `residue_tests::should_not_call_an_amendment_quiet_when_the_code_held_has_nothing_at_its_address` | `residue_tests::should_report_an_amendment_to_a_note_as_not_held_and_not_as_a_miss` | Same fixture, same amendment `5219c7e9a020`. Dropped: `assert_ne!(category, "quiet")`. Kept: `assert_eq!(category, "not_held")`, and the OLRC row and the reason. The dropped test's own comment says that the case it was written for no longer exists after #259. | M3 |
| D4 | `website_example_tests::website_example_compute_diff` | `diff_tests::test_diff_generation_26` | Both parse title 26 at 07-18 and 07-30, call `TreeDiff::from_nodes`, find § 174(a), and assert the same old and new chapeau text. The kept test also asserts that the chapeau change is the *first* change. Move the comment "If this fails, update index.html" to the kept test. | M4 |
| D5 | `tree_navigation_tests::test_find_paragraph_element` | `edge_case_tests::test_deeply_nested_structure` | Same file (title 9), same path (`…/section_10/subsection_a/paragraph_1`), same `node_type == "uscode.paragraph"`. The kept test also asserts six path segments. | M6 |
| D6 | `dataset_tests::should_compute_diff_between_two_expressions_of_one_work` | `sqlite_tests::should_query_via_trait_interface` | Dropped: `diff.root_path == TITLE_7` on an in-memory dataset. Kept: `check_reader` asserts `diff.root_path == TITLE_7` on the in-memory dataset **and** on SQLite. | M12 |
| D7 | `redesignation_tests::should_resolve_the_paragraph_898_c_renumbered_when_title_26_is_in_hand` | `redesignation_tests::should_report_a_renumbered_paragraph_as_moved_when_the_bill_said_so` | Dropped: `resolve` gives § 898(c)(3) → (2). Kept: feeds the same `resolve` into `TreeDiff::from_nodes_with` and asserts `moved == [(p3, p2)]`, no words changed, nothing added. | M13 |
| D8 | `link_tests::should_carry_the_models_reasoning_as_evidence` | `link_tests::should_mark_unreviewed_model_output_as_machine_suggested` | Dropped: the reasoning is not empty. Kept: the reasoning equals the annotation's reasoning. A mutation that cuts the reasoning to 10 characters passes the dropped test and fails the kept test. | M14a, M14b |
| D9 | `usc_citation_tests::should_find_nothing_when_the_text_has_no_citation` | `usc_citation_report_tests::should_report_nothing_when_the_text_names_no_statute` | Same input string. `usc::find` is `find_with_report(text).0` plus a print to stderr (`src/citation/usc.rs:457–461`). The kept test asserts `found.is_empty()` and `report.is_empty()`. | M15 |
| D10 | `cli_tests::should_list_only_the_named_work_when_expressions_is_given_one` | `cli_tests::should_report_no_second_expression_for_a_work_published_once`, **after one assertion is added** | The dropped test runs `expressions --work` on a dataset that holds one work, so the filter cannot show. A mutation that ignores `--work` passes it. The kept test fails on that mutation. Add `assert_eq!(expressions[0]["work"], UNCHANGED_WORK)` to it first, so that it also catches a filter that picks the wrong work. | M10 |

Time saved, debug, one thread: D2 2.5 s, D3 16 s, D4 10 s, D6 6 s, D7 8 s,
the rest under 0.1 s each. **About 43 s, 12 tests** (D1 is three).

### 2.2 Candidates not yet proved

These look redundant from the code. I did not prove them by mutation, so do
not cut them until a mutation shows that the kept test fails.

| Drop | Keep | Why it looks redundant |
|---|---|---|
| `bill_parser_tests::should_extract_amendments_from_bill` | `…::should_find_amendments_with_multiple_action_types` | `!is_empty()` is implied by `action_types.len() > 1` on some amendment. |
| `bill_parser_tests::should_extract_action_types_from_amendments` | the same | "some amendment has action types" is implied by "some amendment has more than one". |
| `bill_parser_tests::should_parse_bill_id_in_congress_number_format` | `…::should_parse_bill_and_extract_bill_id` | Both check the `bill_id` that the test passes in. |
| `diff_tests::test_diff_generation_across_titles::case_3` (title 26) | `diff_tests::test_diff_generation_26` | Same parse and diff; asserts only `root_path`. Saves about 10 s. |
| `website_example_tests::website_download_links_files_exist` | any test that parses the three files | `Path::exists` on files that other tests parse. |
| `no_regression_tests::should_hold_the_same_node_count…` (5 cases) | `…::should_generate_the_same_paths…` (same 5 files) | The digest is over the path list, so a count change changes the digest. Merge: assert the count first for a readable failure, then the digest. Saves 5 parses (about 8 s). |
| `schema_version_tests::should_refuse_a_dataset_written_by_a_different_schema` (version 1) | `…::should_refuse_a_dataset_written_at_the_previous_schema` (version 9) | Same `found != expected` check. |
| `member_party_tests::should_report_republican_when_kiley_is_asked…` | `…::should_distinguish_an_unresolved_party…` | The JSON test asserts `{"Resolved":"Republican"}` for the same member and date. |
| `inspect_tests::should_find_heading_matches_on_sqlite_backend` | `search_tests::should_return_results_in_the_same_order_on_both_backends…` | Same query on title 9; the kept test asserts SQLite hits equal memory hits. |
| `amendment_id_printing_tests::should_mint_the_same_id_when_one_printing_transliterates_an_em_dash` | `…::should_mint_the_same_amendment_ids_when_two_printings…` | The kept test asserts that every id agrees; the dropped one checks a subset. |
| `text_content_tests::test_all_text_field_accessors` | `…::test_parse_chapeau_field` | Its only assertion is `chapeau.is_some()`; the kept test asserts that and the text. |
| `tree_navigation_tests::test_find_appendix_element` | `tree_navigation::test_find_root_in_real_document`, `edge_case_tests::test_parse_appendix_title` | Root `find` and root node type are checked elsewhere; the rest is an `if let` (see section 3). |
| `duplicate_path_tests::should_say_a_path_exists_when_it_names_more_than_one_provision` | `…::should_say_whether_a_path_exists_on_both_backends` + `…::should_hold_every_provision_sharing_a_path…` | The two kept tests together make every assertion the dropped one makes. |

About 20 tests. I estimate that most of them are real redundancy, but the
confidence is **medium** until each has a mutation.

### 2.3 Overlaps that are not redundant

These look alike but each catches a fault that the other does not. Keep both.

- `evidence_matching_tests::should_leave_changes_as_residue_when_the_quoted_words_cannot_place_them`
  and `residue_tests::should_list_an_amendment_with_its_stage_reason_window_and_changes…`:
  same amendment (`a9fd405d5415`), same matcher. The first is the library
  answer; the second is the only check of the `residue --json` fields
  `address`, `from` and `to`.
- The amendment `d624331f459d` (7 U.S.C. 2015(o)) is asserted in five tests in
  two files. Each asserts a different fact (not f217's, not in a later window,
  unwritten, two links).
- `cli_tests` and `inspect_tests` check the same subtree annotations (8 rows).
  The CLI tests are the only SQLite check of subtree and exact matching.
- `redesignation_tests` `:880` and `:963` build the same corpus but assert
  different counts. Share the build (section 4); keep both tests.

---

## 3. Tests that guard nothing

| Test | Why it guards nothing | Proof | What to do |
|---|---|---|---|
| `tree_navigation_tests::test_find_deeply_nested_structure` | Its whole body is `if let Some(found) = result { … }`, and a comment says that a missing path is "OK". | M9: `find` changed to return `None` always. This test **passed**. Eight other tests in the file failed. | Assert `is_some()`, or delete it. |
| `inspect_tests::should_collect_changed_added_and_removed_paths_matching_the_tree_diff` and `…::should_produce_identical_diff_summary_for_sqlite_backend` | Title 9 has no change between the two release points, so they compare 0 with 0. | M11: `inspect::diff` changed to collect no path. Both tests **passed**. Four other tests failed, in `cli_tests`, `inspect_tests` and `scoped_report_tests`. | Keep them, but move them to a fixture with changes (title 51, which `cli_tests` already builds). Today the path lists of `inspect::diff` have no library-level test. |
| `cli_tests::should_list_only_the_named_work_when_expressions_is_given_one` | One work in the dataset, so the filter cannot show. | M10 (see D10). | Drop after D10. |
| `readme_example_tests::readme_example_dataset_workflow` | No `assert!`. It shows only that the README code runs without an error. | Read: lines 24–90 hold `expect` calls and no assertion. | Merge its one unique step (the compact save) into `readme_example_dataset_workflow_results`. Saves 13.6 s. |
| `evidence_resolve_tests::should_not_give_a_change_to_an_amendment_that_only_renumbers_by_elimination` | Its comment says that before #262 the amendment had no address, so the test "only guards the rule". The run took 0.000 s. | **Not proved.** #262 is merged now. #264 says the test failed without the fix on a branch with #262. | Check by mutation of the elimination rule before any cut. |

**Internal details, not behavior.** These are weak, not empty. Change them when
the file is next edited:

- `review_tests` (the schema test) and `schema_version_tests` assert
  `SCHEMA_VERSION == 10`. That pins a constant, and each schema change edits
  both.
- `dataset_tests`, `readme_example_tests` and `sqlite_tests` read
  `storage().bills` directly. `list_bill_ids` and `get_bill` are the public
  way.
- `storage_traits_tests::should_implement_storage_without_the_legislature_extension`
  asserts `legislature().is_none()` on a test type whose method returns `None`.
- `court_opinion_tests` builds the opinion node by hand (`obergefell()`), so
  four tests assert values that the test set. Production dating is tested by
  `court_opinion_citation_tests::should_date_the_opinion_from_the_cluster…`.

**Code that PR #261 left behind.** #261 removed the model pipeline and kept
some types for stored data.

- `Link::from_annotation` has **no caller in `src/`**. Only tests call it, to
  build fixtures (`link_decision`, `contradiction`, `sqlite`, `inspect`,
  `cli`, `section_agreement`, `link_storage`). `link_tests.rs` (6 tests) and
  `link_storage_tests.rs` lines 37–159 (5 tests) test it. It is public API of
  a published crate, so the tests guard real behavior for a library user. If
  the maintainer removes the function from the API, these 11 tests go with it.
- `Dataset::add_reply` and `update_amendments` have no caller in `src/bin`.
  `reply_evidence_tests` lines 77 and 142 test only the writer. Lines 103,
  176 and 220 test the readers of old datasets, which is real behavior. Keep
  those.
- `uncovered_amendment_tests::should_say_nothing_when_a_run_of_the_removed_model_method_has_covered_the_window`
  is the only test of the compatibility branch `REMOVED_MODEL_METHOD`
  (`src/inspect.rs:1668–1678`). Keep it.

---

## 4. Expensive setup that is shared badly

### 4.1 Inside one binary

The fix is the same everywhere: parse each file one time per binary in a
`OnceLock`, clone the root for each test, and build the `Dataset` from the
clones. Tests that only read can share one `&'static` value. The
per-test estimate uses the measured costs above (parse of a title-26 pair
about 8 s, clone and record about 0.5 s).

| File | Now | Proposal | Saves, 1 thread (estimate) |
|---|---|---|---:|
| `review_tests.rs` | `dataset_with_real_links()` runs 13 times and `dataset_with_amendment_links()` 3 times: about 32 parses of title 26. | Parse the pair once; clone per test. | about 125 s of 225 s |
| `redesignation_tests.rs` | About 36 parses of title 26 and 4 of title 42. `:880` and `:963` each parse the same 14 files. | Parse the title-26 and title-42 pairs once. Build the seven-title sweep once in a `OnceLock` for `:880` and `:963`. | about 150–180 s of 336 s |
| `renumbering_window_tests.rs` | `title_7_and_the_bill()` parses title 7 at three dates in each call: about 36 parses. | Parse once; clone per test. | about 70 s of 114 s |
| `duplicate_path_tests.rs` | About 19 parses of title 26. Two tests only look up bad paths. | Parse once. | about 70 s of 85 s |
| `uncovered_amendment_tests.rs` | 13 parses of title 26 in 6 builds. | Parse once. | about 55 s of 87 s |
| `sqlite_tests.rs`, `dataset_tests.rs` | `make_expression` parses title 7 in each call: 13 calls in each file. | Parse once in a `OnceLock<DocumentNode>`; clone. | about 85 s of 114 s |
| `diff_tests.rs`, `website_example_tests.rs`, `readme_example_tests.rs` | 8 + 8 + 4 parses of title 26. | Parse the pair once per binary. | about 70 s of 112 s |
| `unresolved_redesignation_tests.rs` | `grown_dataset()` parses title 26 twice, three times. | Parse once. | about 20 s of 35 s |
| `contradiction_tests.rs` | `two_window_file()` and `two_window_step_file()` parse the same 15 files. | Parse the 15 files once; record the renumberings two ways. | about 40 s of 162 s |
| `parser_tests.rs`, `source_credit_tests.rs`, `no_regression_tests.rs` | Title 26 four times, title 7 five times, the no-regression files twice. | Parse once per file. | about 30 s |
| `cli_add_bills_tests.rs` | Five `build-dataset` runs over title 7 at three dates. Three tests build the same dataset with no bill. Two build the same dataset with the bill. | Build each variant once, and copy the file for each test that writes. | about 45 s of 95 s |
| `residue_tests.rs` | `residue --json` runs 8 times on the unchanged `linked()` fixture (lines 188, 402, 423, 433, 453, 477, 544, 562), at about 16 s each. | Run it once in a `OnceLock` and let the tests read the rows. This is "slow because the test does a lot", not setup: each run is a full matcher run. | about 110 s of 564 s |

**Sum: about 870–900 s of the 2,773 s single-thread time, with no test
removed.** In the parallel local run the wall time falls less, because a
binary's wall time is bounded by its slowest test. On CI, which is
CPU-bound, the saving follows the single-thread number more closely.

**Confidence: medium.** The cost of each parse is measured. The count of
parses in each file comes from reading the code. One risk: a clone of a
55 MB tree for each test uses memory. `path_redesignation_tests` does this
today with no problem.

### 4.2 Across binaries

The same dataset is built in several binaries. A `OnceLock` cannot cross a
binary.

| Dataset | Built in |
|---|---|
| The bill, title 26 at 07-18 and 07-30, renumberings over the one window | `evidence_resolve`, `contradiction` (one window), `unplaced_statement`, `uncovered_amendment`, `review` |
| Title 26 at two dates and the title-26 links of `evidence_links.json` | `one_end_words` (shared), `review` (built three times) |
| The bill, titles at three release points, renumberings, `link-by-evidence` | `evidence_matching` (title 7), `link_decision` (title 26 + OLRC), `residue` (titles 7 and 26 + OLRC) |
| Seven titles at two release points, the sweep | `redesignation` (twice), `unplaced_statement` (once) |

Two ways to share:

1. **Move the tests into one binary.** For example, one `tests/corpus_sweep.rs`
   for the three sweep tests saves about 100 s. This moves tests; it does
   not remove one. Fewer binaries also means less link time.
2. **A disk cache in `CARGO_TARGET_TMPDIR`.** I do **not** recommend this. A
   cache that an older build wrote can hide a fault in the code that builds
   it, and the risk is larger than the saving.

**Confidence: medium.** The first option saves the build of one fixture for
each binary that moves.

---

## 5. Consolidation

These groups have the same body and differ only in input and expected
output. Each can become one `#[rstest]` with `#[case]` rows, or one table
loop. `#[rstest]` keeps one report line for each case, so a failure still
names its row.

This saves code and makes the next case one line. It saves no run time,
except where noted.

| File | Tests | Becomes |
|---|---|---|
| `utils_tests.rs` (+ `date_tests.rs`) | 6 invalid dates, 5 valid dates, 3 date tests | 2 tables |
| `edge_case_tests.rs` | 4 bad-date parses (`:25`, `:35`, `:45`, `:52`) | 1 |
| `tree_navigation_tests.rs` | 5 `find(..).is_none()`, 6 `find(..)` → node type | 2, with one parse of title 9 |
| `path_tests.rs`, `type_conversion_tests.rs` | 3 + 3 | 2 |
| `amendment_address_tests.rs` | 7 `address_saying(phrase)` → section and steps | 1, with one parse of the bill (saves about 10 s) |
| `section_agreement_tests.rs` | 3 "find row → disagrees" | 1 |
| `appendix_container_tests.rs` | 3 `find(..).is_some()` on title 28a | 1 |
| `cli_tests.rs` | 7 failure cases (args → non-zero exit, stderr text) and 4 span cases | 2 |
| `cli_release_point_tests.rs` | 3 tests with the same `add-release-points` run | 1 (saves 2 runs) |
| `congress_tests.rs`, `compact_head_tests.rs`, `schema_version_tests.rs` | 3 + 3 + 4 | 3 |
| `amendment_link_door_tests.rs` | 6 refusals (args → failure, stderr, no file) | 1 |
| `link_decision_tests.rs` | 6 `settle --explain` lines | 1 |
| `usc_citation_tests.rs`, `usc_citation_report_tests.rs`, `usc_citation_link_tests.rs` | 6 + 4 + 6 single-input cases | 3 |
| `amendment_link_door_tests.rs` `:117`/`:466`, `:168`/`:503`; `link_decision_tests.rs` `:156`/`:220`, `:236`/`:255`; `contradiction_tests.rs` `:548`/`:793` | 5 pairs that run the same command and assert different parts of its output | 5 (saves one command run each) |

**About 100 functions become about 25.** Confidence that coverage stays the
same: high, if each row keeps its own assertions.

---

## 6. The rule itself

### What the rule does now

`CLAUDE.md` says: one RED-GREEN cycle at a time; each cycle writes a test
first; run the full suite after each cycle. It says nothing about when a test
goes.

The PR record shows the cost. Agents said so themselves:

- #242: "**Three of the seven tests passed the moment the counting landed.**
  … They are kept as acceptance guards rather than dropped."
- #229: "**Four cycles were green on arrival**" (cycles 5, 7, 10 and 13).
- #256: "Two tests passed on their first run."
- #265: "Guard tests show that two reviewers make two records … No change
  was necessary."

Each of these tests stays for good. Some of them assert a fact that an
earlier test already asserts (section 2 shows twelve proved cases). And each
new test file builds its own fixture, because the rule says "write a test"
and not "extend a test". That is why title 26 is parsed more than 100 times in one
run of the suite.

The rule also says to run the **full** suite after each cycle. At about 15
minutes of wall time on this machine (and 25 on a loaded one), that is the
largest cost that an agent pays per cycle.

### What I would add

I would keep the TDD rule. It finds real faults: #242 and #256 show agents
who used mutation to check a green test, and found that it could bite. I
would add four rules beside it.

> **A test that was never RED needs a reason.** When a new test passes on its
> first run, break the code it is about and show that the test fails. Put the
> mutation and the result in the PR. If no other test catches that fault,
> keep the new test. If another test catches it, put the new assertion into
> that test instead.

> **Extend before you add.** Before you write a new test, look for a test that
> already runs the same fixture and the same call. If there is one, add your
> assertion to it. The RED step is the new assertion failing, not a new
> function.

> **Delete what a later test covers.** In the refactor step, look at the tests
> this cycle touched. When a new test asserts everything an older test
> asserts, delete the older one in the same PR, and say so.

> **Parse a title once per test binary.** Put a parsed file in a `OnceLock`
> and clone it for each test. `tests/path_redesignation_tests.rs` shows how.
> Never parse title 26, 42 or 7 inside a helper that each test calls.

And I would change one line:

> Run the tests you touched after each cycle. Run the full suite once before
> you open the PR.

**Why.** The first three rules stop the growth at its source, and they do not
weaken TDD: each still needs a failing check before the code. The fourth
rule stops the most expensive pattern this audit found. The last change
cuts the per-cycle cost that pushed agents to batch work.

The maintainer decides. None of this is in this PR.

---

## 7. Recommendations, in order of value

"1 thread" is the debug single-thread saving, which is the best local model
of CI. Confidence is high when the change was measured or proved, and medium
when it is an estimate from measured parts.

| # | Recommendation | Tests removed or merged | Time saved | Confidence |
|---|---|---:|---|---|
| 1 | Build tests at `opt-level = 1` (`[profile.dev] opt-level = 1`, or the same under `[profile.test]`). Debug assertions stay on. | 0 | Parallel sum 930 s → 217 s (measured, −77%). Clean build +17 s. CI `rest` shard: about 1,100 s → about 300 s (estimate). | High (local, measured). Medium for CI. I measured the `dev` setting; I did not measure `[profile.test]` alone. |
| 2 | Parse each title once per binary and clone per test (section 4.1), in the 13 files listed. | 0 | About 730 s of 2,773 s, 1 thread. | Medium |
| 3 | `residue_tests`: run `residue --json` on the unchanged fixture once. | 0 | About 110 s of 564 s, 1 thread; about 90 s of 443 s on CI. | High |
| 4 | Drop the 12 tests proved redundant (D1–D10), after the one assertion that D10 needs. | 12 | About 43 s, 1 thread. | High |
| 5 | Fix the three tests that guard nothing (section 3): assert or delete `test_find_deeply_nested_structure`; move the two `inspect::diff` tests to title 51; merge the README workflow test. | 2 removed, 2 made real | About 14 s | High |
| 6 | Put the seven-title sweep tests in one binary, and build the sweep once. | 0 (3 moved) | About 100 s, 1 thread | Medium |
| 7 | Build `cli_add_bills_tests`' two datasets once each. | 0 | About 45 s | Medium |
| 8 | Consolidate the groups in section 5. | about 75 functions merged | About 20 s (fewer command runs, one bill parse) | High (coverage), low (time) |
| 9 | Prove and then drop the candidates in section 2.2. | about 20 | About 30 s | Medium |
| 10 | Add the companion rules of section 6 to `CLAUDE.md`. | 0 now | Stops growth | Medium |
| 11 | Decide on `Link::from_annotation`. If it leaves the public API, its tests go. | 11 (only if the API goes) | Under 1 s | High (the facts); the decision is the maintainer's |

**Totals.** Proved: 14 tests removed (4 and 5) and about 75 merged (8), from
665 functions to about 575. With the section 2.2 candidates, about 555. Time:
the single-thread debug sum falls from 2,773 s to about 1,900 s with 2–7; at
`opt-level = 1` the whole suite then runs in well under 200 s of summed
binary time.

### A note on CI shards (out of scope)

The shard list in `ci.yml` says that "the shard times have not been measured
again since" #252. The `rest` shard now holds `review_tests` (158 s),
`contradiction_tests` (130 s), `amendment_link_door_tests` (129 s) and
`unplaced_statement_tests` (103 s): 520 s of its 1,114 s. If recommendation 1
lands, the shards may not be needed at all. If it does not, move these four
out of `rest`.

---

## Appendix A: mutations

Each mutation was a temporary edit to `src/`. I ran the tests, then reverted
the file with `git checkout -- <file>` and checked that `git status` was
clean. The committed tree holds only this report. Every command used
`--no-fail-fast`, so that one failing binary did not hide the other.

**M1a.** `src/date.rs`: `7 => Ok(time::Month::July)` → `7 => Ok(time::Month::August)`.

```
$ cargo test --no-fail-fast --test date_tests --test utils_tests
exit=101
test test_date_str_to_date_valid ... FAILED
test test_valid_date_parsing ... FAILED
```

**M1b.** `src/date.rs`: `if date_split.len() != 3` → `if date_split.len() < 2`.

```
$ cargo test --no-fail-fast --test date_tests --test utils_tests
exit=101
test test_date_str_to_date_invalid_format ... FAILED
test test_invalid_date_too_many_components ... FAILED
test test_invalid_date_missing_components ... FAILED
```

**M1c.** `src/date.rs`: `Date::from_calendar_date(year_num, …)` → `(year_num + 1, …)`.

```
$ cargo test --no-fail-fast --test date_tests --test utils_tests
exit=101
test test_date_str_to_date_valid ... FAILED
test test_date_str_to_date_leap_year ... FAILED
test test_leap_year_feb_29 ... FAILED
test test_valid_date_parsing ... FAILED
```

**M2.** `src/bin/words_to_data/path.rs:147`: `"… §{}, {}{part}"` → `"… §{}; {}{part}"`.

```
$ cargo test --no-fail-fast --test olrc_reader_tests
exit=101
test should_show_the_olrc_row_when_path_reads_a_section_the_olrc_classified ... FAILED
test should_say_a_row_classifies_a_note_when_its_descriptions_are_all_notes ... FAILED
test should_say_a_row_classifies_a_heading_when_its_description_is_prec ... FAILED
test should_show_the_olrc_row_when_a_reviewer_explains_a_link_to_a_classified_section ... FAILED
```

**M3.** `src/legislature/residue/mod.rs:195`: `Category::NotHeld` → `Category::Quiet`.

```
$ cargo test --no-fail-fast --test residue_tests -- should_not_call_an_amendment_quiet should_report_an_amendment_to_a_note
exit=101
test should_not_call_an_amendment_quiet_when_the_code_held_has_nothing_at_its_address ... FAILED
test should_report_an_amendment_to_a_note_as_not_held_and_not_as_a_miss ... FAILED
  left: String("quiet")   right: "quiet"
  left: String("quiet")   right: "not_held"
```

**M4.** `src/diff/mod.rs:761`: `old_value: a.to_string()` → `old_value: b.to_string()`.

```
$ cargo test --no-fail-fast --test diff_tests --test website_example_tests -- test_diff_generation_26 website_example_compute_diff
exit=101
test test_diff_generation_26 ... FAILED
test website_example_compute_diff ... FAILED
```

**M6.** `src/document.rs`, `DocumentNode::find`: return `None` when the path has
six segments.

```
$ cargo test --no-fail-fast --test tree_navigation_tests --test edge_case_tests
exit=101
test test_empty_element_no_children ... FAILED
test test_deeply_nested_structure ... FAILED
test test_find_paragraph_element ... FAILED
```

**M9.** `src/document.rs`, `DocumentNode::find`: always return `None`.

```
$ cargo test --no-fail-fast --test tree_navigation_tests
exit=101
test test_find_deeply_nested_structure ... ok        <- guards nothing
test test_find_paragraph_element ... FAILED
test test_find_root_in_real_document ... FAILED
(8 failed, 6 passed; the 5 that passed besides the one above assert `is_none()`)
```

**M10.** `src/bin/words_to_data/expressions.rs:24`: the `--work` value is
dropped (`.filter(|_| false)`).

```
$ cargo test --no-fail-fast --test cli_tests -- should_list_only_the_named_work_when_expressions_is_given_one should_report_no_second_expression_for_a_work_published_once
exit=101
test should_report_no_second_expression_for_a_work_published_once ... FAILED
test should_list_only_the_named_work_when_expressions_is_given_one ... ok   <- guards nothing
```

**M11.** `src/inspect.rs:2048`: `inspect::diff` does not call
`collect_diff_paths`.

```
$ cargo test --no-fail-fast --test inspect_tests --test cli_tests --test scoped_report_tests
exit=101
test should_collect_changed_added_and_removed_paths_matching_the_tree_diff ... ok   <- guards nothing
test should_produce_identical_diff_summary_for_sqlite_backend ... ok             <- guards nothing
test should_account_coverage_against_the_real_diff ... FAILED
test should_report_every_changed_path_as_unannotated_when_nothing_is_annotated ... FAILED
test should_list_the_paths_that_changed_between_two_expressions_when_diff_runs ... FAILED
test should_list_only_the_changes_inside_a_path_when_diff_names_one ... FAILED
```

**M12.** `src/storage/memory.rs`, `compute_diff`: set `root_path` to
`"uscode"`.

```
$ cargo test --no-fail-fast --test dataset_tests --test sqlite_tests -- should_compute_diff_between_two_expressions_of_one_work should_query_via_trait_interface
exit=101
test should_compute_diff_between_two_expressions_of_one_work ... FAILED
test should_query_via_trait_interface ... FAILED
```

**M13.** `src/legislature/redesignation.rs:1333`: `to_path` →
`format!("{to_path}_x")`.

```
$ cargo test --no-fail-fast --test redesignation_tests -- should_resolve_the_paragraph_898_c_renumbered_when_title_26_is_in_hand should_report_a_renumbered_paragraph_as_moved_when_the_bill_said_so
exit=101
test should_resolve_the_paragraph_898_c_renumbered_when_title_26_is_in_hand ... FAILED
test should_report_a_renumbered_paragraph_as_moved_when_the_bill_said_so ... FAILED
```

**M14a.** `src/link.rs:527`: the reasoning cut to its first 10 characters.

```
$ cargo test --no-fail-fast --test link_tests
exit=101
test should_mark_unreviewed_model_output_as_machine_suggested ... FAILED
test should_carry_the_models_reasoning_as_evidence ... ok      <- the kept test catches more
```

**M14b.** `src/link.rs:527`: the reasoning dropped (`.filter(|_| false)`).

```
$ cargo test --no-fail-fast --test link_tests
exit=101
test should_mark_unreviewed_model_output_as_machine_suggested ... FAILED
test should_carry_the_models_reasoning_as_evidence ... FAILED
```

**M15.** `src/citation/usc.rs`, `find_with_report`: add the citations of
`"26 U.S.C. § 174"` to every answer.

```
$ cargo test --no-fail-fast --test usc_citation_tests --test usc_citation_report_tests -- should_find_nothing_when_the_text_has_no_citation should_report_nothing_when_the_text_names_no_statute
exit=101
test should_report_nothing_when_the_text_names_no_statute ... FAILED
test should_find_nothing_when_the_text_has_no_citation ... FAILED
```

## Appendix B: the runs

| Run | Command | Exit | Result |
|---|---|---|---|
| Build | `cargo test --no-run` | 0 | 81 executables |
| Run 1, parallel, debug | `cargo test -q --test <name>` for each of 79 files, then `--lib`, `--doc` | 0 for all 81 | 696 passed, 2 ignored; binaries sum 931 s |
| Run 2, parallel, debug | the same | 0 for all 81 | binaries sum 928 s |
| One thread, debug | `RUSTC_BOOTSTRAP=1 <binary> -Z unstable-options --report-time --test-threads=1` for each of 79 | 0 for all 79 | sum 2,773 s |
| Release | `cargo test --release -q --test <name>` for each | 0 for all 81 | binaries sum 196 s; clean build 55 s |
| `opt-level = 1` | `CARGO_TARGET_DIR=hacks/target-o1 CARGO_PROFILE_DEV_OPT_LEVEL=1 cargo test -q --test <name>` for each | 0 for all 81 | binaries sum 217 s; clean build 41 s |
| Clean debug build | `CARGO_TARGET_DIR=hacks/target-dbg cargo test --no-run` | 0 | 24 s |
