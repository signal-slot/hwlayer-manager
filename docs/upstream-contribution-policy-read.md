# Upstream contribution policy: Weston, libliftoff, AGL (WP5-05, W5-07)

Desk note for `docs/wp5-gap-analysis.md` W5-07 and open issue WP5-05. Written
and read 2026-11-28. Nothing here was run, built or measured, and no board was
touched. This is not legal advice; it repeats what the sources say and marks the
rest. A statement tagged `read from source (2026-11-28)` was read in a shallow
clone (`git clone --depth 1`) of the pinned commit on that date.

## Pins

| Project | URL | Commit read | Files read |
| --- | --- | --- | --- |
| Weston | `https://gitlab.freedesktop.org/wayland/weston.git`, branch `main` | `c8d00e2a1dbc98f4a8fb6473a4b485ffce409f3a` | `CONTRIBUTING.md` (whole, searched for sign-off, LLM and licence text), `LLM-USAGE.md` (whole), `AGENTS.md` (whole), `DCO-1.1.txt` (title lines), `COPYING` (named only, see `docs/third-party-licences.md` §3.5); `README.md`, `RELEASING.md` and `doc/` searched for LLM/AI terms |
| libliftoff | `https://gitlab.freedesktop.org/emersion/libliftoff.git`, branch `master` | `c4226a79b7a52f59bb51789b1eda112658609125` | `README.md` (searched for contributing, sign-off and licence lines), `LICENSE` (first lines), `doc/` searched for LLM/AI terms |
| AGL docs | `https://docs.automotivelinux.org/` (home page and `/en/master/`) | no commit: web pages, fetched 2026-11-28 | home page text only |

Both clones' `HEAD` equals the commit `docs/wp5-gap-analysis.md` read on the same
date. The Weston release tag `16.0.2` (see `docs/weston-release-read.md`) was
**not** checked for these files; the policy text above is that of `main`.

## Weston

| Statement | Basis |
| --- | --- |
| `CONTRIBUTING.md`: "Your commit messages must also include a Signed-off-by line with your name (or pseudonym) and email address which indicates that you agree to the Developer's Certificate of Origin 1.1" (linking `DCO-1.1.txt`) | read from source (2026-11-28), `c8d00e2`: `CONTRIBUTING.md` |
| "**Agreeing to DCO 1.1 is mandatory.** Patches without a Signed-off-by cannot be accepted, but using a pseudonym is fine as long as the email address is yours personally." If you are not the original author, gather Signed-off-by from them as well | read from source (2026-11-28), `c8d00e2`: `CONTRIBUTING.md` |
| `DCO-1.1.txt` is "Developer Certificate of Origin, Version 1.1" (Linux Foundation text) | read from source (2026-11-28), `c8d00e2`: `DCO-1.1.txt`, title lines only |
| Patches by email are no longer accepted; merge requests on GitLab are the route | read from source (2026-11-28), `c8d00e2`: `CONTRIBUTING.md` ("Do not send patches over email") |
| `CONTRIBUTING.md` has an "LLM-Policy" section pointing to `LLM-USAGE.md`: "Please read it before opening a MR if you want to use any LLM or AI tools" | read from source (2026-11-28), `c8d00e2`: `CONTRIBUTING.md` |
| `LLM-USAGE.md`: AI agents must not use API, CLI or web UI automation to create, edit or close issues, post comments on merge requests, issues or commits, or open or update merge requests | read from source (2026-11-28), `c8d00e2`: `LLM-USAGE.md`, "Upstream interaction rules" |
| `LLM-USAGE.md`: "Prose written by an AI agent must not be forwarded upstream unless it is concise, and fully reviewed and copyedited by a human first" | read from source (2026-11-28), `c8d00e2`: `LLM-USAGE.md` |
| `LLM-USAGE.md`: "AI agents MUST NOT add Signed-off-by tags. Only humans can legally certify the Developer Certificate of Origin." The human submitter reviews all AI-generated material, ensures licensing compliance and adds their own Signed-off-by | read from source (2026-11-28), `c8d00e2`: `LLM-USAGE.md` |
| `LLM-USAGE.md`: the human contributor is "solely responsible" for all code; AI-generated code and comments are treated as human-written; "AI suggested it" is not an acceptable justification; the human must understand and validate logic, side effects and security implications. AI-assisted code goes through the normal peer review; tests should be added or updated | read from source (2026-11-28), `c8d00e2`: `LLM-USAGE.md`, "Responsibility", "Reviewing and validation" |
| `AGENTS.md`: agents never add Signed-off-by; no automation on issues, merge requests or commits; do not write documentation, commit messages, merge-request text or code comments for the human unless asked for a rough draft, since the human writes the final commit message and any comments to keep; the task is not complete until those human-authored parts exist and are reviewed | read from source (2026-11-28), `c8d00e2`: `AGENTS.md` |
| Licence: new files "should specify the MIT Expat license in their boilerplate"; review checks "the code license is the same MIT licence the project generally uses" | read from source (2026-11-28), `c8d00e2`: `CONTRIBUTING.md`, "Licensing" and "Review" |
| A freedesktop.org-wide policy on AI or LLM contributions | not found in source (2026-11-28): none in the Weston tree; the freedesktop.org sites were not read |
| Whether `LLM-USAGE.md` or `AGENTS.md` exist at tag `16.0.2` or in other stable branches | not found in source (2026-11-28): not checked |

