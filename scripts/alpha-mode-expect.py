#!/usr/bin/env python3
"""Expected overlay colours of twoplane for premultiplied, straight and no alpha.

  alpha-mode-expect.py [--overlay RRGGBB] [--alpha 0..255] [--under RRGGBB]...
                       [--plane-alpha 0..65535] [--tolerance N]
                       [--expect-json FILE --model MODEL --rect u0,v0,u1,v1
                        [--region-name NAME]]
  alpha-mode-expect.py --self-test

Defaults are the constants of crates/composition-drm/examples/twoplane.rs: the
translucent half is magenta (ff00ff) at pixel alpha 0x80, over the green block
(00ff00) and the background (202020). Pass other values if the example changes.

Models, with pa = plane alpha / 65535 and fa = pixel alpha / 255, as in
twoplane's Look::over (KMS blending equations):
  premultiplied  buffer holds s = round(c*alpha/255);  out = pa*s + (1-pa*fa)*under
  straight       (coverage) out = pa*fa*c + (1-pa*fa)*under
  none           pixel alpha ignored;                  out = pa*c + (1-pa)*under
Arithmetic is exact (fractions) and rounds half up; twoplane uses f64 and
rounds half away from zero, so they agree except on exact ties.

For every under colour the script prints each model's 8-bit RGB (the buffer
written for that model, blended by that model), then two difference tables:
 - "models": per-channel absolute difference of each model pair, and whether a
   camera check with --tolerance can tell them apart (largest channel
   difference > 2*tolerance). A well-formed premultiplied and straight run
   give nearly the same colour, so this table alone cannot separate them.
 - "mismatch": what the picture shows when the buffer was written for one
   model but the hardware blends by another (hw=... buffer=...). This is what
   tells an ignored or inverted alpha property apart: compare the picture of a
   `twoplane --blend X` run against the row with hw equal to the real
   behaviour.
--expect-json writes an EXPECT file in the format camera-check.py reads, for
one model, one region and the first under colour.
Exit status: 0 = ok, 2 = input error. Python 3, standard library only.
"""
import json
import re
import sys
from fractions import Fraction

MODELS = ("premultiplied", "straight", "none")
OPAQUE = 65535


class InputError(Exception):
    pass


