//! The HTTP agent every client shares, and the one retry it adds.
//!
//! A machine can hold an IPv6 address with no working route to a host that
//! also answers on IPv4 — a home network behind a VPN is the common case.
//! `ureq` walks a host's resolved addresses in order, and moves to the next one
//! only when a connection is *refused* or *times out*. Any other error ends the
//! request, so a first IPv6 address that is merely unreachable fails a request
//! that IPv4 would have answered. `curl` does not fail this way, which makes the
//! fault look like the tool's and not the network's.
//!
//! So a request that fails because the network or the host is unreachable is
//! made once more over IPv4 only. Forcing IPv4 from the start was rejected: it
//! would break a machine that reaches the internet over IPv6 alone, which this
//! retry leaves working, because its first attempt succeeds.
//!
//! **After one unreachable answer, every request goes over IPv4 at once
//! (#286).** On some networks the "unreachable" error comes back about once a
//! second when several connections ask for it at the same time. Parallel
//! requests then wait in line for an error, and 432 member requests take eight
//! minutes. The network does not change during a run, so it is asked once.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ureq::Agent;
use ureq::config::IpFamily;

/// Two agents: the default, and an IPv4-only one to retry with.
#[derive(Debug, Clone)]
pub struct Http {
    any: Agent,
    ipv4: Agent,
    /// Set when the network was unreachable once. Clones share it.
    unreachable_before: Arc<AtomicBool>,
}

impl Http {
    pub fn new() -> Self {
        Self {
            any: Agent::new_with_defaults(),
            ipv4: Agent::config_builder()
                .ip_family(IpFamily::Ipv4Only)
                .build()
                .new_agent(),
            unreachable_before: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Make a request, and make it again over IPv4 if the network was unreachable.
    ///
    /// `request` is given the agent to build the request on, so a caller's
    /// headers and error mapping stay where they are.
    pub fn call<T>(
        &self,
        request: impl Fn(&Agent) -> Result<T, ureq::Error>,
    ) -> Result<T, ureq::Error> {
        if self.unreachable_before.load(Ordering::Relaxed) {
            return request(&self.ipv4);
        }
        match request(&self.any) {
            Err(error) if is_unreachable(&error) => {
                self.unreachable_before.store(true, Ordering::Relaxed);
                request(&self.ipv4)
            }
            answered => answered,
        }
    }
}

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether a request failed because no route led to the host.
fn is_unreachable(error: &ureq::Error) -> bool {
    matches!(
        error,
        ureq::Error::Io(io_error) if matches!(
            io_error.kind(),
            io::ErrorKind::NetworkUnreachable | io::ErrorKind::HostUnreachable
        )
    )
}
