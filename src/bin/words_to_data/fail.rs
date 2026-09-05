//! Reporting a failure to the person who ran the command.
//!
//! `Result::expect` prints the `Debug` form, so
//! `WorkMismatch { from: WorkId("uscode/title_51"), .. }` reaches the terminal
//! instead of the sentence written for a reader. Every error in this crate
//! carries a `Display` message saying what went wrong and what to do about it.
//! This is how it gets out.

use std::fmt::Display;

/// Exit code for a command that failed at its work. Distinct from clap's 2,
/// which means the arguments were wrong before any work started.
const FAILED: i32 = 1;

/// Unwrap, or print `context: <message>` to stderr and exit non-zero.
pub fn or_exit<T, E: Display>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{context}: {error}");
            std::process::exit(FAILED);
        }
    }
}