def _round(q):
    return int((q + Fraction(1, 2)) // 1)


def stored(model, colour, alpha):
    """Channel values written into the buffer (premultiplied only scales)."""
    if model != "premultiplied":
        return tuple(colour)
    return tuple((c * alpha + 127) // 255 for c in colour)


def blend(model, colour, alpha, under, plane_alpha=OPAQUE, buffer=None):
    """Composited 8-bit RGB of one overlay pixel over `under`.

    `model` is how the hardware blends; `buffer` (default `model`) is the model
    the buffer contents were written for.
    """
    buffer = buffer or model
    for m in (model, buffer):
        if m not in MODELS:
            raise InputError("unknown model " + str(m))
    if not 0 <= plane_alpha <= OPAQUE:
        raise InputError("plane alpha must be in 0..65535")
    if not 0 <= alpha <= 255:
        raise InputError("alpha must be in 0..255")
    pa = Fraction(plane_alpha, OPAQUE)
    fa = Fraction(alpha, 255)
    out = []
    for c, u in zip(stored(buffer, colour, alpha), under):
        if model == "premultiplied":
            v = pa * c + (1 - pa * fa) * u
        elif model == "straight":
            v = pa * fa * c + (1 - pa * fa) * u
        else:
            v = pa * c + (1 - pa) * u
        out.append(min(255, max(0, _round(v))))
    return tuple(out)


def hexc(c):
    return "#%02x%02x%02x" % tuple(c)


def parse_rgb(s):
    if not re.fullmatch(r"#?[0-9a-fA-F]{6}", s):
        raise InputError("bad colour " + s)
    s = s.lstrip("#")
    return tuple(int(s[i:i + 2], 16) for i in (0, 2, 4))


def diffs(results, tol):
    """List of (a, b, per-channel difference, separable) for each model pair."""
    rows = []
    for i, a in enumerate(MODELS):
        for b in MODELS[i + 1:]:
            d = tuple(abs(x - y) for x, y in zip(results[a], results[b]))
            rows.append((a, b, d, max(d) > 2 * tol))
    return rows


def expect_doc(rgb, rect, name, tol):
    return {"rotation": 0,
            "panel": {"corners": [[0, 0], [1, 0], [1, 1], [0, 1]],
                      "unit": "frac"},
            "tolerance": tol,
            "regions": [{"name": name, "rect": list(rect),
                         "expect": list(rgb)}]}


def report(colour, alpha, unders, plane_alpha, tol):
    lines = ["overlay %s pixel-alpha %#04x plane-alpha %d tolerance %g"
             % (hexc(colour), alpha, plane_alpha, tol)]
    first = None
    for under in unders:
        res = {m: blend(m, colour, alpha, under, plane_alpha) for m in MODELS}
        first = first or res
        lines.append("under %s" % hexc(under))
        for m in MODELS:
            lines.append("  %-13s %s" % (m, hexc(res[m])))
        for a, b, d, sep in diffs(res, tol):
            lines.append("  models %s vs %s: dR=%d dG=%d dB=%d %s"
                         % (a, b, d[0], d[1], d[2],
                            "separable" if sep else "NOT separable"))
        for hw in MODELS:
            for buf in MODELS:
                if hw == buf:
                    continue
                got = blend(hw, colour, alpha, under, plane_alpha, buf)
                d = tuple(abs(x - y) for x, y in zip(got, res[buf]))
                lines.append("  mismatch hw=%s buffer=%s: %s dR=%d dG=%d dB=%d %s"
                             % (hw, buf, hexc(got), d[0], d[1], d[2],
                                "separable" if max(d) > 2 * tol
                                else "NOT separable"))
    return lines, first


def parse_args(argv):
    o = {"overlay": "ff00ff", "alpha": 0x80, "under": [], "plane": OPAQUE,
         "tol": 40.0, "json": None, "model": None, "rect": None,
         "name": "overlay translucent"}
    keys = {"--overlay": "overlay", "--alpha": "alpha", "--under": "under",
            "--plane-alpha": "plane", "--tolerance": "tol",
            "--expect-json": "json", "--model": "model", "--rect": "rect",
            "--region-name": "name"}
    i = 0
    try:
        while i < len(argv):
            a = argv[i]
            if a not in keys:
                raise InputError("unknown argument " + a)
            i += 1
            v = argv[i]
            k = keys[a]
            if k == "under":
                o[k].append(v)
            elif k in ("alpha", "plane"):
                o[k] = int(v, 0)
            elif k == "tol":
                o[k] = float(v)
            elif k == "rect":
                o[k] = [float(x) for x in v.split(",")]
                if len(o[k]) != 4:
                    raise ValueError(v)
            else:
                o[k] = v
            i += 1
    except (IndexError, ValueError):
        raise InputError("bad or missing option value")
    return o


def main(argv):
    if argv == ["--self-test"]:
        return self_test()
    o = parse_args(argv)
    unders = [parse_rgb(u) for u in (o["under"] or ["00ff00", "202020"])]
    colour = parse_rgb(o["overlay"])
    lines, first = report(colour, o["alpha"], unders, o["plane"], o["tol"])
    print("\n".join(lines))
    if o["json"]:
        if o["model"] not in MODELS or o["rect"] is None:
            raise InputError("--expect-json needs --model and --rect")
        rgb = first[o["model"]]
        try:
            with open(o["json"], "w") as f:
                json.dump(expect_doc(rgb, o["rect"], o["name"], o["tol"]), f,
                          indent=1)
                f.write("\n")
        except OSError as e:
            raise InputError("cannot write output: %s" % e)
        print("wrote %s" % o["json"])
    return 0


def self_test():
    import io
    import unittest

    M, G, BG, K = (255, 0, 255), (0, 255, 0), (32, 32, 32), (0, 0, 0)

    class T(unittest.TestCase):
        def test_hand_computed(self):
            # premultiplied: s=(128,0,128); R=128, G=127/255*255=127
            self.assertEqual(blend("premultiplied", M, 128, G), (128, 127, 128))
            # straight: 128/255*255 = 128; 127/255*255 = 127
            self.assertEqual(blend("straight", M, 128, G), (128, 127, 128))
            # over 0x20: 128 + 127/255*32 = 143.94, 127/255*32 = 15.94
            self.assertEqual(blend("premultiplied", M, 128, BG), (144, 16, 144))
            self.assertEqual(blend("straight", M, 128, BG), (144, 16, 144))
            self.assertEqual(blend("none", M, 128, G), (255, 0, 255))
            self.assertEqual(blend("none", M, 128, BG), (255, 0, 255))

        def test_mismatch(self):
            # premultiplied buffer (128,0,128) blended as straight:
            # 128*128/255 + 127/255*u = 64.25 + .498u
            self.assertEqual(blend("straight", M, 128, K, buffer="premultiplied"),
                             (64, 0, 64))
            self.assertEqual(blend("straight", M, 128, G, buffer="premultiplied"),
                             (64, 127, 64))
            # straight buffer blended as premultiplied: c + (1-fa)*u
            self.assertEqual(blend("premultiplied", M, 128, K, buffer="straight"),
                             (255, 0, 255))
            self.assertEqual(blend("premultiplied", M, 128, G, buffer="straight"),
                             (255, 127, 255))

        def test_plane_alpha_half(self):
            # pa = 32768/65535 = 0.500008; pa*fa = 0.25099
            self.assertEqual(blend("straight", M, 128, K, 32768), (64, 0, 64))
            self.assertEqual(blend("premultiplied", M, 128, K, 32768),
                             (64, 0, 64))
            self.assertEqual(blend("none", M, 128, K, 32768), (128, 0, 128))
            self.assertEqual(blend("none", M, 128, G, 32768), (128, 127, 128))
            self.assertEqual(blend("straight", M, 128, G, 32768), (64, 191, 64))
            self.assertEqual(blend("premultiplied", M, 128, G, 32768),
                             (64, 191, 64))

        def test_plane_alpha_full_is_none(self):
            for m in MODELS:
                for a in (0, 1, 128, 255):
                    self.assertEqual(blend(m, M, a, G, 65535),
                                     blend(m, M, a, G))
            # a plane alpha of 65535 leaves 'none' as plain replacement
            self.assertEqual(blend("none", M, 7, G, 65535), M)
            for u in (G, BG):
                self.assertEqual({blend(m, M, 255, u, 65535) for m in MODELS},
                                 {M})

        def test_plane_alpha_zero(self):
            for m in MODELS:
                self.assertEqual(blend(m, M, 128, G, 0), G)

        def test_rounding(self):
            # premultiplied store rounds to nearest: 255*128/255 = 128,
            # 1*128/255 = 0.50 -> 1? (128+127)//255 = 1; 2*128/255 = 1.004 -> 1
            self.assertEqual(stored("premultiplied", (255, 1, 2), 128),
                             (128, 1, 1))
            self.assertEqual(stored("premultiplied", (10, 0, 0), 128)[0], 5)
            # half rounds up in the final value
            self.assertEqual(_round(Fraction(5, 2)), 3)
            self.assertEqual(_round(Fraction(499, 200)), 2)
            # 128/255 = 0.50196 -> 1, 127/255 = 0.498 -> 0
            self.assertEqual(blend("straight", (128, 0, 0), 1, K)[0], 1)
            self.assertEqual(blend("straight", (127, 0, 0), 1, K)[0], 0)
            self.assertEqual(blend("straight", (255, 0, 0), 255 // 2 + 1, K)[0],
                             128)
            self.assertEqual(blend("straight", (255, 0, 0), 127, K)[0], 127)

        def test_diffs_and_json(self):
            res = {m: blend(m, M, 128, G, 32768) for m in MODELS}
            d = {(a, b): (x, s) for a, b, x, s in diffs(res, 40)}
            self.assertEqual(d[("premultiplied", "straight")], ((0, 0, 0), False))
            self.assertEqual(d[("straight", "none")], ((64, 64, 64), False))
            doc = expect_doc(res["none"], (0.1, 0.2, 0.3, 0.4), "x", 40)
            self.assertEqual(doc["regions"][0]["expect"], [128, 127, 128])
            self.assertEqual(doc["panel"]["unit"], "frac")
            self.assertEqual(doc["rotation"], 0)

        def test_cli(self):
            old, sys.stdout = sys.stdout, io.StringIO()
            try:
                self.assertEqual(main(["--plane-alpha", "32768"]), 0)
                out = sys.stdout.getvalue()
            finally:
                sys.stdout = old
            for m in MODELS:
                self.assertIn("  %-13s" % m, out)
            with self.assertRaises(InputError):
                main(["--plane-alpha", "70000"])
            with self.assertRaises(InputError):
                main(["--bogus"])
            with self.assertRaises(InputError):
                main(["--overlay", "zzz"])

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(T)
    res = unittest.TextTestRunner(verbosity=1).run(suite)
    return 0 if res.wasSuccessful() else 1


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except InputError as e:
        print("alpha-mode-expect: %s" % e, file=sys.stderr)
        sys.exit(2)
