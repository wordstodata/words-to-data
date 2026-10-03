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

/// The network that was unreachable once is not tried again (#286).
///
/// On some networks the "unreachable" error for IPv6 comes back about once a
/// second when several connections ask at the same time. Every parallel request
/// then waits in line for it, and a fetch of 432 members takes eight minutes.
#[test]
fn should_go_straight_to_ipv4_when_the_network_was_unreachable_before() {
    let http = Http::new();
    let _ = http.call(|agent| match agent.config().ip_family() {
        IpFamily::Ipv4Only => Ok(()),
        _ => Err(ureq::Error::Io(io::Error::from(
            io::ErrorKind::NetworkUnreachable,
        ))),
    });

    let families = std::cell::RefCell::new(Vec::new());
    let answered = http.call(|agent| {
        families.borrow_mut().push(agent.config().ip_family());
        Ok::<_, ureq::Error>(())
    });

    assert!(answered.is_ok());
    assert_eq!(
        families.into_inner(),
        vec![IpFamily::Ipv4Only],
        "the next request should be made over IPv4 alone"
    );
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
