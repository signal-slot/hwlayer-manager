# AGL contribution policy: what was read (WP5 lease route)

Written 2026-11-28. Scope: AGL's own rules for contributing a change to `meta-agl` (the layer that contains `meta-agl-drm-lease`). It complements `docs/upstream-contribution-policy-read.md` (Weston/freedesktop, libliftoff). **Nothing here promises, predicts or implies that AGL or any maintainer would accept a contribution.**

Tags: **read from source (2026-11-28)** = read in a clone or on a fetched page on that date; **inferred** = my reading of what the source implies; **not read** = not looked at.

## 1. Sources and how each was read

| Source | How | Date |
| --- | --- | --- |
| `meta-agl`, `git clone --depth 1 https://gerrit.automotivelinux.org/gerrit/AGL/meta-agl`, branch `master`, commit `3cb893411904a7fe1ff27e9c76011c8210f012ce` (cloned into the dependency cache; files `README.md`/`meta-agl.md`, `README-AGL.md`, `LICENSE`, `.gitreview`, `ai-coding.md`, `.gitlab/merge_request_templates/default.md`, `meta-agl-drm-lease/README.md`, `meta-agl-drm-lease/SECURITY.md`, `meta-agl-drm-lease/ai-coding.md` read in full by me) | read directly | 2026-11-28 |
| AGL docs, chapter "How To Contribute", `https://docs.automotivelinux.org/en/master/07_How_To_Contribute/` pages `01_Getting_Linux_Foundation_account`, `03_Working_with_Gerrit`, `04_Submitting_Changes`, `07_General_Guidelines`, `08_AI-coding-assistants`, `08_Code_contribution_guidelines`, `10_Contribution_Checklist` | fetched by a web-fetch helper agent that reported quotes and paraphrase; I did not read the pages myself, so wording below is quoted only where the helper quoted it, and the page text should be re-checked before it is cited upstream | 2026-11-28 |
| Same chapter, pages `02_Using_Jira_for_current_work_items`, `05_Reviewing_Changes`, `06_Gerrit_Recommended_Practices`, `08_Getting_help`, `08_Reporting_bugs`, `09_Adding_Documentation`, `11_Setup_AGL_LAVA_Lab` | **not read** | 2026-11-28 |
| `https://git.automotivelinux.org/AGL/meta-agl/docs/contributing.md` (linked from the AI page) | **not read**; `meta-agl` at the pinned commit has a `docs` entry in its tree listing but no such file was checked | 2026-11-28 |
| AGL wiki (`wiki.automotivelinux.org`) | **not read** | 2026-11-28 |
| Generic Gerrit pages on the AGL host (`/gerrit/Documentation/`, `user-signedoffby.html`, `config-cla.html`; Gerrit v3.7.2) | read; generic Gerrit text, nothing AGL-specific, so not used below | 2026-11-28 |

## 2. Findings

### 2.1 How a change is submitted

- Repo `README-AGL.md` (pinned commit): "Please use https://gerrit.automotivelinux.org for code contributions." and points to the docs chapter "How to contribute". read from source (2026-11-28)
- `.gitreview` at the pinned commit: host `gerrit.automotivelinux.org`, port `29418`, project `AGL/meta-agl`, default branch `master`. read from source (2026-11-28)
- Docs `03_Working_with_Gerrit`: a Linux Foundation ID is needed (page 01), an ssh key is added to the Gerrit account, the repo is cloned over ssh with the `commit-msg` hook, and a change is uploaded with `git review` (`git review -s` once; update with `git commit --amend` and `git review` again). Patches go through Gerrit, not the mailing list; the `agl-dev-community` list and IRC are for discussion. Submitters vote +1/-1, maintainers +2/-2. read from source (2026-11-28), via fetch helper
- Docs `04_Submitting_Changes`: each commit needs a subject (72 characters or fewer), a description, a `Signed-off-by:` line and a `Change-Id:` line ("Gerrit won't accept patches without this identifier"); the sign-off name and email must match the commit authorship. read from source (2026-11-28), via fetch helper
- Docs `07_General_Guidelines`: each commit must include the Jira identifier (title and `Bug-AGL: [SPEC-<id>]` in the body); one change per review; include tests; aim for 200-300 lines, larger changes likely get -2; new features are proposed in a Jira Epic or Story and announced on `agl-dev-community`. The same chapter says 50 characters for the title where page 04 says 72: the pages disagree. read from source (2026-11-28), via fetch helper
- Docs `08_Code_contribution_guidelines` / `10_Contribution_Checklist`: code must build against master; a `meta-*` layer must pass `yocto-check-layer`; "Gerrit commits need two upvotes (not from authors!) to be merged". read from source (2026-11-28), via fetch helper
- The layer's `meta-agl-drm-lease/README.md` names `agl-dev-community@lists.automotivelinux.org` as maintainer and patch address. read from source (2026-11-28)
- Inferred (2026-11-28): a lease-route contribution would need an LF ID, a Jira item, a Gerrit change against `AGL/meta-agl` (or the `src/drm-lease-manager` project, whose Gerrit gitweb URL the layer README names), and two non-author approvals. None of this was exercised.

