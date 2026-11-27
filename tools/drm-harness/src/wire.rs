//! Unix sockets that carry file descriptors (`SCM_RIGHTS`), and the wire
//! protocol of AGL drm-lease-manager's client library.
//!
//! Both the lease-manager protocol and the producer protocol of
//! `producer.rs` use `SOCK_SEQPACKET` sockets: one `sendmsg` is one
//! message, with at most one descriptor attached.

use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rustix::net::{
    self, AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketAddrUnix, SocketFlags, SocketType,
};

fn protocol(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// A connected pair of `SOCK_SEQPACKET` sockets, close-on-exec.
pub fn seqpacket_pair() -> io::Result<(OwnedFd, OwnedFd)> {
    Ok(net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )?)
}

/// Makes receives on `socket` give up after `timeout`.
pub fn set_receive_timeout(socket: BorrowedFd<'_>, timeout: Duration) -> io::Result<()> {
    Ok(net::sockopt::set_socket_timeout(
        socket,
        net::sockopt::Timeout::Recv,
        Some(timeout),
    )?)
}

/// Sends one message, with `pass` attached as `SCM_RIGHTS` if given.
pub fn send(socket: BorrowedFd<'_>, data: &[u8], pass: Option<BorrowedFd<'_>>) -> io::Result<()> {
    let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    let fds;
    if let Some(fd) = pass {
        fds = [fd];
        if !control.push(SendAncillaryMessage::ScmRights(&fds)) {
            return Err(protocol("control buffer too small"));
        }
    }
    let sent = net::sendmsg(
        socket,
        &[io::IoSlice::new(data)],
        &mut control,
        SendFlags::NOSIGNAL,
    )?;
    if sent != data.len() {
        return Err(protocol(format!("sent {sent} of {} bytes", data.len())));
    }
    Ok(())
}

/// A received message.
#[derive(Debug)]
pub struct Received {
    /// Bytes of the message; zero means the peer closed the socket.
    pub len: usize,
    /// Descriptors that came with it, in order.
    pub fds: Vec<OwnedFd>,
}

/// Receives one message into `buf`. Descriptors are received close-on-exec.
/// A message longer than `buf` or with too many descriptors is an error.
pub fn recv(socket: BorrowedFd<'_>, buf: &mut [u8]) -> io::Result<Received> {
    // Room for a few descriptors, so that a peer sending too many is seen
    // (and its descriptors closed) rather than silently truncated.
    let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(4))];
    let mut control = RecvAncillaryBuffer::new(&mut space);
    let message = net::recvmsg(
        socket,
        &mut [io::IoSliceMut::new(buf)],
        &mut control,
        RecvFlags::CMSG_CLOEXEC,
    )?;
    let mut fds = Vec::new();
    for item in control.drain() {
        if let RecvAncillaryMessage::ScmRights(received) = item {
            fds.extend(received);
        }
    }
    if message.flags.contains(ReturnFlags::TRUNC) {
        return Err(protocol("message longer than expected"));
    }
    if message.flags.contains(ReturnFlags::CTRUNC) {
        return Err(protocol("too many descriptors in one message"));
    }
    Ok(Received {
        len: message.bytes,
        fds,
    })
}

