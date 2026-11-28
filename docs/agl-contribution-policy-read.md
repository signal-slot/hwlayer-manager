# AGL contribution policy: what was read (WP5 lease route)

**Status: incomplete read.** Written 2026-11-28. Only the generic Gerrit documentation served by AGL's Gerrit host could be fetched. The `meta-agl` clone, the AGL docs site and the AGL wiki could not be read in this session, because the network commands for them (git clone, curl, the docs-site fetch) were refused for lack of permission. So the findings that AGL's own rules would answer are marked **not read**, not guessed. Nothing here promises that AGL, `meta-agl-drm-lease` or any maintainer would accept a contribution.

Scope: complements `docs/upstream-contribution-policy-read.md` (Weston/freedesktop, libliftoff). Tags used: **read from source (2026-11-28)**, **not read**, **no rule found (in the pages named)**.

## 1. Sources and how each was read

| Source | Result | Date |
| --- | --- | --- |
| `https://gerrit.automotivelinux.org/gerrit/Documentation/` (index; Gerrit v3.7.2 as reported by the page) | read, HTTP 200 | read from source (2026-11-28) |
| `https://gerrit.automotivelinux.org/gerrit/Documentation/user-signedoffby.html` | read, HTTP 200 | read from source (2026-11-28) |
| `https://gerrit.automotivelinux.org/gerrit/Documentation/config-cla.html` | read, HTTP 200 | read from source (2026-11-28) |
| `https://gerrit.automotivelinux.org/gerrit/plugins/gitiles/AGL/meta-agl/+/refs/heads/master/README.md` and the tree listing and `CONTRIBUTING.md` at that path | HTTP 404 (the gitiles plugin path is not served there) | 2026-11-28 |
| `meta-agl` clone at a pinned commit | **not read**: `git clone` was refused (permission). No commit is pinned. | 2026-11-28 |
| `https://docs.automotivelinux.org/` contributing pages | **not read**: fetch refused (permission) | 2026-11-28 |
| `https://wiki.automotivelinux.org/` | **not read**: not attempted after the docs site was blocked | 2026-11-28 |
| `Documentation/user-upload.html`, `concept-refs-for-namespace.html` | listed in the index, **not read** | 2026-11-28 |

## 2. Findings

| Question | Finding | Tag |
| --- | --- | --- |
| How is a change submitted? | **Not read for AGL.** The Gerrit index links "Uploading Changes" and "The refs/for Namespace", i.e. the host runs ordinary Gerrit review, but the AGL-specific steps (account, SSH key, `git review` or `git push ... refs/for/master`, branch names, reviewer expectations) were not read. | read from source (2026-11-28) for "Gerrit is the review system"; the rest **not read** |
| Sign-off / DCO | The host's Gerrit page `user-signedoffby.html` carries Developer's Certificate of Origin 1.1, points (a) to (d), copied from the Linux kernel's `SubmittingPatches`, and asks for `Signed-off-by:` with the real name ("no pseudonyms or anonymous contributions"). It is generic Gerrit text. It does **not** say that AGL requires it or enforces it. | read from source (2026-11-28); AGL's own requirement **not read** |
| CLA | `config-cla.html` is the generic Gerrit page ("Users can be required to sign one or more contributor agreements..."). Nothing read shows whether the AGL server has a CLA configured for `meta-agl`. | read from source (2026-11-28); AGL's setting **not read** |
| Licence expected of layer contributions | **Not read.** No `COPYING`/`LICENSE`/`MIT` file of `meta-agl` was read, so this document states no licence. (`docs/third-party-licences.md` and `docs/wp5-licence-options.md` hold this project's side.) | **not read** |
| Rule on AI-generated contributions | **No rule found** in the three Gerrit pages read (none mentions AI). The README, CONTRIBUTING, docs site and wiki, where such a rule would more likely be, were not read, so this is absence in a narrow sample, not proof that none exists. | no rule found (in the pages named), read from source (2026-11-28) |

## 3. What this means for the proposal

- The proposal may say only that AGL uses Gerrit and that the Gerrit host publishes DCO 1.1 text. It must not say that AGL requires DCO sign-off, a CLA, or a given licence, and must not say AGL has no AI rule.
- Until the DCO text is confirmed for AGL, treat it like Weston's: a human has to make the sign-off and carry its meaning; an AI tool cannot give it. This is **inferred** from the DCO wording (the certification is made by "I"), not read from an AGL rule.
- Acceptance is not promised and not predicted.

## 4. To finish (for whoever has network permission)

1. Clone `https://gerrit.automotivelinux.org/gerrit/AGL/meta-agl` into the dependency cache, record the commit hash, and read `README*`, `CONTRIBUTING*`, `COPYING`/`LICENSE*`, and any `meta-agl-drm-lease` layer licence files.
2. Read the AGL docs-site contributing pages and the wiki pages on Gerrit, sign-off and CLA; record each URL with its date.
3. Read `user-upload.html` for the submission steps on this host.
4. Search those pages for "AI", "LLM", "generated", "Copilot" and record "no rule found" or the quoted rule.
5. Replace the **not read** cells above and drop the incomplete-read status line.
