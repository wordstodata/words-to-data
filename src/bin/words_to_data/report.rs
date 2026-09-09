//! Telling the person who ran a sweep what it lost.
//!
//! An LLM sweep drops any amendment whose reply does not parse. The reply is not
//! kept — it backs no statement, so it is not evidence (`docs/adr/0005`) — which
//! leaves the run itself as the only place the loss is visible. Silence here
//! means an amendment goes missing while every total says the run went fine.

/// How many ids to name before falling back to a count.
///
/// A sweep over a whole Congress can fail on hundreds. Naming every one buries
/// the totals printed after it, and nobody reads the four hundredth id.
const NAMED: usize = 10;

/// Report the amendments a sweep lost, naming as many as will fit.
///
/// The id is what a person greps for in stderr to read the model's raw text, so
/// a bare count would say a loss happened without saying where to look.
/// Prints nothing when nothing failed: a clean run stays quiet.
pub fn failed_amendments(failed: &[String]) {
    if failed.is_empty() {
        return;
    }

    let noun = if failed.len() == 1 {
        "amendment"
    } else {
        "amendments"
    };
    println!("\n{} {noun} failed:", failed.len());

    // One id per line. An id is 64 characters, so ten of them on one line is a
    // wall of hex that a person has to scroll sideways through to read.
    for id in failed.iter().take(NAMED) {
        println!("  {id}");
    }
    let rest = failed.len().saturating_sub(NAMED);
    if rest > 0 {
        println!("  and {rest} more");
    }

    println!("Their replies are not kept. Run the command again to retry them.");
    println!("What the model sent is on stderr above, under the same ids.");
}
