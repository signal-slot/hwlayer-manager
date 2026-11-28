# WP5: Apache-2.0 or MIT for offering code upstream

Written 2026-11-28 for SoW §8 WP5 (`docs/open-issues.md` WP5-03 and WP5-05, `docs/wp5-gap-analysis.md` W5-07 and W5-09).

**This is not legal advice.** It sets out the options and their trade-offs. The decision is the owner's; this note recommends none. Anything marked *inferred* is a reading of licence texts by a non-lawyer and should be checked by someone qualified before the owner relies on it.

## 1. What is offered

Offered parts (names as in the repository at `df507ed`, read from source 2026-11-28):

- the advisory planner: `composition_core::advise` (`crates/composition-core/src/advise.rs`) and its C form `ce_advise` (`crates/composition-ffi`), described in `docs/advisory-planner-design.md`;
- `DriverProfile` data: `crates/composition-drm/src/profile.rs`, with `DriverProfile::Imx95Dpu`. W5-09 already notes that a per-driver table has no counterpart in generic KMS, so it may stay project-local (inferred).

This note covers licence mechanics only. What form an offer takes, and whether upstream wants it, is for `docs/wp5-proposal.md` and the upstream maintainers. Nothing here promises acceptance.

## 2. Facts

### 2.1 This repository

| Fact | Basis |
| --- | --- |
| The project is under the Apache License, Version 2.0; `Cargo.toml` `[workspace.package]` has `license = "Apache-2.0"`, and the crates inherit it with `license.workspace = true` | read from source (2026-11-28): `LICENSE`, `Cargo.toml` line 10, `crates/*/Cargo.toml` line 8 |
| `NOTICE` reads "Copyright 2026 The hwlayer-manager contributors" and points to `LICENSE`. It names no legal entity | read from source (2026-11-28): `NOTICE` |
| `CONTRIBUTING.md` says a contribution is accepted under the Apache-2.0 terms. It has no sign-off or DCO text and no copyright-assignment or CLA text | read from source (2026-11-28): `CONTRIBUTING.md`, `docs/wp5-gap-analysis.md` §8 |
| `git log` shows one author name on all 622 commits at `df507ed`. 486 of them carry a `Co-Authored-By` trailer, which in this repository is the AI-assistant attribution line | observed (2026-11-28): `git log` of the workspace clone |
| `LICENSE` names no copyright holder; the placeholder in `NOTICE` is not settled (`docs/wp5-gap-analysis.md` §8, "the copyright holder") | read from source (2026-11-28) |

### 2.2 Upstream targets

| Target | Licence | Basis |
| --- | --- | --- |
| libliftoff, `https://gitlab.freedesktop.org/emersion/libliftoff.git`, commit `c4226a79b7a52f59bb51789b1eda112658609125` (2026-08-24) | MIT; `LICENSE` is "MIT License", "Copyright (c) 2019 Simon Ser", the standard MIT text | read from source (2026-11-28): recorded in `docs/third-party-licences.md` §3.4 and `docs/wp5-gap-analysis.md` §1 from a shallow fetch of that commit |
| Weston, `https://gitlab.freedesktop.org/wayland/weston.git`, commit `c8d00e2a1dbc98f4a8fb6473a4b485ffce409f3a` (2026-11-27) | MIT, X.org wording ("the version of the MIT "Expat" License used by X.org"); copyright lines of Kristian Høgsberg, Intel Corporation, Benjamin Franzke, Collabora, Ltd., Red Hat. Per-file headers were not read | read from source (2026-11-28): recorded in `docs/third-party-licences.md` §3.5 |
| Contribution rules of both | Weston requires a DCO sign-off from a human; AI agents must not create or edit merge requests or post comments. libliftoff follows Weston's guidelines | read from source (2026-11-28): `docs/wp5-gap-analysis.md` §2 and §5, from `LLM-USAGE.md`, `AGENTS.md` (Weston) and `README.md` (libliftoff) |

