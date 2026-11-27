---
name: Board result
about: Record a dated result measured on a board
labels: board-result
---

> **Leave out** addresses, host names, users, serial numbers and UART or
> device paths, and credentials. Write "the board" or the resource name only.

## Board

- Board and carrier:
- Image and kernel version:
- Date of the measurement (YYYY-MM-DD):

## Test

- Repository commit:
- Test program (path under `tests/board` or `tools`):

## Command and verdict

Exact command, with addresses and paths to the board removed:

```
<command>
```

Verdict lines, copied verbatim:

```
<verdict lines>
```

## Tag

One of: observed / read from source / inferred.
Anything not measured is stated as "not measured".

- Tag:

## Restored afterwards

What was changed on the board and how it was put back (services, boot chain,
files under `/tmp`), and how that was confirmed:
