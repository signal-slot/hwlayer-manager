# AGL drm-lease-manager for aarch64 (i.MX95 Linux)

Status (2026-11-28): the source at the pinned commit was read (this document's file:line references are to it). The build script `scripts/build-dlm-aarch64.sh` exists, but **no binary has been produced**: the development machine has no aarch64 cross toolchain, so the fetch and build steps have never run. Nothing here is observed on a board.

## Pinned source

Every source is pinned by a **full commit hash only**. Tag and branch names in this table are for reference and are never used to check anything out.

| Item | Commit | Reference / basis |
| --- | --- | --- |
| drm-lease-manager, `https://gerrit.automotivelinux.org/gerrit/src/drm-lease-manager` | `68c48758c46d93c8e83abb2fb1ec168fa13d1a53` | `master` when read on 2026-11-28 (`git ls-remote`). Version `0.1.0` (`meson.build:4`) |
| libdrm, `https://gitlab.freedesktop.org/mesa/libdrm.git` | `25dec5b91fe4d2638787d033a0b22b6c1dc145e0` | the commit that tag `libdrm-2.4.123` points to (tag object `55c2ca2fa5b6e8dbe1a7aa25135919c8c8a1c023`; commit "build: bump version to 2.4.123"), read 2026-11-28 from the GitLab tags API of project mesa/libdrm. The daemon needs libdrm `>= 2.4.89` (`meson.build:31`) |
| tomlc99 (libtoml), `https://github.com/cktan/tomlc99` | `62ce93ceb713dc49909d0fef1411f9d37bef83f2` | `master` when read on 2026-11-28. Needed because `meson.build:33` has `dependency('libtoml')` and the tree has no wrap for it (`subprojects/` holds only `fff.wrap`). The header at that commit has the `toml_int_at`/`toml_string_in`/`toml_datum_t.u` API that `lease-config.c` uses |

How the script fetches (in `fetch()`):

1. Each pin is checked to be 40 lowercase hex digits before anything else runs. A tag or branch name is rejected.
2. If the commit object is already in the checkout under the deps directory, there is no network access.
3. Otherwise it runs `git fetch --depth 1 origin <hash>`. GitHub, GitLab and AGL gerrit serve a fetch by hash; the drm-lease-manager source was read this way. If a server refuses, the script fetches all branches and tags with explicit destination refspecs (`+refs/heads/*:refs/remotes/origin/*`, `+refs/tags/*:refs/tags/*`, unshallowing first), so every object reachable from them is local.
4. It checks that the commit object exists (`git cat-file -e <hash>^{commit}`), refuses a checkout with local changes, checks out `<hash>` detached, and compares `HEAD` with the pin.

No ref name is ever resolved, so the fetched refs cannot change what gets built.

Override a pin with `DLM_COMMIT`, `LIBDRM_COMMIT` or `TOML_COMMIT` (full hashes), and a URL with `DLM_URL`, `LIBDRM_URL` or `TOML_URL` (e.g. a mirror). Sources go into `$CONNECTED_DEPS_DIR`, or the directory given as the argument, as `drm-lease-manager/`, `libdrm/` and `tomlc99/`. The script refuses `/tmp` and `/var/tmp`. Sources and binaries are not committed.

Options of the tree (`meson_options.txt`): `enable-docs` (default true), `enable-tests` (false, needs `check` and `fff`), `enable-systemd` (false), `runtime_subdir` (`run/drm-lease-manager`). The script turns docs, tests and systemd off, so neither Doxygen nor `check` is needed.

## Build

```sh
scripts/build-dlm-aarch64.sh [DEPS_DIR]
```

Host tools: `git`, `meson`, `ninja`, `pkg-config`, `file`. The script picks the first cross toolchain it finds and prints it (`cross toolchain: ...`):

1. `DLM_CC`: a full compiler command, e.g. `zig cc -target aarch64-linux-musl`. Set `DLM_AR`/`DLM_STRIP` with it.
2. `aarch64-linux-gnu-gcc`: glibc, `-static`.
3. `aarch64-linux-musl-gcc`: musl, `-static`.
4. `zig cc -target aarch64-linux-musl`: musl, `-static`. zig bundles musl and its headers, so it needs nothing else.
5. `clang --target=aarch64-linux-gnu --sysroot=$DLM_SYSROOT -fuse-ld=lld`: needs `ld.lld` and a sysroot that has `usr/include`.

If none is found, it prints what is missing and exits with status 2 **before** fetching or creating anything.

The build steps:

- libdrm: static, GPU drivers, tests and man pages off. The `-D` option names were not checked against `meson_options.txt` at the libdrm pin.
- tomlc99: compiled by hand into `libtoml.a`, plus a `libtoml.pc`.
- drm-lease-manager and `libdlmclient`: built with `-static`.

Everything goes into `$BUILD_DIR/out`. `$BUILD_DIR` defaults to `$TMPDIR/dlm-aarch64-build`, else `<repo>/target/dlm-aarch64-build`. A cross file restricts pkg-config to that prefix. At the end the script prints `file out/bin/drm-lease-manager`, fails unless it is an aarch64 executable, and warns if it is not statically linked. A good result reads `ELF 64-bit LSB executable, ARM aarch64 ... statically linked`. The musl routes give a binary that does not depend on the board's glibc.

The default meson `localstatedir` is relative to the prefix, so the compiled-in socket directory is a build-host path, not a board path. Always set `DLM_RUNTIME_PATH` at run time (below).

### Outcome on the development machine (2026-11-28)

No cross toolchain was usable, and none could be fetched:

- `aarch64-linux-gnu-gcc`, `aarch64-linux-musl-gcc`, `zig` and `ld.lld` are not installed.
- `clang` 23 is present, but there is no aarch64 sysroot (no headers, `crt1.o` or `libc.a`), and `DLM_SYSROOT` is unset.
- The rustup targets `aarch64-unknown-linux-gnu`/`-musl` ship no C headers.
- Downloading zig and running git over the network were not permitted in that session.
- `git`, `meson`, `ninja`, `pkg-config` and `file` are present.

`scripts/build-dlm-aarch64.sh` was run there: it printed the message above and exited with status 2, having created and fetched nothing.

The fetch step was tested on its own against local `file://` repositories. Each test repository had an annotated tag on a commit that is not the branch tip, like libdrm's tag. The URL and pin variables pointed at those repositories, and a stub `DLM_CC` stopped the script once the fetch was done. Results:

- A fresh fetch by hash checked out each pinned commit.
- A tag name as the pin was rejected before anything was created.
- A deps directory under `/tmp` was rejected.
- With protocol v0 and `uploadpack.allowReachableSHA1InWant=false` (fetch by hash refused), the fallback fetched all branches and tags and checked out the pinned commit.
- A rerun with broken URLs used the local commits without network access.
- An unknown hash failed with `commit ... is not available`.
- A checkout with local changes was refused.

Not covered: the fallback into an existing shallow checkout, and the fetch from the real servers (git had no network access in that session). The build step is untested. Expect small fixes on the first run on a machine that has `zig` or an aarch64 gcc, especially in the libdrm `-D` options and in drm-lease-manager's install layout. To unblock, put `zig` on `PATH`, or set `DLM_CC` / `DLM_SYSROOT`.

## Run on the board, from /tmp

Nothing is installed into the image. Copy only the binary and a config, then run as root:

```sh
scp drm-lease-manager root@<board>:/tmp/
scp dlm.toml          root@<board>:/tmp/
# on the board:
chmod +x /tmp/drm-lease-manager
mkdir -p /tmp/dlm
DLM_RUNTIME_PATH=/tmp/dlm /tmp/drm-lease-manager --config /tmp/dlm.toml -v -u 1 /dev/dri/card0
```

Config format (`doc/configuration-guide.md` §2–3, `lease-config.c`): an array of `[[lease]]` tables with `name` and `connectors`, plus an optional top-level table named after a connector, with `optional` and `planes`. Plane IDs are kernel object IDs. They are placeholders until WP0 has recorded them:

```toml
[[lease]]
name = "hwlayer"
connectors = ["LVDS-1"]

[LVDS-1]
planes = [<PRIMARY_PLANE_ID>, <OVERLAY_PLANE_ID>]
```

The socket is then `/tmp/dlm/hwlayer` (`<DLM_RUNTIME_PATH>/<lease name>`). A client must use the same `DLM_RUNTIME_PATH`.

Caveats:

- **Option parsing (verified in source).** The short option string is `"vtkhcu:"` (`drm-lease-manager/main.c:46`): `-c` takes **no** argument, and `-u` **requires** one. The long options `--config` and `--enable-universal-plane` are both `required_argument` (`main.c:52-53`).
  - `-c /tmp/dlm.toml` silently drops the path: the next word is taken as the DRM device, and the default `/etc/drm-lease-manager.toml` is used. Write `--config /tmp/dlm.toml` instead.
  - `-u` needs a dummy argument (`-u 1`), and the word after it is not the device. Put the DRM device last.
  - `usage()` also has a stray comma that makes `-h` print the wrong text (`main.c:41-42`).
- Without `-u`, universal planes are off. With it, primary and cursor planes are enumerated too. Whether the listed overlays are accepted on i.MX95 is not tested. On the board's kernel, overlays are shared between CRTCs, and an explicit `planes` list is what permits shared planes (`configuration-guide.md:69-72`).
- On a TOML parse error, a missing `lease` array or a missing `name`, `parse_config()` returns 0. The daemon then silently falls back to one lease per connector, named `card<minor>-<connector>` (`configuration-guide.md:76-90`). Run with `-v` and read the startup log.
- A listed plane ID that does not exist makes lease creation fail (`configuration-guide.md:131-132`).
- Stop the display stack that holds DRM master (e.g. Weston) first, or the daemon cannot become master.
- Only one client per lease: a second `DLM_GET_LEASE` is refused unless `-t` (`--lease-transfer`) is given (`main.c:130-141`). With `-t`, the old client is disconnected (`main.c:143-146`).

## libdlmclient wire protocol

All references are to commit `68c48758c46d`.

**Socket.** `AF_UNIX`, `SOCK_SEQPACKET`: client `libdlmclient/dlmclient.c:51`; server `drm-lease-manager/lease-server.c:191`, non-blocking, `bind` at `:197`, `listen` with backlog 0 at `:204`. There is one socket per lease name.

**Path.** `common/socket-path.c:27-48` writes `"<dir>/<lease name>"` into `sockaddr_un.sun_path` with `snprintf`.

- `<dir>` is `getenv("DLM_RUNTIME_PATH")` if set. Otherwise it is the compiled-in `DLM_DEFAULT_RUNTIME_PATH` (`meson.build:20-21`: `localstatedir` + `run/drm-lease-manager`, i.e. `/var/run/drm-lease-manager` for a system prefix).
- The path must fit in `sun_path` (108 bytes), else `ENAMETOOLONG`. The client connects with `addrlen = sizeof(struct sockaddr_un)` (`dlmclient.c:57-58`).
- The server also holds a lock file named `<socket path>.lock` (`lease-server.c:35,139-165`), and `unlink`s a stale socket before `bind` (`:187`).
- The lease name is the `name` in the TOML, or `card<minor>-<connector>` by default (`configuration-guide.md:76-84`).
- The server does not create the directory: there is no `mkdir` in `lease-server.c`, and the project only installs it (`meson.build:23`). Create it first.

**Request (client → server).** `common/dlm-protocol.h:21-28`:

```c
enum dlm_opcode { DLM_GET_LEASE /*0*/, DLM_RELEASE_LEASE /*1*/ };
struct dlm_client_request { enum dlm_opcode opcode; };
```

- The message is exactly `sizeof(struct dlm_client_request)` = 4 bytes: one native-endian `u32` (a C enum, `int` on aarch64 and x86-64 Linux). `0` means get lease, `1` means release.
- It is sent as one `sendmsg` with a single iovec and no ancillary data (`dlm-protocol.c:49-66`).
- The server reads it with `recvmsg` and rejects any length other than 4 with `EPROTO` (`dlm-protocol.c:24-47`). An unknown opcode is logged and ignored (`lease-server.c:117-135`).

**Reply (server → client).** `send_lease_fd` (`dlm-protocol.c:122-149`) sends one `sendmsg` with:

- **1 byte** of payload, whose content is undefined (the buffer is uninitialised);
- one `SCM_RIGHTS` control message (`cmsg_level = SOL_SOCKET`, `cmsg_type = SCM_RIGHTS`, `cmsg_len = CMSG_LEN(sizeof(int))`) with exactly one fd, the DRM lease fd.

The client side is `receive_lease_fd` (`dlm-protocol.c:68-120`). It calls `recvmsg` with a 1-byte iovec and a `CMSG_SPACE(sizeof(int))` control buffer.

- A zero-length read (the server closed the socket) means refusal and is reported as `EACCES` (`:84-87`).
- A control message carrying any number of fds other than 1 closes them all and gives `EPROTO` (`:101-109`).

There is no status field: **success is a message with one fd, refusal is EOF**. When it cannot satisfy a request, the server disconnects the client (`main.c:135-141`).

**Release.** The client sends `DLM_RELEASE_LEASE` (4 bytes, as above), then closes the lease fd and the socket (`dlmclient.c:140-149`). On its failure path the client also sends a release before closing (`dlmclient.c:130-137`).

- Closing the socket without that message is handled too: the server treats `LS_REQ_CLIENT_DISCONNECT` like a release and revokes the lease (`main.c:159-166`).
- With `-k` (`--keep-on-crash`), a disconnect without an explicit release keeps the lease object open.
- Revoking invalidates the lease fd the client holds (DRM lease revoke), so every ioctl on it fails afterwards.

**A Rust client** therefore does the following:

1. `socket(AF_UNIX, SOCK_SEQPACKET)`.
2. `connect` to `<DLM_RUNTIME_PATH or /var/run/drm-lease-manager>/<name>`, with a full-size `sockaddr_un`.
3. `send` the four bytes `00 00 00 00`.
4. One `recvmsg` with 1 data byte and a control buffer for one `RawFd`; take the fd from the `SCM_RIGHTS` message.
5. Use that fd as a DRM device. It is a lease fd: `drmModeGetResources` shows only the leased objects.
6. On exit, `send` `01 00 00 00` and close both fds.

The client API is `dlm_get_lease(name)`, `dlm_lease_fd()` and `dlm_release_lease()` (`libdlmclient/dlmclient.h`; implemented at `dlmclient.c:108-157`).

A fallback that needs none of this: the kernel lease ioctls (`DRM_IOCTL_MODE_CREATE_LEASE`) from a small leaser, which is also an item of the plan.

## How the source was read

`git fetch --depth 1 https://gerrit.automotivelinux.org/gerrit/src/drm-lease-manager 68c48758c46d93c8e83abb2fb1ec168fa13d1a53`, then `git show FETCH_HEAD:<path>`; tomlc99 the same way from GitHub. That fetch by hash also shows the gerrit server serves the script's primary fetch path. Gitiles and gitweb URLs on the gerrit host return HTTP 404.

Not read: `lease-manager.c` (how planes and CRTCs are put into the lease object), and libdrm's `meson_options.txt` at the pin.