Limit of this note: the upstream licence files were read for the pinned commits by the earlier WP5 reading on 2026-11-28 and are cited here from those records. This item tried to fetch them again on 2026-11-28 and could not: gitlab.freedesktop.org answered the fetch with an anti-bot block page and the sandbox refused a direct clone. So the quotations above are second-hand (read from source by another document, not re-read by this note), and the per-file headers of both trees stay unread.

## 3. General consequences (inferred)

- MIT code can be included in an Apache-2.0 work with the MIT notice kept. The reverse, putting Apache-2.0 code into an MIT project, needs the copyright holders to offer that code under MIT (or under a dual licence that includes MIT). A maintainer asked to merge a patch under Apache-2.0 into an MIT tree may decline for that reason alone; whether libliftoff or Weston would accept Apache-2.0 code was not asked and is not known (not measured).
- Apache-2.0 §3 grants a patent licence from each contributor, and ends it for a licensee who sues over the work. MIT has no express patent grant. Code relicensed under MIT therefore carries no express patent licence from the owner. Whether an implied licence arises depends on jurisdiction and facts.
- Apache-2.0 §4 asks a redistributor to carry the licence text, a `NOTICE` file's content, change statements and existing notices. MIT asks only that the copyright notice and permission notice stay in all copies or substantial portions. The offered files would carry whichever applies.
- Only the holder of the copyright can relicense. Who that is for this repository is not recorded (§2.1). All commits have one author name, which suggests one human contributor, but the repository does not say whether that person, an employer or a client holds the rights. Whether AI-assisted portions are copyrightable, and by whom, depends on jurisdiction and on the human contribution. This note does not settle it.
- Existing third-party dependencies are not in the offered files. `composition-core` has its own dependency list in `Cargo.toml`; the offered parts must be checked for dependencies before any copy leaves the repository (not measured here).

## 4. Options

Common to all options: the offered part is the advisory planner and `DriverProfile` data, not the whole repository. The rest of the repository stays Apache-2.0 under every option unless the owner decides otherwise.

### Option A: relicense the offered parts under MIT

The files offered upstream are published under MIT only. The owner may keep Apache-2.0 for the same files in this repository, if the owner holds all the rights, because a holder can license the same code to different recipients under different terms (inferred).

What the owner must decide or do:

- **Copyright holders.** Identify every holder of the offered files (§3, fourth bullet) and confirm each agrees. If anyone else contributed, their consent is needed. `CONTRIBUTING.md` has no sign-off or assignment text, so the repository itself gives no grant from other contributors beyond "accepted under Apache-2.0 terms" (inferred).
- **Patent grant.** The express Apache-2.0 §3 patent licence does not apply to copies received under MIT. The owner decides whether that matters. Possible sources of patent risk would be the owner's own or the owner's employer's patents; none is recorded in the repository (not measured).
- **NOTICE.** MIT needs no `NOTICE`. The upstream copy would carry the MIT permission notice with the holder's name in each offered file or in the tree's licence file. A name has to be chosen: "The hwlayer-manager contributors" is a placeholder and is not a legal person. Two licences for the same file need a stated rule in this repository, for example an SPDX line per file and a sentence in `NOTICE`.
- **Contribution flow.** Weston's DCO is a human's sign-off. The human submitter certifies the right to submit under the project's licence (inferred from the DCO's wording; the DCO text itself was not re-read for this note). With Option A that statement is straightforward only if the rights position above is settled.

Trade-offs (inferred): smallest licence mismatch with both targets; loses the express patent grant for the MIT copies; needs the holder question answered first.

### Option B: dual licence, `Apache-2.0 OR MIT`

The offered files are marked `Apache-2.0 OR MIT`, so a recipient chooses. This is the form the Rust ecosystem uses widely: of the crates listed in `docs/third-party-licences.md` §2, `bitflags`, `errno` and others are `MIT OR Apache-2.0` (read from source, 2026-11-28, that document).

What the owner must decide or do:

