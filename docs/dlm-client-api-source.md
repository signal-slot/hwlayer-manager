# drm-lease-manager client interface: read from source

Status: **read from source (2026-11-28)**. Nothing in this document was run; it is the reading of the source tree only. Licences are not recorded here (see `docs/third-party-licences.md`); build steps are in `docs/dlm-aarch64.md`.

## Source and pin

- Repository: `https://gerrit.automotivelinux.org/gerrit/src/drm-lease-manager`
- Commit read: `68c48758c46d93c8e83abb2fb1ec168fa13d1a53`, the commit that `docs/dlm-aarch64.md` pins. The tree was a detached checkout at that hash in the machine's dependency cache (`dlm2/drm-lease-manager`); `HEAD` equalled the pin. Meson project version `0.1.0` (`meson.build`).
- All `file:line` references below are to that commit.

## 1. Public client API (read from source, 2026-11-28)

`libdlmclient/dlmclient.h` declares four functions and one opaque type. Nothing else is installed (`libdlmclient/meson.build` installs only `dlmclient.h`, plus a `libdlmclient` pkg-config file).

| Function | Behaviour (`dlmclient.c` line) |
| --- | --- |
| `void dlm_enable_debug_log(bool)` | switches the library's debug log (`:32-35`) |
| `struct dlm_lease *dlm_get_lease(const char *name)` | connect to the socket named `name`, send `DLM_GET_LEASE`, receive the fd. Returns the handle, or `NULL` with `errno` set (`:108-138`). Header table of `errno`: `EACCES` (socket directory not accessible, or request denied), `ENAMETOOLONG`, `ENOENT` (no manager or no such lease), `ENOMEM`, `EPROTO`, and others from socket calls (`dlmclient.h:47-60`) |
| `int dlm_lease_fd(struct dlm_lease *)` | the DRM master fd of the lease, `-1` for a `NULL` handle (`:151-157`) |
| `void dlm_release_lease(struct dlm_lease *)` | sends `DLM_RELEASE_LEASE`, then closes the lease fd and the socket and frees the handle; a `NULL` handle is ignored (`:140-149`) |

The README says a client replaces `drmOpen()`/`drmClose()` with these calls (`README.md`, "Client API usage"). The example `examples/dlm-client-test/dlm-client-test.c:68-87` is the whole sequence: `dlm_get_lease(argv[1])`, `dlm_lease_fd`, `drmModeGetLease` and resource listing on the fd, `dlm_release_lease`.

Points that matter to a caller:

- `dlm_get_lease` is blocking: `connect` and `recvmsg` retry on `EINTR` and otherwise wait (`dlmclient.c:57-65`, `dlm-protocol.c:83-91`). There is no timeout argument. *Inferred:* a caller that must not hang needs its own timeout around the call.
- The handle owns both the socket and the fd. `dlm_release_lease` closes the fd, so the caller must not close it too (`dlmclient.h:63-70`).
- If the fd arrives but the call fails later in `dlm_get_lease`, it sends `DLM_RELEASE_LEASE` and closes the socket (`dlmclient.c:130-137`).

## 2. Socket protocol and versioning (read from source, 2026-11-28)

- Transport: `AF_UNIX`, `SOCK_SEQPACKET` (`dlmclient.c:51`; server side `lease-server.c:191`). One `sendmsg` is one message.
- The socket path is `<dir>/<lease name>`. `<dir>` is `$DLM_RUNTIME_PATH` if set, else the compiled-in `DLM_DEFAULT_RUNTIME_PATH`; the path must fit `sun_path` or `ENAMETOOLONG` (`common/socket-path.c:27-48`).
- Compiled-in default: `localstatedir` joined with option `runtime_subdir`, default `run/drm-lease-manager` (`meson.build`, `meson_options.txt`). With the usual `/var` the README gives `/var/run/drm-lease-manager`. `docs/dlm-aarch64.md` passes `--localstatedir var` for its staged install.
- Request: `struct dlm_client_request { enum dlm_opcode opcode; }`, opcodes `DLM_GET_LEASE = 0`, `DLM_RELEASE_LEASE = 1` (`common/dlm-protocol.h:21-28`). On the wire this is the native-endian `int`, 4 bytes on the targets here. The receiver rejects any message of another length with `EPROTO` (`dlm-protocol.c:42-45`); an unknown opcode is logged and ignored by the server (`lease-server.c:131-133`).
- Reply: one message of one byte (content undefined) with exactly one fd as `SCM_RIGHTS` (`dlm-protocol.c:68-149`). Zero fds or more than one is `EPROTO` (more than one: all are closed). A **refusal is the server closing the connection**: the client's `recvmsg` returns 0 and the library sets `EACCES` (`dlm-protocol.c:83-87`).
- Versioning: **there is none in the protocol.** No version field, handshake or capability exchange appears in `dlm-protocol.[ch]`, `dlmclient.c` or `lease-server.c`. The only version in the tree is the project version `0.1.0`, used as the shared-library version of `libdlmclient` (`libdlmclient/meson.build:12`) and in the pkg-config file. *Inferred:* client and daemon must be built from compatible trees; a change to the opcode enum or message size would not be detected except as `EPROTO` or an ignored request.

