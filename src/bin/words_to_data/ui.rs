//! What a person at a terminal sees while a command works.
//!
//! **The stream contract.** stdout holds the answer: the report, the table, the
//! `--json` a pipe reads. stderr holds everything about the run: progress,
//! warnings and errors. A command that printed "Downloading bill..." on stdout
//! mixed the two, and a reader of its output had to skip the chatter (#125).
//!
//! **Progress draws only for a person.** A bar is drawn when stderr is a
//! terminal. In a pipe, a log or a test it is hidden, and only the one-line
//! result of each task is printed, so a log still says what ran and how long it
//! took.
//!
//! **Colour follows the stream.** [`console`] turns colour off when the stream
//! is not a terminal, or when `NO_COLOR` is set, so styled text is byte for byte
//! the plain text wherever a program reads it.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use console::{Style, StyledObject, Term, style};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use words_to_data::legislature::redesignation::{RedesignationReport, clause_start};
use words_to_data::progress::Progress;

/// One named piece of work with a live line on stderr, such as downloading one
/// bill or adding one release point.
///
/// The library reports its steps to the task through [`Progress`], and each
/// step replaces the detail after the title: "119-hr-1 · 432 members, 411
/// cached". [`Task::done`] replaces the live line with a line that stays.
pub struct Task {
    bar: ProgressBar,
    title: String,
    started: Instant,
    /// How many notes were printed, for the result line.
    notes: Mutex<usize>,
}

impl Task {
    /// Start a task. The line spins until the first step with a known size.
    pub fn start(title: impl Into<String>) -> Self {
        let title = title.into();
        let bar = ProgressBar::with_draw_target(None, ProgressDrawTarget::stderr());
        bar.set_style(spinner_style());
        bar.set_message(style(&title).bold().for_stderr().to_string());
        bar.enable_steady_tick(Duration::from_millis(100));
        Self {
            bar,
            title,
            started: Instant::now(),
            notes: Mutex::new(0),
        }
    }

    /// Finish the task, and leave `summary` beside its title with the time it
    /// took: "✓ 119-hr-1  432 members  3.2s".
    pub fn done(self, summary: &str) {
        let notes = *self.notes.lock().unwrap();
        let mut line = format!(
            "{} {}",
            style("✓").green().for_stderr(),
            style(&self.title).bold().for_stderr()
        );
        if !summary.is_empty() {
            line.push_str(&format!("  {summary}"));
        }
        line.push_str(&format!("  {}", dim(&elapsed(self.started.elapsed()))));
        if notes > 0 {
            line.push_str(&format!(
                "  {}",
                warn_text(&format!("{notes} note(s) above"))
            ));
        }
        self.bar.finish_and_clear();
        eprintln!("{line}");
    }

    /// Finish the task and leave no line, for work that was over too soon for
    /// a person to have wondered about it.
    fn clear(self) {
        self.bar.finish_and_clear();
    }

    /// Show `label` after the title, with a bar when `total` is known.
    fn show(&self, label: &str, total: Option<u64>, bar_style: ProgressStyle) {
        let message = format!(
            "{} {}",
            style(&self.title).bold().for_stderr(),
            dim(&format!("· {label}"))
        );
        self.bar.set_position(0);
        match total {
            Some(total) => {
                self.bar.set_length(total);
                self.bar.set_style(bar_style);
            }
            None => {
                self.bar.unset_length();
                self.bar.set_style(spinner_style());
            }
        }
        self.bar.set_message(message);
    }
}

impl Progress for Task {
    fn begin(&self, label: &str, total: Option<u64>) {
        self.show(label, total, count_style());
    }

    fn begin_bytes(&self, label: &str, total: Option<u64>) {
        self.show(label, total, bytes_style());
    }

    fn advance(&self, units: u64) {
        self.bar.inc(units);
    }

    fn note(&self, message: &str) {
        *self.notes.lock().unwrap() += 1;
        self.bar
            .suspend(|| eprintln!("  {} {message}", warn_text("!")));
    }
}

/// Run `work` under a spinner titled `title`, and print its result line.
///
/// For a step the library cannot report inside, such as opening or saving a
/// dataset: the person sees that it is working and, after, how long it took.
/// A step done in under a second leaves no line, so a small dataset opens
/// without a word and a large one says what the wait was.
pub fn step<T>(title: &str, work: impl FnOnce() -> T) -> T {
    let task = Task::start(title);
    let value = work();
    if task.started.elapsed() < WORTH_A_LINE {
        task.clear();
    } else {
        task.done("");
    }
    value
}

/// How long a [`step`] must take before its result line is worth printing.
const WORTH_A_LINE: Duration = Duration::from_secs(1);

