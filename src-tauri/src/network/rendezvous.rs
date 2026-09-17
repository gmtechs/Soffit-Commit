//! LAN rendezvous — resolves a short, human-typable pairing code to a peer's
//! iroh `EndpointAddr` **without any server**.
//!
//! Why this exists: dialing an iroh endpoint requires the peer's 32-byte
//! public key, and no encoding makes 32 bytes human-short. A short code can
//! therefore only work if something *resolves* it to an address. There is no
//! rendezvous service in this project, so the device that displays a code
//! announces `<code> -> <EndpointAddr>` on an IPv4 multicast group, and the
//! device that types the code listens for it.
//!
//! Scope: both devices must be on the same local network (same Wi-Fi/Ethernet,
//! or the same machine). For devices on different networks the portable
//! `<CODE>:<base64 ticket>` form is used instead — that one embeds the
//! address directly, so it needs no rendezvous.
//!
//! The transport is deliberately dumb: one datagram per announcement, no
//! acknowledgement, no state to corrupt. If anything is missed, the next
//! announcement 1.2 s later covers it.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

/// Administratively-scoped IPv4 multicast group (239.255.0.0/16) so the
/// traffic never leaves the local network. TTL is pinned to 1 for the same
/// reason.
const GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 42, 99);
const PORT: u16 = 45871;
/// Prefix identifying our datagrams so unrelated traffic on the port is ignored.
const MAGIC: &str = "SOFFITRV1";
const ANNOUNCE_EVERY: Duration = Duration::from_millis(1200);
/// How long a heard announcement stays usable if the announcer stops.
const REMOTE_TTL: Duration = Duration::from_secs(120);
const MAX_DATAGRAM: usize = 8192;
/// `|` never appears in JSON or in a normalized code, so it is a safe separator.
const SEP: char = '|';

struct LocalAnnounce {
    endpoint_addr: String,
    expires: Instant,
}

struct RemoteAnnounce {
    endpoint_addr: String,
    seen: Instant,
}

/// Strip formatting and normalize case. `k7f2-qp3m` and `K7F2 QP3M` both
/// become `K7F2QP3M`, so users can type codes with or without the dash.
pub fn normalize(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase()
}

pub struct Rendezvous {
    /// `None` when the socket could not be created (e.g. multicast unavailable).
    /// Pairing then falls back to the portable ticket form instead of failing
    /// to start the whole networking stack.
    ///
    /// This is a *blocking* std socket on purpose: `Rendezvous::start()` is
    /// called from Tauri's `setup()`, which has no Tokio context, and
    /// `tokio::net::UdpSocket::from_std` panics outside a runtime. The socket
    /// is only handed to the runtime later, inside `spawn_loops`.
    socket: Option<Arc<std::net::UdpSocket>>,
    group: SocketAddr,
    locals: Mutex<HashMap<String, LocalAnnounce>>,
    remote: Mutex<HashMap<String, RemoteAnnounce>>,
}