## libliftoff

| Statement | Basis |
| --- | --- |
| README "Contributing": "We use the Wayland/Weston style and contribution guidelines, see Weston's contributing document" (links Weston's `CONTRIBUTING.md` on `master`) | read from source (2026-11-28), `c4226a7`: `README.md` |
| Licence: README says "MIT"; `LICENSE` begins "MIT License", "Copyright (c) 2019 Simon Ser" | read from source (2026-11-28), `c4226a7`: `README.md`, `LICENSE` |
| A `CONTRIBUTING.md`, DCO text, CLA or AI policy of libliftoff's own | not found in source (2026-11-28): the tree has no `CONTRIBUTING.md` and a search of `README.md` and `doc/` for LLM/AI terms found nothing |
| That Weston's DCO rule therefore applies to libliftoff | not found in source (2026-11-28) as an explicit statement; it follows from the README line above and is inferred |

## AGL (Gerrit, `meta-agl-drm-lease`, `drm-lease-manager`)

| Statement | Basis |
| --- | --- |
| The AGL docs home page says anyone can join the mailing list and Zoom meetings, or contribute to any project, and names Gerrit "for code collaboration" | read from source (2026-11-28): `https://docs.automotivelinux.org/` home page text (web page, no commit) |
| Signed-off-by, DCO, CLA or licence-of-contribution rules, or an AI policy, for AGL Gerrit | not found in source (2026-11-28): not on the docs home page or `/en/master/`; the wiki and `git.automotivelinux.org` were not readable (the latter returned an anti-bot page) |
| Licence of the `meta-agl-drm-lease` recipes and its Weston patches | not found in source (2026-11-28): the layer could not be read in this session |
| Licence of `drm-lease-manager` and `libdlmclient` | see `docs/third-party-licences.md` §3.1; not repeated here |
| Layer contents (`weston_13.0.%.bbappend` and three patches, tag `unagi_20.92.0`) | recorded in `docs/weston-imx-lease.md` row S7; not re-read here |

## What this means for an AI-assisted contribution from this repository

All of this section is **inferred** from the rows above, and is not legal
advice.

- A human must sign off. Weston makes the DCO sign-off mandatory and says only
  humans can certify it. An AI-assisted change can go to Weston (and, by the
  README line, libliftoff) only if a named person has reviewed it, accepts
  responsibility, and adds their own `Signed-off-by`. Agent-made commits in this
  repository must not carry one on the person's behalf. (inferred)
- Limits that follow:
  - An agent does not open, edit, close or comment on upstream issues or merge
    requests, and does not push a merge request. The human does it in their own
    name. (inferred from `LLM-USAGE.md`)
  - Text an agent drafted (commit message, merge-request text, code comments)
    is, under `AGENTS.md`, not for the agent to finalise: the human writes the
    final commit message and comments. `docs/wp5-proposal.md` and this note are
    internal drafts, not upstream text. (inferred)
  - The human must be able to explain and validate every line; "the AI wrote
    it" is no defence. (inferred)
  - Upstream accepts or rejects through normal review; nothing in this
    repository can promise acceptance. (inferred)
- Licence direction: Weston and libliftoff are MIT; this repository is
  Apache-2.0. Code offered upstream must be under the upstream's licence, which
  the copyright holders must grant; the copyright holder is not decided
  (`docs/wp5-gap-analysis.md` W5-07). Decision for the owner.
- AGL Gerrit: no rule found, so no statement is made about a CLA, a DCO or AI
  authorship there. Treat it as unknown until the contribution pages are read.
- Commits here follow the repository's own rules (`CONTRIBUTING.md`); this note
  does not change them.

## Not done

- AGL wiki and Gerrit contribution pages, `meta-agl` licence files.
- freedesktop.org-wide pages on AI contributions.
- The same files at Weston tag `16.0.2` and in stable branches.