/// Write a renumbering report to stderr: the summary, then the statements no
/// reader placed, grouped under the reason each was not placed.
///
/// One reason often covers many statements. A bill loaded before any window it
/// could be checked against leaves all 57 of its statements unplaced for one
/// reason, and 57 lines each ending in that reason hide it. So the reason is
/// written once, and the clauses under it.
///
/// **Every statement is written when stderr goes to a file or a pipe**, because
/// the tool's silence must not read as the corpus's silence (#110). At a
/// terminal a long group stops after a few clauses and says how many more there
/// are and which command lists them all.
pub fn warn_unplaced(bill: &str, report: &RedesignationReport) {
    let placed = report.statements() - report.statements_unplaced();
    let mark = if report.unplaced.is_empty() {
        style("✓").green().for_stderr()
    } else {
        style("!").yellow().for_stderr()
    };
    eprintln!(
        "{mark} {}  {placed} of {} renumbering statement(s) placed, {} link(s) recorded",
        style(format!("Renumberings of {bill}")).bold().for_stderr(),
        report.statements(),
        report.links(),
    );

    let mut by_reason: Vec<(String, Vec<String>)> = Vec::new();
    for unplaced in &report.unplaced {
        let reason = format!("{} reader: {}", unplaced.reader, unplaced.reason);
        match by_reason.iter_mut().find(|(known, _)| *known == reason) {
            Some((_, clauses)) => clauses.push(unplaced.clause_start()),
            None => by_reason.push((reason, vec![unplaced.clause_start()])),
        }
    }
    let shown_per_reason = if Term::stderr().is_term() {
        CLAUSES_SHOWN_AT_A_TERMINAL
    } else {
        usize::MAX
    };
    let mut hidden = 0;
    for (reason, clauses) in &by_reason {
        eprintln!("  {} {}", warn_text("not placed"), dim(reason));
        for clause in clauses.iter().take(shown_per_reason) {
            eprintln!("      {clause}");
        }
        hidden += clauses.len().saturating_sub(shown_per_reason);
    }
    for later in &report.later_windows {
        eprintln!(
            "  {} {}",
            warn_text("review"),
            dim(&format!(
                "{} -> {} also changed under this statement, and holds no link",
                later.from, later.to.at
            ))
        );
        eprintln!("      {}", clause_start(&later.text));
    }
    if hidden > 0 {
        eprintln!(
            "  {}",
            dim(&format!(
                "… and {hidden} more. To list them all: words_to_data redesignation-report <dataset> --bill-id {bill}"
            ))
        );
    }
}

/// How many clauses of one reason [`warn_unplaced`] shows at a terminal.
const CLAUSES_SHOWN_AT_A_TERMINAL: usize = 3;

/// A heading on stdout: bold where there is a terminal, plain elsewhere.
pub fn heading(text: &str) -> StyledObject<&str> {
    style(text).bold()
}

/// A figure on stdout that is the point of the line.
pub fn figure<D>(value: D) -> StyledObject<D> {
    style(value).cyan().bold()
}

/// Text on stdout that is good news: a check passed, nothing is left.
pub fn good<D>(value: D) -> StyledObject<D> {
    style(value).green()
}

/// Text on stdout that asks for attention: something is outstanding.
pub fn attention<D>(value: D) -> StyledObject<D> {
    style(value).yellow()
}

/// Text on stdout that is bad news: refuted, broken, incomplete.
pub fn bad<D>(value: D) -> StyledObject<D> {
    style(value).red()
}

/// Text on stdout that is context, not the answer: ids, dates, hints.
pub fn quiet<D>(value: D) -> StyledObject<D> {
    style(value).dim()
}

/// A count painted by `paint`, or dimmed when it is zero, so the eye goes to
/// the counts that are not.
pub fn unless_zero(count: usize, paint: fn(usize) -> StyledObject<usize>) -> StyledObject<usize> {
    if count == 0 {
        quiet(count)
    } else {
        paint(count)
    }
}

/// A shell command on stdout, printed so it can be copied and run.
pub fn command(text: &str) -> StyledObject<&str> {
    style(text).cyan()
}

fn warn_text(text: &str) -> String {
    Style::new()
        .yellow()
        .for_stderr()
        .apply_to(text)
        .to_string()
}

fn dim(text: &str) -> String {
    Style::new().dim().for_stderr().apply_to(text).to_string()
}

fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan} {msg}  {elapsed:.dim}")
        .expect("the spinner template is valid")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
}

fn count_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{spinner:.cyan} {msg}  {wide_bar:.cyan/blue} {pos}/{len}  {eta:.dim}",
    )
    .expect("the bar template is valid")
    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
    .progress_chars("━╸─")
}

fn bytes_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{spinner:.cyan} {msg}  {wide_bar:.cyan/blue} {bytes}/{total_bytes}  {bytes_per_sec:.dim}",
    )
    .expect("the bar template is valid")
    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
    .progress_chars("━╸─")
}

/// "850ms", "3.2s", "2m 05s".
fn elapsed(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1_000 {
        format!("{millis}ms")
    } else if millis < 60_000 {
        format!("{:.1}s", duration.as_secs_f64())
    } else {
        let secs = duration.as_secs();
        format!("{}m {:02}s", secs / 60, secs % 60)
    }
}
