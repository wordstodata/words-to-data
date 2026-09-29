//! How a long step says what it is doing, without knowing who is watching.
//!
//! The library does slow work: it downloads a release point of the Code, parses
//! fifty-seven titles, and fetches a member record for every voter on a roll
//! call. A command that runs that work in silence looks stuck. The first bill a
//! person ever downloads makes about 432 member requests, and before this hook
//! it printed one line, "Downloading bill 119-hr-1...", and then nothing for
//! minutes (#124).
//!
//! The library does not draw anything. It reports to a [`Progress`], and the
//! caller decides what a report looks like. The command line draws bars on
//! stderr; a test or a library user passes [`Silent`] and sees nothing.

/// Someone who wants to hear how a long step is going.
///
/// Every method has a default that does nothing, so a watcher implements only
/// what it draws. `Sync`, because some steps report from several threads.
pub trait Progress: Sync {
    /// A new piece of work starts. `total` is how many units it has, when that
    /// is known before the work starts, and `None` when it is not.
    fn begin(&self, _label: &str, _total: Option<u64>) {}

    /// [`Self::begin`] for work counted in bytes, such as a download, so a
    /// watcher can show "31 MiB of 402 MiB" rather than a bare count.
    fn begin_bytes(&self, label: &str, total: Option<u64>) {
        self.begin(label, total);
    }

    /// `units` more units of the current piece of work are done.
    fn advance(&self, _units: u64) {}

    /// Something a person must see, such as a record that could not be fetched
    /// and was left out. Not an error: the work goes on.
    fn note(&self, _message: &str) {}
}

/// A watcher that draws nothing.
pub struct Silent;

impl Progress for Silent {}