## 3. Lease naming (read from source, 2026-11-28)

There are two cases, and only the first is connector-based.

1. **No usable configuration** (file missing, unparsable, or no `lease` array: `parse_config` returns 0): one lease per DRM connector, named `card<minor>-<connector name>`. `<minor>` is `minor(st_rdev)` of the DRM device node; `<connector name>` is `<type>-<connector_type_id>` (`lease-manager.c:96-125`, `:467-505`). The type strings are in `connector_type_names` (`:74-94`), for example `HDMI-A`, `DSI`, `eDP`, `DP`, `LVDS`, `DPI`, `Virtual`. A connector of unknown type uses its connector id instead (`:104-107`). Example: `card0-HDMI-A-1`.
2. **With a configuration file**: the lease name is the `name` string of the `[[lease]]` entry, any string. It need not mention a connector (`lease-config.c:134-140`). The socket file is `<dir>/<name>`, so the name is also a file name.

`docs/dlm-aarch64.md` and `doc/configuration-guide.md` write the default pattern as `<DRM device minor number>-<connector name>`; the code has the literal prefix `card` (`lease-manager.c:121`). The README's `card0-LVDS-1` example agrees with the code.

## 4. Configuration (read from source, 2026-11-28)

Command line (`drm-lease-manager/main.c`):

| Option | Effect |
| --- | --- |
| `[<DRM device>]` (positional) | device node; if absent, the first device from `drmGetDevices2` that opens and has resources (`lease-manager.c:590-611`) |
| `-c`, `--config <path>` | TOML file, default `/etc/drm-lease-manager.toml` (`main.c:60`) |
| `-t`, `--lease-transfer` | a new client may take over a granted lease |
| `-k`, `--keep-on-crash` | do not close the lease fd when a client disconnects without releasing |
| `-u`, `--enable-universal-plane` | set `DRM_CLIENT_CAP_UNIVERSAL_PLANES`, so primary and cursor planes can be assigned |
| `-v`, `--verbose` | debug log |

Source quirks seen, not tested: the short-option string is `"vtkhcu:"` (`main.c:46`), so `-c` as a *short* option takes no argument while `--config` does, and `-u` takes one although the usage text treats it as a flag; the `usage()` `printf` has a stray comma and a mismatched argument list (`:33-43`). *Inferred:* use the long forms `--config` and `--enable-universal-plane`. For `--enable-universal-plane` the table declares `required_argument` (`:53`), so it also needs an argument; this is *not tested*, and `docs/dlm-aarch64.md` is the place for the command line actually used.

TOML (`lease-config.c`, `doc/configuration-guide.md`, README "Configuration"):

```toml
[[lease]]
name = "front-display"
connectors = ["HDMI-A-1"]

["HDMI-A-1"]        # optional per-connector table, named after the connector
optional = true     # a missing connector is a warning, the lease is made without it
planes = [31, 33]   # explicit plane ids; also allows planes shared between CRTCs
```

- Which connectors are leased is the `connectors` array of each `[[lease]]` (`:142-148`); a lease may hold several connectors. A missing mandatory connector makes that lease fail to be created (`lease-manager.c:405-409`); other leases still start, and the daemon exits only if no lease was created (`:632-633`).
- Without `planes`, a lease gets the planes that are possible for its CRTC only (exclusive planes); with `planes`, shared ones are allowed (`:232-265`). Plane ids are kernel ids and differ per board and driver.
- The CRTC for a connector is the active CRTC of its encoder if any, else the first free CRTC among its encoders (`:127-183`). A connector with no CRTC available cannot be leased (`:432-436`).
- A `[[lease]]` without `connectors` parses but holds no objects (`nconnectors` and `ncids` both 0). A parse error silently falls back to the default configuration (`lease-config.c:103-120`, `main.c:101-104`); `doc/configuration-guide.md` says to check the startup log.
- The daemon needs libdrm `>= 2.4.89` and tomlc99 (`libtoml`) (`meson.build`).
- Per lease it creates `<dir>/<name>` and `<dir>/<name>.lock`; a second daemon, or a duplicate lease name, fails on `flock(LOCK_EX|LOCK_NB)` (`lease-server.c:139-171`). The socket is created with the daemon's umask, and no `chmod` or credential check appears in `lease-server.c`. *Inferred:* access control is the permission of the runtime directory (`EACCES` in the client table).