- **Copyright holders.** The same consent as Option A: the holder must grant both licences.
- **Patent grant.** A recipient who chooses Apache-2.0 keeps the patent licence. A recipient who chooses MIT does not. Upstream would receive the files and could pick MIT for the upstream tree (inferred), so the grant would be lost in practice for that tree; the owner would still keep it for Apache-2.0 users of this repository.
- **NOTICE.** Add the SPDX expression `Apache-2.0 OR MIT` to the offered files, add the MIT text next to `LICENSE` (for example `LICENSE-MIT`), and state in `NOTICE` and `CONTRIBUTING.md` that contributions are accepted under both. Today `CONTRIBUTING.md` says Apache-2.0 only, so contributors who have already contributed gave Apache-2.0 only unless they agree otherwise (inferred). A dual licence for later contributions does not reach back to earlier ones without their holders' consent.
- **Cargo metadata.** `license = "Apache-2.0"` in the workspace `Cargo.toml` would become `"MIT OR Apache-2.0"` if the whole workspace were dual-licensed, or the offered crate would get its own field. Changing it is out of scope here.

Trade-offs (inferred): keeps Apache-2.0 for existing users and still gives upstream an MIT choice; adds a second licence file and a rule for contributors; the MIT-chosen copy has no patent grant.

### Option C: clean reimplementation in C under MIT

The owner or another person writes the planner anew in C, from the design (`docs/advisory-planner-design.md`) and the public behaviour, without copying the Rust source. The C code is then under MIT from the start.

What the owner must decide or do:

- **Copyright holders.** The new C code belongs to whoever writes it. If the author has read the Rust source closely, a maintainer or a court could regard the C code as a derivative work, so "clean" would need a record of what the author used (inferred). The design note and the C ABI header are the repository's own and would be the input. Whether those documents can be used as a specification without carrying over the Apache-2.0 terms depends on the same holder question as in Option A (inferred).
- **Patent grant.** None for the new code under MIT. The Rust code keeps its Apache-2.0 grant in this repository.
- **NOTICE.** The new C code carries its own MIT notice. If any text or data (for example `DriverProfile` values for `imx95`) were copied from this repository, the question returns to Options A or B for those parts. Values taken from a driver's source carry that driver's licence terms in turn; `profile.rs` says it is filled from driver source (`docs/wp5-gap-analysis.md` §1), and the licence of that source was not reviewed for this note (not measured).
- **Cost and drift.** Two implementations to keep in agreement. The repository's tests pin the Rust behaviour; the C version would need its own tests or a cross-check against the Rust one (inferred). The effort is not estimated (not measured).
- **Contribution flow.** The DCO and AI rules apply to the human who submits the C code. If the C code is written with an AI assistant, Weston's rules limit that (`docs/wp5-gap-analysis.md` §2); `docs/wp5-proposal.md` is where that is worked out.

Trade-offs (inferred): the cleanest licence position for upstream, with no change to this repository's licence; the most work; the risk of a "derived from" argument unless the process is documented.

## 5. Side by side (inferred)

| | A: relicense under MIT | B: `Apache-2.0 OR MIT` | C: C rewrite under MIT |
| --- | --- | --- | --- |
| Needs holder consent for the offered files | yes | yes | no for new code; yes if anything is copied |
| Express patent grant for the upstream copy | no | only if the recipient picks Apache-2.0 | no |
| Change to this repository's licence files | an SPDX note or exception | add `LICENSE-MIT`, change `NOTICE`, `CONTRIBUTING.md`, maybe `Cargo.toml` | none |
| Upstream licence match | exact | exact if MIT is chosen | exact |
| New code to write | none | none | the whole planner and its tests in C |

## 6. What the owner decides first

1. Who holds the copyright in the offered files (§3), and whether anyone other than the owner must agree. `NOTICE` needs a real name once that is answered (open since W5-07).
2. Whether the express patent grant matters enough to prefer an option that keeps it for some recipients.
3. Whether AI-assisted authorship of 486 of 622 commits affects any of this, for the owner's jurisdiction and for upstream's AI rules.
4. Which of Options A to C to ask upstream about, and whether to ask before writing anything (`docs/wp5-proposal.md`).

Files this note leaves unchanged (by its contract): `LICENSE`, `NOTICE`, `Cargo.toml`, `docs/third-party-licences.md`, `docs/wp5-proposal.md`.