// ---------------------------------------------------------------------------
// drm-lease-manager
//
// Read from AGL src/drm-lease-manager at commit 68c48758c46d (the protocol
// is described with line references in docs/dlm-aarch64.md):
// - common/dlm-protocol.h:21-28: `enum dlm_opcode { DLM_GET_LEASE,
//   DLM_RELEASE_LEASE }`, `struct dlm_client_request { enum dlm_opcode
//   opcode; }`: a request is one native-endian 32-bit integer.
// - common/dlm-protocol.c:24-66: the request is one message of exactly
//   that size; the server rejects any other length.
// - common/dlm-protocol.c:68-149 (`receive_lease_fd`, `send_lease_fd`): the
//   reply is one message of one byte (contents undefined) with exactly one
//   descriptor, the lease, as SCM_RIGHTS. A refusal is the server closing
//   the socket (the client reports EACCES); a number of descriptors other
//   than one is EPROTO.
// - common/socket-path.c:27-48: the socket is `<dir>/<lease name>`, `<dir>`
//   being $DLM_RUNTIME_PATH or the compiled-in runtime directory,
//   `<localstatedir>/run/drm-lease-manager`.
// - libdlmclient/dlmclient.c:51-58: AF_UNIX, SOCK_SEQPACKET, connect.
// - libdlmclient/dlmclient.c:108-157: get lease = connect, request, receive;
//   release = send DLM_RELEASE_LEASE, close the lease descriptor and the
//   socket. drm-lease-manager revokes the lease on release or disconnect.
// ---------------------------------------------------------------------------

/// The runtime directory drm-lease-manager uses when `DLM_RUNTIME_PATH` is
/// not set, for a build with the usual `/var` local state directory.
pub const DLM_DEFAULT_RUNTIME_PATH: &str = "/var/run/drm-lease-manager";

/// A request of the lease-manager protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DlmRequest {
    /// `DLM_GET_LEASE`: send me the lease.
    GetLease,
    /// `DLM_RELEASE_LEASE`: I am done with it.
    ReleaseLease,
}

impl DlmRequest {
    /// Size of a request on the wire, `sizeof(struct dlm_client_request)`.
    pub const SIZE: usize = 4;

    /// The request as sent: the opcode as a native-endian C `int`.
    pub fn encode(self) -> [u8; Self::SIZE] {
        let opcode: u32 = match self {
            DlmRequest::GetLease => 0,
            DlmRequest::ReleaseLease => 1,
        };
        opcode.to_ne_bytes()
    }

    /// Parses a request. `Ok(None)` is a well-formed request with an
    /// unknown opcode, which drm-lease-manager logs and ignores.
    pub fn decode(bytes: &[u8]) -> io::Result<Option<Self>> {
        let bytes: [u8; Self::SIZE] = bytes
            .try_into()
            .map_err(|_| protocol(format!("request of {} bytes, not 4", bytes.len())))?;
        Ok(match u32::from_ne_bytes(bytes) {
            0 => Some(DlmRequest::GetLease),
            1 => Some(DlmRequest::ReleaseLease),
            _ => None,
        })
    }
}

/// Where the socket of lease `name` is: `<dir>/<name>`, with `dir` given,
/// else `$DLM_RUNTIME_PATH`, else [`DLM_DEFAULT_RUNTIME_PATH`].
pub fn dlm_socket_path(dir: Option<&Path>, name: &str) -> PathBuf {
    let dir = match dir {
        Some(dir) => dir.to_path_buf(),
        None => std::env::var_os("DLM_RUNTIME_PATH")
            .map_or_else(|| PathBuf::from(DLM_DEFAULT_RUNTIME_PATH), PathBuf::from),
    };
    dir.join(name)
}

/// Client side, on a connected socket: sends `DLM_GET_LEASE` and receives
/// the lease descriptor. A refusal (the server closes the socket) is
/// `PermissionDenied`, as libdlmclient's EACCES.
pub fn dlm_request_lease(socket: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    send(socket, &DlmRequest::GetLease.encode(), None)?;
    let mut byte = [0u8; 1];
    let mut received = recv(socket, &mut byte)?;
    if received.len == 0 && received.fds.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "lease refused: the lease manager closed the connection",
        ));
    }
    if received.fds.len() != 1 {
        return Err(protocol(format!(
            "lease reply with {} descriptors, not 1",
            received.fds.len()
        )));
    }
    Ok(received.fds.remove(0))
}

