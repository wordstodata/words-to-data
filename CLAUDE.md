It is important to make sure the code stays readable, and easy to follow and edit. It is of high importance that it is easy for humans to work on the software

This repo is Rust only. The PyO3 bindings were removed in September 2026, because the core data model is changing and each break cost three edits. Do not add bindings back without a decision from the user. Git history holds the old `src/python.rs` and `python/` if you need them.

## Testing (TDD Workflow)

Never, under any circumstances, mock data when writing tests. Always use real data provided in the tests/ folder. If you have trouble finding examples to process, ask the user for more samples.

Also, do not add tests with #[cfg(test)] to source files. Instead, add them to the `tests/` folder. Doc tests are acceptable.

When building new features or fixing bugs:
0. **Plan** — Confirm which behaviors to test, prioritize them, and design the public interface.
1. **Establish baseline** — Run the existing suite once, record pass/fail counts. Pre-existing failures are not your problem.
2. **Write test first** (RED) — test MUST fail before implementation exists. Name it like a spec: `should <expected behavior> when <condition>`.
3. **Run the new test** (targeted) to verify it fails. Show the command, exit code, and output.
4. **Write minimal implementation** (GREEN) — just enough to pass.
5. **Run the full suite** to verify nothing regressed vs baseline. Show the command, exit code, and output.
6. **Refactor** if needed, re-run tests to confirm nothing broke.

**One RED-GREEN cycle at a time.** Never batch multiple tests before implementing. Start with the simplest happy path, then progress to edge cases and error scenarios. After each cycle, ask what behavior to add next.

Per-cycle checklist:
- [ ] Test describes behavior, not implementation
- [ ] Test uses public interface only
- [ ] Test would survive an internal refactor
- [ ] Code is minimal for this test
- [ ] No speculative features added

Rules:
- NEVER write implementation before a failing test exists.
- Tests come from the spec/requirements, not from the implementation.
- Mock at system boundaries (external APIs, DB, time, filesystem). Avoid mocking your own modules.
- Always show the actual command, exit code, and relevant output.
- Run the full test suite after completing a feature, not just the new test.

Error classification:
- **Code error** → keep iterating
- **Access error** (permissions, auth, rate limit) → stop, report, wait
- **Environment error** (missing binary, wrong runtime) → stop, report, wait

## Annotator

The annotator prototype was removed in September 2026, because the link and verification model beneath it changed. Git history holds it. A new review tool comes after the core settles.

## SLEUTH

There is a branch called `sleuth` which builds an iced desktop app. It's also reasonably free form

## AI Building
If set off on your own autonomous task and you find yourself on the `main` branch, create a new one prefixed with `vibes/`. If filing a PR autonomously, it should target the `vibes` branch instead of `main`

## Communication
Communicate _only_ via  ASD-STE100 Simplified Technical English (STE)

## Agent skills

### Issue tracker

Issues live in GitHub Issues for `wordstodata/words-to-data`, through the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