## 5. Client exit and lease revoke (read from source, 2026-11-28)

Server loop, `main.c:126-173`; lease functions, `lease-manager.c:707-779`.

- **Grant** (`DLM_GET_LEASE`): `drmModeCreateLease` on the daemon's own DRM master fd with the lease's object ids. If the lease is already granted, the grant fails; with `-t` the daemon revokes and grants anew (`lm_lease_transfer`), without `-t` it logs "Can't fulfill lease request" and **closes the new client's connection**, so that client gets `EACCES`. On a transfer the previous client is disconnected (`main.c:132-148`). The old client's fd is closed only after the new client has put a new framebuffer on the CRTC (`wait_for_fb_update`, `lease-manager.c:267-335`), which keeps the last frame on screen.
- **Release** (`DLM_RELEASE_LEASE`): the daemon disconnects the client, calls `drmModeRevokeLease` and closes its copy of the lease fd (`main.c:159-166`).
- **Client exit without release** (crash or `kill`): the socket hangs up and the server treats it as `LS_REQ_CLIENT_DISCONNECT` (`lease-server.c:332-333`): the same revoke. The daemon closes its fd as well unless `-k`; with `-k` the lease fd stays open and the lease is revoked but not closed (`main.c:165`).
- **Revoke effect on the client:** `drmModeRevokeLease` invalidates the client's lease fd; the README says DRM calls on it fail with `-ENOENT` afterwards, and that a client replaced by a transfer must handle this. The library gives the client **no notification**: the client learns of a revoke only from a failing DRM call. *Inferred:* the harness's `DeviceLost` path is the right shape for this.
- **Daemon exit:** `lm_destroy` revokes and closes every lease (`lease-manager.c:675-696`) and `ls_destroy` unlinks the sockets (`lease-server.c:242-256`). `main` returns `EXIT_FAILURE` if the request loop ends (`main.c:178`). A daemon killed without that path leaves its socket file and the kernel drops the lease with the master fd; this was not read in detail and is *not measured*.
- Client slots: at most two active connections per lease, one owner and one requester; others wait in the `listen` backlog, which is 0 (`lease-server.c:37-47`, `:204`). *Inferred:* a third simultaneous client may see `connect` fail or block; not tested.
- Revoking is for the lease only: the daemon keeps DRM master on the real device, so a compositor on another connector of the same device needs the daemon not to lease that connector. Whether a compositor can run alongside is the WP5-07 question (`docs/observed/bcm2711-cm4-*`), not answered here.

## 6. How this repository uses it

- `tools/drm-harness/src/wire.rs` is the harness's own re-implementation of this client protocol in Rust (not a binding to `libdlmclient`). Its header comment cites this commit and the same source lines: opcodes and the 4-byte native-endian request (`DlmRequest`), the 1-byte reply with one `SCM_RIGHTS` fd (`dlm_request_lease`, `dlm_send_lease`), a closed connection as `PermissionDenied` (the library's `EACCES`), and `dlm_socket_path` for `<dir>/<name>` with `$DLM_RUNTIME_PATH` then `/var/run/drm-lease-manager`. `DlmLease::get` connects and requests; its `Drop` releases the lease.
- `tools/drm-harness/src/source.rs` (`Source::Dlm`, `DlmLease::get`) gives the engine only the received fd; `tools/drm-harness/src/main.rs` parses `--source dlm --lease <NAME> [--dlm-dir <DIR>]`.
- `tools/drm-harness/src/lessor.rs` (`drm-harness lessor`) serves the same protocol from this repository, so `--source dlm` runs without the daemon. It refuses a second client by closing the connection, as the daemon does, and revokes on release or disconnect.
- `tools/drm-harness/README.md` ("Sources") documents the options. The `revoke` scenario applies to `--source kernel-lease` only, because with `--source dlm` the daemon is the lessor.
- *Gap (read from source, 2026-11-28):* the harness does not link `libdlmclient`, so a run of the harness against the real daemon tests the wire reading above, not the library. `dlm-client-test` from `docs/dlm-aarch64.md` is the program that uses the library itself. Neither has been run against the daemon on a board in this document's scope.
- Build and the daemon command line for aarch64: `docs/dlm-aarch64.md`, `scripts/build-dlm-aarch64.sh`.