### 2.2 Sign-off / DCO

- Docs `07_General_Guidelines`, "Legal stuff": a sign-off is required "to ensure that the author of the change adhere to follow DCO", with a link to developercertificate.org and `git commit -s` recommended. `10_Contribution_Checklist`: "Code contributions submitted need to have a Sign-off-by! (Follow DCO.)" read from source (2026-11-28), via fetch helper
- Docs `08_AI-coding-assistants`: "AI agents MUST NOT add Signed-off-by tags." Only humans can certify the DCO; the human submitter must review all AI-generated code, ensure licensing compliance, add their own `Signed-off-by` and take full responsibility. read from source (2026-11-28), via fetch helper

### 2.3 CLA

- No CLA appears on any page read (chapter pages listed in section 1 as read, and the repo files). Pages 02, 05, 06, 08_Getting_help, 08_Reporting_bugs, 09, 11 and the wiki were not read, so this is "no CLA found in the pages read", not a statement that none exists. read from source (2026-11-28)

### 2.4 Licence expected of layer contributions

- `README-AGL.md` / `LICENSE` at the pinned commit: components are under a mix of MIT and GPLv2; "All metadata (e.g. .bb, .bbappend, .bbclass, .inc, templates/* and similar) is MIT licensed unless otherwise stated." Patches carried in-tree are under the licence stated in the recipe. Other files (scripts) default to GPL-2.0-only unless stated. Files may carry `SPDX-License-Identifier: MIT` or `GPL-2.0-only`. read from source (2026-11-28)
- Docs `08_AI-coding-assistants` lists Apache-v2 for "Source Code developed by AGL as a project" and MIT for Yocto recipes/layers. `10_Contribution_Checklist` asks for an OSI-approved licence, a `LICENSE`/`COPYING` file and licence headers or SPDX tags. read from source (2026-11-28), via fetch helper
- Consequence (inferred, 2026-11-28): recipes or metadata offered to `meta-agl-drm-lease` would be MIT; separate AGL-developed source under Apache-2.0 per the AI page. The licence of drm-lease-manager itself is not covered here (`docs/third-party-licences.md`). Which licence this project's code would be offered under is an owner decision (`docs/wp5-licence-options.md`).

### 2.5 AI-generated contributions

A rule exists, and it is in AGL's own docs; it is not "no rule found".

- The repository carries `ai-coding.md` in the root and in each sub-layer including `meta-agl-drm-lease/` (identical three-line files). Content, read directly: "Please carefully review the respective reference documentation here:" followed by the docs URL `https://docs.automotivelinux.org/en/master/#07_How_To_Contribute/08_AI-coding-assistants/`. read from source (2026-11-28)
- Docs page `08_AI-coding-assistants` ("Coding Assistants"), as reported by the fetch helper: AI tools must follow the standard AGL process; AI agents must not add `Signed-off-by`; the human certifies the DCO; contributions carry an `Assisted-by: AGENT_NAME:MODEL_VERSION [TOOL1] [TOOL2]` tag (specialised analysis tools only, not git/gcc/make/editors); the human is solely responsible and "The AI suggested it" is not accepted as justification; contributors must ensure AI output does not copy verbatim blocks from GPL-only or proprietary sources and does not misapply licence headers in `.bb`/`.bbappend` files; AI-assisted code goes through normal peer review. read from source (2026-11-28), via fetch helper
- Inferred (2026-11-28): a lease-route change drafted with an AI tool needs a human submitter with their own LF ID, their own `Signed-off-by`, an `Assisted-by:` line naming the tool and model, and the commit must not carry a sign-off from the tool. Whether this project's own commit trailers (`Co-Authored-By`) are acceptable to AGL is not stated by the source; the documented AGL tag is `Assisted-by`.

## 3. What this means for the proposal

- It may state: AGL uses Gerrit with `git review` and an LF account; DCO sign-off is required; an AI-assistant policy exists, forbids tool-added sign-offs and asks for `Assisted-by`; layer metadata is MIT.
- It must not state: that a CLA does not exist (not found in pages read, others unread), that a specific change would be accepted, or the review outcome.
- Open for the owner (not decided here): who is the human submitter and signs off; whether to open a Jira item first; whether to offer anything to AGL at all.

## 4. Not read, to finish

1. The seven chapter pages and the wiki listed as not read (section 1), and the linked `docs/contributing.md`.
2. Re-read the fetched pages directly (they came through a helper) before quoting them in an upstream message.
3. Check for a `drm-lease-manager` repository `CONTRIBUTING` in `src/drm-lease-manager` on Gerrit (not cloned in this item).