/// Server side: reads one request. `Ok(None)` means the client closed the
/// socket; an unknown opcode is `Ok(Some(None))`.
pub fn dlm_read_request(socket: BorrowedFd<'_>) -> io::Result<Option<Option<DlmRequest>>> {
    let mut buf = [0u8; 16];
    let received = recv(socket, &mut buf)?;
    if received.len == 0 {
        return Ok(None);
    }
    if !received.fds.is_empty() {
        return Err(protocol("request with descriptors"));
    }
    DlmRequest::decode(&buf[..received.len]).map(Some)
}

/// Server side: sends the lease descriptor, one byte of payload with it.
pub fn dlm_send_lease(socket: BorrowedFd<'_>, lease: BorrowedFd<'_>) -> io::Result<()> {
    send(socket, &[0], Some(lease))
}

/// A lease obtained from drm-lease-manager (or anything that speaks its
/// protocol, such as `drm-harness lessor`). Dropping it releases the lease.
#[derive(Debug)]
pub struct DlmLease {
    socket: OwnedFd,
    path: PathBuf,
}

impl DlmLease {
    /// Connects to `path` and asks for the lease. Returns the connection
    /// (keep it while the lease is used) and the lease descriptor.
    pub fn get(path: &Path) -> io::Result<(Self, OwnedFd)> {
        let socket = net::socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )?;
        let address = SocketAddrUnix::new(path)?;
        net::connect(&socket, &address)
            .map_err(|e| io::Error::new(io::Error::from(e).kind(), format!("connect: {e}")))?;
        let fd = dlm_request_lease(socket.as_fd())?;
        Ok((
            Self {
                socket,
                path: path.to_path_buf(),
            },
            fd,
        ))
    }

    /// The socket the lease came from.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DlmLease {
    fn drop(&mut self) {
        // Closing the socket alone would do too; the explicit release is
        // what libdlmclient sends.
        let _ = send(
            self.socket.as_fd(),
            &DlmRequest::ReleaseLease.encode(),
            None,
        );
    }
}

/// Binds a listening `SOCK_SEQPACKET` socket at `path`, replacing a stale
/// socket file as drm-lease-manager does.
pub fn listen(path: &Path) -> io::Result<OwnedFd> {
    let socket = net::socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )?;
    let address = SocketAddrUnix::new(path)?;
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    net::bind(&socket, &address)?;
    net::listen(&socket, 1)?;
    Ok(socket)
}