impl Rendezvous {
    /// Bind the multicast socket. Never fails: a disabled rendezvous only
    /// means short codes will not resolve on this machine.
    pub fn start() -> Arc<Self> {
        let socket = match bind_socket() {
            Ok(sock) => Some(Arc::new(sock)),
            Err(err) => {
                eprintln!("[rendezvous] disabled, short codes will not resolve locally: {err}");
                None
            }
        };
        Arc::new(Self {
            socket,
            group: SocketAddr::from((GROUP, PORT)),
            locals: Mutex::new(HashMap::new()),
            remote: Mutex::new(HashMap::new()),
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.socket.is_some()
    }

    /// Announce `code` on the LAN for `ttl`. Call this while the code is on
    /// screen; the announcer prunes it automatically once it expires.
    pub fn publish(&self, code: &str, endpoint_addr: &str, ttl: Duration) {
        if self.socket.is_none() || endpoint_addr.trim().is_empty() {
            return;
        }
        let code = normalize(code);
        if code.is_empty() {
            return;
        }
        self.locals
            .lock()
            .expect("rendezvous locals poisoned")
            .insert(
                code,
                LocalAnnounce {
                    endpoint_addr: endpoint_addr.to_string(),
                    expires: Instant::now() + ttl,
                },
            );
    }

    /// Stop announcing a code (called when the pairing modal is dismissed).
    pub fn unpublish(&self, code: &str) {
        self.locals
            .lock()
            .expect("rendezvous locals poisoned")
            .remove(&normalize(code));
    }

    /// Run the announce + listen loops until the runtime shuts down. Requires
    /// a Tokio runtime; both loops run concurrently in one task.
    pub async fn spawn_loops(self: Arc<Self>) {
        let Some(std_socket) = self.socket.clone() else {
            return;
        };
        // Attach to the runtime here (not in `start`) — this runs inside the
        // Tokio runtime, so `from_std` is legal. `try_clone` dups the same
        // socket, so both loops share one multicast membership and port.
        let (Some(send_socket), Some(recv_socket)) = (to_async(&std_socket), to_async(&std_socket))
        else {
            eprintln!("[rendezvous] could not attach the socket to the async runtime");
            return;
        };
        let announcer = {
            let this = self.clone();
            tokio::spawn(async move { this.announce_loop(send_socket).await })
        };
        let listener = {
            let this = self.clone();
            tokio::spawn(async move { this.listen_loop(recv_socket).await })
        };
        let _ = tokio::join!(announcer, listener);
    }

    async fn announce_loop(self: Arc<Self>, socket: UdpSocket) {
        let mut ticker = tokio::time::interval(ANNOUNCE_EVERY);
        loop {
            ticker.tick().await;
            let now = Instant::now();
            let datagrams: Vec<String> = {
                let mut locals = self.locals.lock().expect("rendezvous locals poisoned");
                locals.retain(|_, announce| announce.expires > now);
                locals
                    .iter()
                    .map(|(code, announce)| {
                        format!("{MAGIC}{SEP}{code}{SEP}{}", announce.endpoint_addr)
                    })
                    .collect()
            };
            for datagram in datagrams {
                // A send failure here is transient by nature (interface down,
                // no route yet) and the next tick retries — never fatal.
                let _ = socket.send_to(datagram.as_bytes(), self.group).await;
            }
        }
    }

    async fn listen_loop(self: Arc<Self>, socket: UdpSocket) {
        let mut buf = vec![0u8; MAX_DATAGRAM];
        loop {
            let Ok((len, _from)) = socket.recv_from(&mut buf).await else {
                continue;
            };
            let Ok(text) = std::str::from_utf8(&buf[..len]) else {
                continue;
            };
            let mut parts = text.splitn(3, SEP);
            if parts.next() != Some(MAGIC) {
                continue;
            }
            let (Some(code), Some(endpoint_addr)) = (parts.next(), parts.next()) else {
                continue;
            };
            if endpoint_addr.trim().is_empty() {
                continue;
            }
            let code = normalize(code);
            if code.is_empty() {
                continue;
            }
            self.remote
                .lock()
                .expect("rendezvous remote poisoned")
                .insert(
                    code,
                    RemoteAnnounce {
                        endpoint_addr: endpoint_addr.to_string(),
                        seen: Instant::now(),
                    },
                );
        }
    }

    /// Wait until a device announcing `code` is heard on the local network.
    /// Returns its `EndpointAddr` JSON, or `None` if `timeout` elapses first.
    pub async fn resolve(&self, code: &str, timeout: Duration) -> Option<String> {
        if self.socket.is_none() {
            return None;
        }
        let code = normalize(code);
        if code.is_empty() {
            return None;
        }
        let deadline = Instant::now() + timeout;
        loop {
            {
                let mut remote = self.remote.lock().expect("rendezvous remote poisoned");
                remote.retain(|_, announce| announce.seen.elapsed() < REMOTE_TTL);
                if let Some(announce) = remote.get(&code) {
                    return Some(announce.endpoint_addr.clone());
                }
            }
            if Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }

    /// Every code currently heard on the LAN (used by the UI to show when the
    /// other device is waiting).
    pub fn heard_codes(&self) -> Vec<String> {
        let mut remote = self.remote.lock().expect("rendezvous remote poisoned");
        remote.retain(|_, announce| announce.seen.elapsed() < REMOTE_TTL);
        remote.keys().cloned().collect()
    }
}

/// Hand a blocking socket to the async runtime.
///
/// Dups the descriptor (so the original keeps its multicast membership) and
/// switches it to non-blocking, which `from_std` requires. Must be called from
/// within a Tokio runtime.
fn to_async(socket: &std::net::UdpSocket) -> Option<UdpSocket> {
    let clone = socket.try_clone().ok()?;
    clone.set_nonblocking(true).ok()?;
    UdpSocket::from_std(clone).ok()
}

/// Multicast socket shared by both loops.
///
/// `SO_REUSEADDR` is required so two Soffit Commit instances on the same
/// machine can bind the same port — with multicast, every socket joined to
/// the group receives a copy of each datagram, which is exactly what the
/// two-instance test scenario needs. Loopback is enabled so those two local
/// instances can see each other.
fn bind_socket() -> anyhow::Result<std::net::UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, PORT)).into())?;
    // INADDR_ANY as the interface lets the kernel pick the default multicast
    // route, which covers both "one machine" and "normal LAN" cases.
    socket.join_multicast_v4(&GROUP, &Ipv4Addr::UNSPECIFIED)?;
    socket.set_multicast_loop_v4(true)?;
    socket.set_multicast_ttl_v4(1)?;
    socket.set_nonblocking(true)?;
    let std_socket: std::net::UdpSocket = socket.into();
    Ok(std_socket)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_formatting_and_case() {
        assert_eq!(normalize("k7f2-qp3m"), "K7F2QP3M");
        assert_eq!(normalize(" K7F2 QP3M "), "K7F2QP3M");
        assert_eq!(normalize("---"), "");
    }

    /// Guards against the resolution test passing vacuously: if the second
    /// socket cannot bind the multicast port, `announces_and_resolves_*` would
    /// skip instead of proving anything. This asserts both really can bind.
    #[test]
    fn two_sockets_can_share_the_multicast_port() {
        let a = Rendezvous::start();
        let b = Rendezvous::start();
        assert!(a.is_enabled(), "first multicast socket failed to bind");
        assert!(
            b.is_enabled(),
            "second multicast socket failed to bind — two instances on one machine would not see each other"
        );
    }

    /// End-to-end check that one instance's announcement reaches another
    /// instance on this machine (the real two-instance scenario).
    #[tokio::test]
    async fn announces_and_resolves_over_multicast() {
        let announcer = Rendezvous::start();
        let listener = Rendezvous::start();
        assert!(
            announcer.is_enabled() && listener.is_enabled(),
            "multicast unavailable — cannot verify short-code resolution here"
        );
        tokio::spawn(announcer.clone().spawn_loops());
        tokio::spawn(listener.clone().spawn_loops());

        announcer.publish("K7F2QP3M", r#"{"id":"peer-a","addrs":[]}"#, Duration::from_secs(30));

        let resolved = listener.resolve("k7f2-qp3m", Duration::from_secs(10)).await;
        assert_eq!(resolved.as_deref(), Some(r#"{"id":"peer-a","addrs":[]}"#));
    }

    #[tokio::test]
    async fn unknown_code_times_out() {
        let listener = Rendezvous::start();
        if !listener.is_enabled() {
            return;
        }
        tokio::spawn(listener.clone().spawn_loops());
        assert!(listener
            .resolve("ZZZZZZZZ", Duration::from_millis(400))
            .await
            .is_none());
    }
}