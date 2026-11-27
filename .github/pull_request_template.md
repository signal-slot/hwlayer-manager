<!-- Short checklist; the rules are in CONTRIBUTING.md and docs/ci.md. -->

## Summary

<!-- What changes and why. The design goes in the commit body. -->

## Checklist

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the rules behind each item.

- [ ] CI checks pass locally: `cargo fmt --all --check`, `cargo clippy`, `cargo test` (workspace), and both `thumbv7em-none-eabihf` builds (`composition-core`, `composition-ffi-staticlib`)
- [ ] Commit subjects are a component prefix (`engine: ...`, `ffi: ...`, `drm: ...`) or a plain sentence, with the design in the body
- [ ] `#![forbid(unsafe_code)]` in every crate except `composition-ffi`; `composition-core` still has no dependencies
- [ ] `crates/composition-ffi/include/composition_engine.h` changes in the same commit as `crates/composition-ffi/src/lib.rs` (or the C ABI is unchanged)
- [ ] Hardware or timing claims are dated measurements on the named hardware, or marked *not measured*; tags (*observed* / *read from source* / *inferred*) are honest
- [ ] No board addresses, host names, serial paths or credentials in files or commit messages
- [ ] Board tests are Zephyr apps or small programs kept in this repository
