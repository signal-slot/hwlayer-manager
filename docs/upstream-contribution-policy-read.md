# Upstream contribution policy: Weston, libliftoff, AGL (WP5-05, W5-07)

Desk note for `docs/wp5-gap-analysis.md` W5-07 and open issue WP5-05. Written
2026-11-28. Nothing here was run, built or measured, and no board was touched.
This is not legal advice; it repeats what the sources say and marks the rest.

## Limit of this read: read this first

The task was to read the policy files in fresh shallow clones. In this session
(2026-11-28) that was **not possible**: `git clone`, `curl` and a page fetch
were each refused or blocked (the freedesktop GitLab host answered the fetch
tool with an anti-bot page, not the files). Only `git ls-remote` ran.

So this note has two kinds of statement, and they are kept apart:

- **Pin check (observed 2026-11-28):** `git ls-remote` of the remotes, below.
- **Second-hand records:** statements that an earlier item of this repository
  recorded after reading the upstream files. They are quoted from
  `docs/wp5-gap-analysis.md` and `docs/third-party-licences.md`, tagged with
  the date and commit those documents give. **They were not re-read for this
  note**, and the policy files themselves (Weston's `CONTRIBUTING.md`,
  `LLM-USAGE.md`, `AGENTS.md`; libliftoff's `README.md`) were not opened in
  this session. A reader who needs the exact wording must open them at the
  pinned commit.

Everything not recorded anywhere is marked `not found in source (2026-11-28)`
meaning only: not found in the material available to this item. It does not
mean the upstream has no such policy.

## Pins

| Project | URL | Commit | Basis |
| --- | --- | --- | --- |
| Weston | `https://gitlab.freedesktop.org/wayland/weston.git` | `c8d00e2a1dbc98f4a8fb6473a4b485ffce409f3a` (`main`, the commit the earlier read used; `git ls-remote HEAD` still returned it on 2026-11-28) | observed 2026-11-28 (`git ls-remote`); the files at that commit were read by `docs/wp5-gap-analysis.md`, not here |
| Weston release | same URL, tag `16.0.2` (tag object `41d888493b03573ee455d3079bef57d8a1502f29`; target commit `530ae6c769d2fd55ea5157542654810be81a3193` per `docs/weston-release-read.md`) | tag `16.0.2` | observed 2026-11-28 (`git ls-remote`, tag object); target commit taken from `docs/weston-release-read.md`. No policy file was read at the tag |
| libliftoff | `https://gitlab.freedesktop.org/emersion/libliftoff.git` | `c4226a79b7a52f59bb51789b1eda112658609125` (`HEAD`) | observed 2026-11-28 (`git ls-remote`); files read by `docs/wp5-gap-analysis.md` at this commit |
| drm-lease-manager (AGL Gerrit) | `https://gerrit.automotivelinux.org/gerrit/src/drm-lease-manager` | `68c48758c46d93c8e83abb2fb1ec168fa13d1a53` | from `docs/dlm-aarch64.md`; not re-checked |
| meta-agl, layer `meta-agl-drm-lease` | AGL Gerrit `AGL/meta-agl`, tag `unagi_20.92.0` (`8611917335f6d1f5b29ea495c8f4c2c914183e8a`) | tag `unagi_20.92.0` | from `docs/weston-imx-lease.md` row S7; not re-checked |

## Weston

| Statement | Basis |
| --- | --- |
| Weston requires a DCO sign-off from a human | read from source (2026-11-28), at `c8d00e2`: `LLM-USAGE.md`, by `docs/wp5-gap-analysis.md` §2 and §5. Second-hand here, not re-read |
| AI agents must not create or edit issues or merge requests, nor post comments | read from source (2026-11-28), at `c8d00e2`: `LLM-USAGE.md` and `AGENTS.md`, by `docs/wp5-gap-analysis.md` §2. Second-hand here, not re-read |
| AI-written prose is not to be forwarded upstream unless it is concise and reviewed by a human | read from source (2026-11-28), at `c8d00e2`: `LLM-USAGE.md`, by `docs/wp5-gap-analysis.md` §2. Second-hand here, not re-read |
| AI-generated code is the responsibility of the human who submits it | read from source (2026-11-28), at `c8d00e2`: `LLM-USAGE.md`, by `docs/wp5-gap-analysis.md` §2. Second-hand here, not re-read |
| Licence: MIT, X.org wording ("the version of the MIT "Expat" License used by X.org") | read from source (2026-11-28), at `c8d00e2`: `COPYING`, by `docs/third-party-licences.md` §3.5. Second-hand here, not re-read. Individual file headers were not read |
| `CONTRIBUTING.md` of Weston: its sign-off wording and its exact DCO text | not found in source (2026-11-28): the earlier read lists `LLM-USAGE.md` and `AGENTS.md`, not `CONTRIBUTING.md`; this session could not open it |
| A freedesktop.org-wide policy on AI or LLM contributions | not found in source (2026-11-28): not read, not recorded anywhere in the repository |
| Whether the DCO text itself lists a version (1.1) or a trailer form (`Signed-off-by:`) | not found in source (2026-11-28) |

## libliftoff

| Statement | Basis |
| --- | --- |
| Licence: MIT, `LICENSE` "MIT License", "Copyright (c) 2019 Simon Ser"; the README says "MIT" | read from source (2026-11-28), at `c4226a7`: `LICENSE`, `README.md`, by `docs/wp5-gap-analysis.md` and `docs/third-party-licences.md` §3.4. Second-hand here, not re-read |
| It follows Weston's contributing guidelines, so a DCO sign-off is expected | read from source (2026-11-28), at `c4226a7`: `README.md`, by `docs/wp5-gap-analysis.md` §5. The "so the same is expected" step is that document's inference. Second-hand here, not re-read |
| A `CONTRIBUTING.md` in the libliftoff tree, or any rule of its own on DCO, CLA or AI-generated changes | not found in source (2026-11-28): none recorded; this session could not list the tree |

## AGL (Gerrit, `meta-agl-drm-lease`, `drm-lease-manager`)

| Statement | Basis |
| --- | --- |
| `meta-agl-drm-lease` carries `weston_13.0.%.bbappend` with three patches (`0001-backend-drm-Add-method-to-import-DRM-fd.patch`, `0002-Add-DRM-lease-support.patch`, `0001-compositor-do-not-request-repaint-in-output_enable.patch`), enabled by the `agl-drm-lease` feature | read from source (2026-11-28), tag `unagi_20.92.0`, by `docs/weston-imx-lease.md` S7. Second-hand here, not re-read |
| Licence of `drm-lease-manager` and `libdlmclient` | see `docs/third-party-licences.md` §3.1 and the wp5-licences item; not repeated here, so nothing is claimed |
| AGL's rules for contributing to Gerrit (DCO, CLA, `Signed-off-by`) | not found in source (2026-11-28): none recorded anywhere in the repository, and the AGL pages could not be fetched |
| Licence of the Weston patches in `meta-agl-drm-lease` | not found in source (2026-11-28) |

## What this means for an AI-assisted contribution from this repository

All of this section is **inferred** from the second-hand records above, and is
not legal advice.

- A human must sign off. Weston asks for a DCO sign-off by a person, so an
  AI-assisted change can only go upstream if a named human has read it,
  accepts responsibility for it and adds the sign-off themselves. The agent
  does not sign off, and no `Signed-off-by` line is added on its behalf.
  (inferred)
- Limits that follow, if the recorded `LLM-USAGE.md` and `AGENTS.md` points hold
  at the commit the human submits against (inferred):
  - An agent does not open, edit or comment on upstream issues or merge
    requests. The human does, in their own words.
  - Explanatory text written by an agent goes upstream only if short and
    reviewed by that human.
  - The human answers for every line, so they must be able to explain it.
  - Nothing in `docs/wp5-proposal.md` can promise acceptance.
- libliftoff is expected to follow the same rules (inferred from one README
  line); its maintainers' own AI policy is not found in source.
- For AGL Gerrit the rules are not found in source, so no statement is made
  about whether a CLA or DCO applies, or about AI-authored changes.
- Licence direction: both upstreams are MIT, this repository is Apache-2.0
  (`docs/wp5-gap-analysis.md` §5). An offer would have to be made under the
  upstream's licence by the copyright holders; the copyright-holder question
  is open (`docs/wp5-gap-analysis.md` W5-07). Decision for the owner.
- This note must be redone from the primary files before the proposal quotes
  any policy wording.

## Follow-up for whoever can fetch

At the pins above: open Weston `CONTRIBUTING.md`, `LLM-USAGE.md`, `AGENTS.md`,
`COPYING`; libliftoff `README.md`; the AGL Gerrit contribution pages. Replace
each second-hand row with a first-hand quote and a date.
