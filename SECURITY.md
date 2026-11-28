# Security policy

## Reporting a vulnerability

Report suspected vulnerabilities privately, through GitHub's private
vulnerability reporting for `signal-slot/hwlayer-manager`: open the
repository's **Security** tab and choose **Report a vulnerability**. Please do
not open a public issue or pull request for a vulnerability, and do not send
reports to individual people.

A useful report names the crate and function, the input or call sequence that
triggers the problem, what you observed, and the commit you tested. Say whether
the observation was made on a host build, a `thumbv7em` build or a board; do
not claim more than you measured.

This is a small project without a service-level commitment: reports are
looked at as maintainers have time.

## Supported versions

Only `main` is supported. There are no releases yet, so there are no older
versions to patch. A fix lands on `main` through a pull request like any other
change (see `CONTRIBUTING.md`).

## Scope

In scope:

- **`composition-ffi`** (`crates/composition-ffi`): the only crate allowed to
  contain `unsafe` code. Its C ABI is declared in
  `crates/composition-ffi/include/composition_engine.h`.
- **`composition-ffi-staticlib`** (`crates/composition-ffi-staticlib`): the
  static library that packages the C ABI for C programs, built for
  `thumbv7em` among other targets. It is `#![forbid(unsafe_code)]`.
- **The DRM backend** (`crates/composition-drm`) on Linux. It is
  `#![forbid(unsafe_code)]` and drives the file descriptor it is handed
  through atomic modesetting.

Out of scope: vulnerabilities in the Linux kernel, DRM drivers, Weston,
Zephyr, board firmware or third-party crates themselves (report those to their
own projects), and the demo and test programs, which are not hardened.

### The unsafe boundary

`composition-core` has no dependencies and, like `composition-ffi-staticlib`
and `composition-drm`, is `#![forbid(unsafe_code)]`. The workspace lint level
for `unsafe_code` is `deny`; `composition-ffi` opts out with
`#![allow(unsafe_code)]`. What that crate does, as written in
`crates/composition-ffi/src/lib.rs`:

- The C caller supplies the engine's storage (`ce_engine_init`) and a backend
  as a table of function pointers (`ce_backend_ops`) plus a context pointer.
  The engine does not allocate it. `ce_engine_init` returns null if `storage`
  or `ops` is null, if `size` is smaller than the engine, or if `storage` is
  misaligned. It does not check that the pointers are otherwise valid.
- Every `extern "C"` function except `ce_abi_check` and `ce_abi_check_advise`
  is `unsafe` and documents a `# Safety` contract. Null `engine`, `info` or
  `frame` pointers are checked and reported as `CE_ERR_INVALID_ARGUMENT` (or
  a null/zero result for the query functions). Beyond null, the library
  cannot verify pointers: an `engine` that did not come from
  `ce_engine_init`, a dangling or too-short `layers` array in `ce_present` or
  `ce_advise`, or a function table whose
  functions are unsafe to call with the given context is undefined behaviour,
  and it is the caller's responsibility.
- `ce_present` copies the caller's layers into a fixed-size local array
  (`layer_count` above the engine limit is rejected with
  `CE_ERR_TOO_MANY_LAYERS`) and validates the `blend` and `transform` values
  before planning. `ce_advise` does the same with its frame, and also
  checks its `options`, `caps` and `advice` pointers and its buffer table
  (null with a non-zero count is `CE_ERR_INVALID_ARGUMENT`). Unknown event
  kinds from a C backend's `poll_event` are skipped.
- Calls into the C backend (`capabilities`, `check`, `commit`, `poll_event`)
  are made with pointers to live locals or to the engine's own storage; the
  backend's functions are trusted to honour their contract.
- `ce_abi_check` (and `ce_abi_check_advise` for the `ce_advise`
  structures) lets a C program compare its structure sizes with the
  library's. The header is written by hand, so a mismatch between header and
  library is possible if they are changed separately; that is why they change
  in the same commit. The check covers sizes only, not field layout or
  offsets.
- The engine is not thread safe (header: call it from one context, or lock).
  Nothing in the code enforces this.
- The code contains no `catch_unwind`. Panic behaviour depends on the build:
  with the `std` feature a panic crossing the C ABI is not handled by this
  crate, and without it the final program supplies the panic handler
  (`composition-ffi-staticlib` offers a halting one under its
  `panic-handler` feature). Do not rely on a panic being turned into an error
  code.

No claim is made that the unsafe code has been formally verified or audited.
Reports that find a path where valid inputs, as documented in the `# Safety`
sections, reach undefined behaviour are the most valuable.

## Credentials and board details

Board credentials, passwords, keys, IP addresses, host names, user names and
serial-device paths never belong in issues, pull requests, files or commit
messages, including in logs and screenshots pasted into them. Remove or
redact them before posting. If one has been committed or posted, report it
privately as above so it can be rotated and removed.