/// Accepts one connection, close-on-exec.
pub fn accept(listener: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    Ok(net::accept_with(listener, SocketFlags::CLOEXEC)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{Read, Seek, Write};

    fn temp_file(content: &[u8]) -> File {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "drm-harness-wire-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        std::fs::remove_file(&path).unwrap();
        file.write_all(content).unwrap();
        file
    }

    fn read_back(fd: OwnedFd) -> Vec<u8> {
        let mut file = File::from(fd);
        file.rewind().unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();
        content
    }

    #[test]
    fn requests_encode_as_a_native_endian_int() {
        assert_eq!(DlmRequest::GetLease.encode(), 0u32.to_ne_bytes());
        assert_eq!(DlmRequest::ReleaseLease.encode(), 1u32.to_ne_bytes());
        assert_eq!(DlmRequest::SIZE, 4);
        for request in [DlmRequest::GetLease, DlmRequest::ReleaseLease] {
            assert_eq!(
                DlmRequest::decode(&request.encode()).unwrap(),
                Some(request)
            );
        }
        assert_eq!(DlmRequest::decode(&7u32.to_ne_bytes()).unwrap(), None);
        assert!(DlmRequest::decode(&[0, 0, 0]).is_err());
        assert!(DlmRequest::decode(&[0; 8]).is_err());
    }

    #[test]
    fn socket_path_is_dir_and_name() {
        assert_eq!(
            dlm_socket_path(Some(Path::new("/run/x")), "card0-LVDS-1"),
            PathBuf::from("/run/x/card0-LVDS-1")
        );
    }

    #[test]
    fn a_descriptor_travels_with_a_message() {
        let (a, b) = seqpacket_pair().unwrap();
        let file = temp_file(b"lease");
        send(a.as_fd(), b"hello", Some(file.as_fd())).unwrap();
        let mut buf = [0u8; 16];
        let mut got = recv(b.as_fd(), &mut buf).unwrap();
        assert_eq!(&buf[..got.len], b"hello");
        assert_eq!(got.fds.len(), 1);
        assert_eq!(read_back(got.fds.remove(0)), b"lease");
        // Without a descriptor.
        send(a.as_fd(), b"x", None).unwrap();
        let got = recv(b.as_fd(), &mut buf).unwrap();
        assert_eq!((got.len, got.fds.len()), (1, 0));
        // End of stream.
        drop(a);
        let got = recv(b.as_fd(), &mut buf).unwrap();
        assert_eq!(got.len, 0);
    }

    #[test]
    fn a_message_longer_than_the_buffer_is_an_error() {
        let (a, b) = seqpacket_pair().unwrap();
        send(a.as_fd(), &[1; 32], None).unwrap();
        let mut buf = [0u8; 8];
        assert!(recv(b.as_fd(), &mut buf).is_err());
    }

    #[test]
    fn dlm_client_and_server_exchange_a_lease() {
        let (client, server) = seqpacket_pair().unwrap();
        let file = temp_file(b"drm lease");
        let server_thread = std::thread::spawn(move || {
            assert_eq!(
                dlm_read_request(server.as_fd()).unwrap(),
                Some(Some(DlmRequest::GetLease))
            );
            dlm_send_lease(server.as_fd(), file.as_fd()).unwrap();
            assert_eq!(
                dlm_read_request(server.as_fd()).unwrap(),
                Some(Some(DlmRequest::ReleaseLease))
            );
            assert_eq!(dlm_read_request(server.as_fd()).unwrap(), None);
        });
        let lease = dlm_request_lease(client.as_fd()).unwrap();
        assert_eq!(read_back(lease), b"drm lease");
        send(client.as_fd(), &DlmRequest::ReleaseLease.encode(), None).unwrap();
        drop(client);
        server_thread.join().unwrap();
    }

    #[test]
    fn a_closed_connection_is_a_refusal() {
        let (client, server) = seqpacket_pair().unwrap();
        let server_thread = std::thread::spawn(move || {
            assert_eq!(
                dlm_read_request(server.as_fd()).unwrap(),
                Some(Some(DlmRequest::GetLease))
            );
            drop(server);
        });
        let error = dlm_request_lease(client.as_fd()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        server_thread.join().unwrap();
    }

    #[test]
    fn a_reply_without_a_descriptor_is_a_protocol_error() {
        let (client, server) = seqpacket_pair().unwrap();
        let server_thread = std::thread::spawn(move || {
            dlm_read_request(server.as_fd()).unwrap();
            send(server.as_fd(), &[0], None).unwrap();
        });
        let error = dlm_request_lease(client.as_fd()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        server_thread.join().unwrap();
    }

    #[test]
    fn dlm_lease_over_a_socket_file() {
        // Short names: a socket path must fit in 108 bytes, and the
        // temporary directory may already be long.
        let dir = std::env::temp_dir().join(format!("dh{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dlm_socket_path(Some(&dir), "c0");
        let listener = listen(&path).unwrap();
        let file = temp_file(b"leased");
        let server_thread = std::thread::spawn(move || {
            let connection = accept(listener.as_fd()).unwrap();
            assert_eq!(
                dlm_read_request(connection.as_fd()).unwrap(),
                Some(Some(DlmRequest::GetLease))
            );
            dlm_send_lease(connection.as_fd(), file.as_fd()).unwrap();
            // Dropping the client sends the release.
            assert_eq!(
                dlm_read_request(connection.as_fd()).unwrap(),
                Some(Some(DlmRequest::ReleaseLease))
            );
        });
        let (lease, fd) = DlmLease::get(&path).unwrap();
        assert_eq!(lease.path(), path);
        assert_eq!(read_back(fd), b"leased");
        drop(lease);
        server_thread.join().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
