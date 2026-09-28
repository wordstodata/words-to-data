//! The HTTP fallback every client shares (`words_to_data::http`).
//!
//! A machine can hold an IPv6 address with no working route to a host that
//! also answers on IPv4. `ureq` tries the next resolved address only when a
//! connection is refused or times out, so one unreachable IPv6 address ends a
//! request that IPv4 would have answered. These tests pin the retry that closes
//! that gap, at the one boundary the tool does not own: the network.

use std::cell::Cell;
use std::io;

use ureq::config::IpFamily;
use words_to_data::http::Http;

#[test]
fn should_retry_over_ipv4_when_the_network_is_unreachable() {
    let http = Http::new();
    let calls = Cell::new(0);

    let answered = http.call(|agent| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            return Err(ureq::Error::Io(io::Error::from(
                io::ErrorKind::NetworkUnreachable,
            )));
        }
        Ok(agent.config().ip_family())
    });

    assert_eq!(calls.get(), 2, "an unreachable network is tried once more");
    assert_eq!(answered.unwrap(), IpFamily::Ipv4Only);
}

#[test]
fn should_not_retry_when_the_host_answered_with_an_error() {
    let http = Http::new();
    let calls = Cell::new(0);

    let answered: Result<(), _> = http.call(|_| {
        calls.set(calls.get() + 1);
        Err(ureq::Error::StatusCode(404))
    });

    assert_eq!(calls.get(), 1, "a host that answered is not asked again");
    assert!(matches!(answered, Err(ureq::Error::StatusCode(404))));
}
