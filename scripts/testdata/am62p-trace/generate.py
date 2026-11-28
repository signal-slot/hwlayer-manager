#!/usr/bin/env python3
"""Regenerate the synthetic fixtures in this directory (run from here)."""
names = ["safety-only", "app-under-safety", "app-replaced", "safety-flipped",
         "app-off", "refused-no-safety", "refused-blank",
         "refused-foreign-safety"]


def run(skip=()):
    L = ["BEGIN hwlayer-manager board test am62p_r5f"]
    for n, nm in enumerate(names, 1):
        if n in skip:
            continue
        L.append("SCENE %d %s: status 0 (expected 0), commits %d latched %d "
                 "(+1, expected +1), presented yes, released 0 (expected 0): ok"
                 % (n, nm, n, n))
    return L


def nl(L):
    return ("\n".join(L) + "\n").encode()


def w(name, data):
    with open(name, "wb") as f:
        f.write(data)


pre = ["board test: output enable 0", "board test: backlight 0"]
res = ["RESULT PASS (8 scenes, vsyncs 480)"]
full = run() + res
w("clean.txt", nl(pre + full))
w("wrapped.txt",
  b"ene 1 safety-only: status 0 (expected 0), commits 1 latched 1\n"
  + nl(pre + full) + nl(run()[6:] + ["RESULT PASS (8 scenes, vsyncs 470)"]))
w("nul.txt", b"\0" * 64 + nl(pre + full).replace(b"\n", b"\n\0\0\0", 3)
  + b"\xff\xfe junk\n" + b"\0" * 64)
w("two-runs.txt", nl(pre + run() + ["RESULT FAIL (1 checks failed, vsyncs 470)"]
                     + pre + full))
w("incomplete.txt", nl(pre + run()))
w("incomplete-scene.txt", nl(pre + run(skip=(7,)) + res))
