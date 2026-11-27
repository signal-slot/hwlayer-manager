//! Frame generator and sweep driver (design sections 5 and 6;
//! `notes/frame-generator.md`).
//!
//! [`generate`] is a pure function of the topology, its name, the master
//! seed and the sample count: it walks named families in a fixed order,
//!
//! | family | axis |
//! |---|---|
//! | `fill` | layer count `1 ..= min(P+2, 16)`, full-screen 1:1 |
//! | `zperm` | z permutations (all for `n <= 4`), disjoint and overlapping |
//! | `fmt` | each format of the planes alone, then over a common base |
//! | `scale` | 2x/4x up, 2x/4x/16x down, width only; alone and over a base |
//! | `tie` | equal `z` (all, first two), disjoint and overlapping |
//! | `alpha0` | one or all layers with alpha 0 (its own class) |
//! | `rand` | sampled from all axes with a per-case seed |
//!
//! where `P` is the number of planes that reach the first output (one output
//! per sweep in v1). Every layer lies inside the output (design section 7,
//! "Clipping": first versions are restricted to such frames). Frames are
//! named `<family>.<index>` and packed into xcheck files (see
//! [`FrameSet`]); each is written through [`xcheck::write`], so any finding
//! replays from a file.
//!
//! The PRNG is SplitMix64, written here; no registry dependency, no hash map
//! iteration. `case_seed = splitmix64(master ^ fnv1a64(topology) ^
//! (family << 32) ^ index)`, so one family's frames do not change when another
//! family or the sample count changes. [`GEN_VERSION`] is bumped by any change
//! that alters produced frames.

use std::fmt::Write as _;

use composition_core::limits::{MAX_BUFFERS, MAX_LAYERS};

use crate::compare::origin_word;
use crate::state::{rotation, BlendMode, Rect, URect};
use crate::xcheck::{self, BufferDef, Frame, Layer, XCheck};

/// Generator version; bump on any change that alters produced frames.
pub const GEN_VERSION: u32 = 1;

/// Default master seed (fixed; never from time or environment).
pub const DEFAULT_SEED: u64 = 0x5eed_11f7_0ffc_4226;

/// Default number of sampled (`rand`) frames.
pub const DEFAULT_SAMPLES: u32 = 200;

/// No family produces more frames than this.
pub const FAMILY_CAP: usize = 500;

/// Frames per written file at most.
const FRAMES_PER_FILE: usize = 250;

/// Family names, in generation order; the position is the family id in the
/// case seed.
pub const FAMILIES: [&str; 7] = ["fill", "zperm", "fmt", "scale", "tie", "alpha0", "rand"];

/// SplitMix64 step.
pub fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// FNV-1a, 64 bit.
pub fn fnv1a64(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The seed of one case.
pub fn case_seed(master: u64, topology: &str, family: usize, index: u32) -> u64 {
    splitmix64(master ^ fnv1a64(topology) ^ ((family as u64) << 32) ^ u64::from(index))
}

/// A small deterministic PRNG.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// Uniform in `0..n` (`n > 0`).
    fn below(&mut self, n: u32) -> u32 {
        (self.next() % u64::from(n)) as u32
    }
    /// True with probability `percent`/100.
    fn chance(&mut self, percent: u32) -> bool {
        self.below(100) < percent
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

/// Bounds of a sweep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    /// Master seed.
    pub seed: u64,
    /// Number of `rand` frames.
    pub samples: u32,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            samples: DEFAULT_SAMPLES,
        }
    }
}

/// A layer before buffers are numbered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GenLayer {
    buf: (u32, u32, [u8; 4], u64),
    src: URect,
    dst: Rect,
    z: i32,
    alpha: u16,
    blend: BlendMode,
    transform: u32,
}

/// A generated frame before it is packed into a file.
#[derive(Clone, Debug, PartialEq, Eq)]
struct GenFrame {
    family: usize,
    index: u32,
    layers: Vec<GenLayer>,
}

impl GenFrame {
    fn name(&self) -> String {
        format!("{}.{:04}", FAMILIES[self.family], self.index)
    }
}

/// One generated xcheck file: the topology plus a set of frames of one
/// family, with the buffers they use numbered from 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameSet {
    /// Family name.
    pub family: &'static str,
    /// Chunk number within the family (0, 1, ...).
    pub chunk: u32,
    /// The file as a model.
    pub xcheck: XCheck,
    /// The file as text: `#` comments (generator version, seed, family,
    /// topology, origin) followed by [`xcheck::write`] output.
    pub text: String,
}

impl FrameSet {
    /// File name: `<topology>.<origin>.<family>.<chunk>.xcheck`.
    pub fn file_name(&self, topology: &str) -> String {
        format!(
            "{topology}.{}.{}.{:02}.xcheck",
            origin_word(self.xcheck.origin.kind),
            self.family,
            self.chunk
        )
    }
}

/// Topology facts the families draw from.
struct Topo {
    out_w: u32,
    out_h: u32,
    planes: usize,
    /// Format union in dump order (first plane first), unique.
    formats: Vec<([u8; 4], u64)>,
    /// Format accepted by every plane reaching the output, else the first.
    common: Option<([u8; 4], u64)>,
    blends: Vec<BlendMode>,
    base_blend: BlendMode,
    transforms: Vec<u32>,
}

fn topo(x: &XCheck) -> Option<Topo> {
    let out = x.outputs.first()?;
    let planes: Vec<_> = x.planes.iter().filter(|p| p.outputs & 1 != 0).collect();
    let mut formats = Vec::new();
    for p in &planes {
        for f in &p.formats {
            let k = (f.fourcc, f.modifier);
            if !formats.contains(&k) {
                formats.push(k);
            }
        }
    }
    let common = formats
        .iter()
        .copied()
        .find(|k| {
            planes
                .iter()
                .all(|p| p.formats.iter().any(|f| (f.fourcc, f.modifier) == *k))
        })
        .or_else(|| formats.first().copied());
    let all_modes = [
        BlendMode::Opaque,
        BlendMode::Premultiplied,
        BlendMode::Coverage,
    ];
    let blends: Vec<BlendMode> = all_modes
        .into_iter()
        .filter(|m| planes.iter().any(|p| p.blend.contains(*m)))
        .collect();
    let base_blend = if !planes.is_empty()
        && planes
            .iter()
            .all(|p| p.blend.contains(BlendMode::Premultiplied))
    {
        BlendMode::Premultiplied
    } else {
        BlendMode::Opaque
    };
    let transforms: Vec<u32> = [
        rotation::ROTATE_90,
        rotation::ROTATE_180,
        rotation::ROTATE_270,
        rotation::REFLECT_X,
        rotation::REFLECT_Y,
    ]
    .into_iter()
    .filter(|t| planes.iter().any(|p| p.transforms & t != 0))
    .collect();
    Some(Topo {
        out_w: out.width,
        out_h: out.height,
        planes: planes.len(),
        formats,
        common,
        blends,
        base_blend,
        transforms,
    })
}

fn is_yuv(f: [u8; 4]) -> bool {
    matches!(
        &f,
        b"NV12"
            | b"NV21"
            | b"NV16"
            | b"NV61"
            | b"NV24"
            | b"NV42"
            | b"NV15"
            | b"P010"
            | b"YUYV"
            | b"YVYU"
            | b"UYVY"
            | b"VYUY"
            | b"YU12"
            | b"YV12"
    )
}

/// Rounds down to even for YUV formats (chroma subsampling), at least 2.
fn even_for(f: [u8; 4], v: u32) -> u32 {
    if is_yuv(f) {
        (v & !1).max(2)
    } else {
        v.max(1)
    }
}

impl Topo {
    fn n_max(&self) -> usize {
        (self.planes + 2).min(MAX_LAYERS)
    }

    /// A 1:1 layer of `fmt` covering `dst`.
    fn plain(&self, fmt: ([u8; 4], u64), dst: Rect, z: i32) -> GenLayer {
        let w = even_for(fmt.0, dst.w);
        let h = even_for(fmt.0, dst.h);
        GenLayer {
            buf: (w, h, fmt.0, fmt.1),
            src: URect { x: 0, y: 0, w, h },
            dst: Rect { w, h, ..dst },
            z,
            alpha: 0xFFFF,
            blend: self.base_blend,
            transform: rotation::ROTATE_0,
        }
    }

    fn full(&self) -> Rect {
        Rect {
            x: 0,
            y: 0,
            w: self.out_w,
            h: self.out_h,
        }
    }

    /// Destination of layer `i` of `n`: disjoint columns, or rectangles
    /// shifted so that each overlaps its neighbours.
    fn slot(&self, i: usize, n: usize, overlap: bool) -> Rect {
        let n = n as u32;
        let i = i as u32;
        if overlap {
            let w = (self.out_w / 2).max(1);
            let h = (self.out_h / 2).max(1);
            let step_x = (self.out_w - w) / n.max(1);
            let step_y = (self.out_h - h) / n.max(1);
            Rect {
                x: (i * step_x) as i32,
                y: (i * step_y) as i32,
                w,
                h,
            }
        } else {
            let w = (self.out_w / n).max(1);
            Rect {
                x: (i * w) as i32,
                y: 0,
                w,
                h: (self.out_h / 2).max(1),
            }
        }
    }

    fn fill(&self) -> Vec<Vec<GenLayer>> {
        let Some(c) = self.common else {
            return Vec::new();
        };
        (1..=self.n_max())
            .map(|n| {
                (0..n)
                    .map(|i| self.plain(c, self.full(), i as i32))
                    .collect()
            })
            .collect()
    }

    fn zperm(&self) -> Vec<Vec<GenLayer>> {
        let Some(c) = self.common else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for n in 2..=self.n_max() {
            for z in z_orders(n) {
                for overlap in [false, true] {
                    out.push(
                        (0..n)
                            .map(|i| self.plain(c, self.slot(i, n, overlap), z[i]))
                            .collect(),
                    );
                }
            }
        }
        out
    }

    fn fmt(&self) -> Vec<Vec<GenLayer>> {
        let mut out = Vec::new();
        let small = Rect {
            x: 0,
            y: 0,
            w: (self.out_w / 4).max(2),
            h: (self.out_h / 4).max(2),
        };
        for &f in &self.formats {
            out.push(vec![self.plain(f, small, 0)]);
        }
        if let Some(c) = self.common {
            for &f in &self.formats {
                out.push(vec![self.plain(c, self.full(), 0), self.plain(f, small, 1)]);
            }
        }
        out
    }

    fn scale(&self) -> Vec<Vec<GenLayer>> {
        let Some(c) = self.common else {
            return Vec::new();
        };
        // (source numerator, source denominator) per axis: src = dst * n / d.
        let factors: [((u32, u32), (u32, u32)); 6] = [
            ((1, 2), (1, 2)),   // 2x up
            ((1, 4), (1, 4)),   // 4x up
            ((2, 1), (2, 1)),   // 2x down
            ((4, 1), (4, 1)),   // 4x down
            ((16, 1), (16, 1)), // 16x down
            ((1, 2), (1, 1)),   // width only, 2x up
        ];
        let dst = Rect {
            x: 0,
            y: 0,
            w: even_for(c.0, (self.out_w / 4).max(2)),
            h: even_for(c.0, (self.out_h / 4).max(2)),
        };
        let mut out = Vec::new();
        for over_base in [false, true] {
            for ((wn, wd), (hn, hd)) in factors {
                let sw = even_for(c.0, dst.w * wn / wd);
                let sh = even_for(c.0, dst.h * hn / hd);
                if sw > 8192 || sh > 8192 {
                    continue;
                }
                let layer = GenLayer {
                    buf: (sw, sh, c.0, c.1),
                    src: URect {
                        x: 0,
                        y: 0,
                        w: sw,
                        h: sh,
                    },
                    dst,
                    z: i32::from(over_base),
                    alpha: 0xFFFF,
                    blend: self.base_blend,
                    transform: rotation::ROTATE_0,
                };
                let mut frame = Vec::new();
                if over_base {
                    frame.push(self.plain(c, self.full(), 0));
                }
                frame.push(layer);
                out.push(frame);
            }
        }
        out
    }

    fn tie(&self) -> Vec<Vec<GenLayer>> {
        let Some(c) = self.common else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for n in 2..=4.min(self.n_max()) {
            // All equal; then the first two equal and the rest above.
            let orders: [Vec<i32>; 2] =
                [vec![0; n], (0..n).map(|i| (i as i32 - 1).max(0)).collect()];
            for z in &orders {
                for overlap in [false, true] {
                    out.push(
                        (0..n)
                            .map(|i| self.plain(c, self.slot(i, n, overlap), z[i]))
                            .collect(),
                    );
                }
            }
        }
        out
    }

    fn alpha0(&self) -> Vec<Vec<GenLayer>> {
        let Some(c) = self.common else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for n in 2..=self.n_max() {
            let base: Vec<GenLayer> = (0..n)
                .map(|i| self.plain(c, self.full(), i as i32))
                .collect();
            let which: Vec<usize> = if n <= 4 {
                (0..n).collect()
            } else {
                vec![0, n - 1]
            };
            for w in which {
                let mut f = base.clone();
                f[w].alpha = 0;
                out.push(f);
            }
            let mut all = base.clone();
            for l in &mut all {
                l.alpha = 0;
            }
            out.push(all);
        }
        out
    }

    fn sample(&self, rng: &mut Rng) -> Vec<GenLayer> {
        let Some(c) = self.common else {
            return Vec::new();
        };
        let n_max = self.n_max() as u32;
        // At least a third of the frames at n >= P (just fits / just fails).
        let lo = (self.planes as u32).clamp(1, n_max);
        let n = if rng.chance(34) {
            lo + rng.below(n_max - lo + 1)
        } else {
            1 + rng.below(n_max)
        } as usize;
        let sizes = [
            (16, 16),
            (64, 64),
            (320, 240),
            (17, 15),
            (33, 33),
            (641, 361),
            (self.out_w / 2, self.out_h / 2),
            (self.out_w, self.out_h),
        ];
        let mut z: Vec<i32> = (0..n as i32).collect();
        for i in (1..n).rev() {
            z.swap(i, rng.below(i as u32 + 1) as usize);
        }
        (0..n)
            .map(|i| {
                let fmt = if rng.chance(70) || self.formats.is_empty() {
                    c
                } else {
                    rng.pick(&self.formats)
                };
                let (dw, dh) = rng.pick(&sizes);
                let dw = even_for(fmt.0, dw.min(self.out_w));
                let dh = even_for(fmt.0, dh.min(self.out_h));
                let x = rng.below(self.out_w - dw + 1) as i32;
                let y = rng.below(self.out_h - dh + 1) as i32;
                let (sw, sh) = if rng.chance(80) {
                    (dw, dh)
                } else {
                    let (n, d) = rng.pick(&[(1, 2), (2, 1), (1, 4), (4, 1)]);
                    (even_for(fmt.0, dw * n / d), even_for(fmt.0, dh * n / d))
                };
                let alpha = if rng.chance(80) {
                    0xFFFF
                } else {
                    rng.pick(&[0x8000, 0x0001])
                };
                let blend = if rng.chance(80) || self.blends.is_empty() {
                    self.base_blend
                } else {
                    rng.pick(&self.blends)
                };
                let transform = if rng.chance(80) || self.transforms.is_empty() {
                    rotation::ROTATE_0
                } else {
                    rng.pick(&self.transforms)
                };
                GenLayer {
                    buf: (sw, sh, fmt.0, fmt.1),
                    src: URect {
                        x: 0,
                        y: 0,
                        w: sw,
                        h: sh,
                    },
                    dst: Rect { x, y, w: dw, h: dh },
                    z: z[i],
                    alpha,
                    blend,
                    transform,
                }
            })
            .collect()
    }
}

/// z orders for `n` layers: all permutations for `n <= 4`; else identity,
/// reversed, every rotation and every adjacent swap.
fn z_orders(n: usize) -> Vec<Vec<i32>> {
    let id: Vec<i32> = (0..n as i32).collect();
    if n <= 4 {
        let mut all = Vec::new();
        permute(&mut id.clone(), 0, &mut all);
        all.sort();
        return all;
    }
    let mut v = vec![id.clone(), id.iter().rev().copied().collect()];
    for r in 1..n {
        let mut z = id.clone();
        z.rotate_left(r);
        v.push(z);
    }
    for i in 0..n - 1 {
        let mut z = id.clone();
        z.swap(i, i + 1);
        v.push(z);
    }
    v
}

fn permute(v: &mut Vec<i32>, k: usize, out: &mut Vec<Vec<i32>>) {
    if k == v.len() {
        out.push(v.clone());
        return;
    }
    for i in k..v.len() {
        v.swap(k, i);
        permute(v, k + 1, out);
        v.swap(k, i);
    }
}

/// Generates the frames of every family for `x` (its first output), named
/// after `topology` (the file stem) for the case seeds.
fn frames(x: &XCheck, topology: &str, bounds: Bounds) -> Vec<GenFrame> {
    let Some(t) = topo(x) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (family, name) in FAMILIES.iter().enumerate() {
        let sets = match *name {
            "fill" => t.fill(),
            "zperm" => t.zperm(),
            "fmt" => t.fmt(),
            "scale" => t.scale(),
            "tie" => t.tie(),
            "alpha0" => t.alpha0(),
            _ => (0..bounds.samples)
                .map(|i| t.sample(&mut Rng(case_seed(bounds.seed, topology, family, i))))
                .collect(),
        };
        assert!(
            sets.len() <= FAMILY_CAP || *name == "rand",
            "family {name} above {FAMILY_CAP} frames"
        );
        for (index, layers) in sets.into_iter().enumerate() {
            if !layers.is_empty() {
                out.push(GenFrame {
                    family,
                    index: index as u32,
                    layers,
                });
            }
        }
    }
    out
}

/// Packs frames of one family into files.
struct Packer<'a> {
    x: &'a XCheck,
    topology: &'a str,
    bounds: Bounds,
    family: usize,
    chunk: u32,
    buffers: Vec<BufferDef>,
    frames: Vec<Frame>,
    out: Vec<FrameSet>,
}

impl Packer<'_> {
    fn add(&mut self, f: &GenFrame) {
        let new: Vec<_> = {
            let mut v = Vec::new();
            for l in &f.layers {
                let have = self
                    .buffers
                    .iter()
                    .any(|b| (b.width, b.height, b.fourcc, b.modifier) == l.buf);
                if !have && !v.contains(&l.buf) {
                    v.push(l.buf);
                }
            }
            v
        };
        if self.buffers.len() + new.len() > MAX_BUFFERS || self.frames.len() >= FRAMES_PER_FILE {
            self.flush();
        }
        let output = self.x.outputs[0].id;
        let mut layers = Vec::new();
        for l in &f.layers {
            let id = match self
                .buffers
                .iter()
                .find(|b| (b.width, b.height, b.fourcc, b.modifier) == l.buf)
            {
                Some(b) => b.id,
                None => {
                    let id = self.buffers.len() as u32 + 1;
                    self.buffers.push(BufferDef {
                        id,
                        width: l.buf.0,
                        height: l.buf.1,
                        fourcc: l.buf.2,
                        modifier: l.buf.3,
                    });
                    id
                }
            };
            layers.push(Layer {
                buffer: id,
                src: l.src,
                dst: l.dst,
                z: l.z,
                alpha: l.alpha,
                blend: l.blend,
                transform: l.transform,
            });
        }
        self.frames.push(Frame {
            name: f.name(),
            output,
            layers,
        });
    }

    fn flush(&mut self) {
        if self.frames.is_empty() {
            return;
        }
        let mut x = self.x.clone();
        x.buffers = std::mem::take(&mut self.buffers);
        x.frames = std::mem::take(&mut self.frames);
        let body = xcheck::write(&x).expect("generated frames are valid xcheck");
        let mut text = String::new();
        let _ = writeln!(text, "# generated by liftoff-crosscheck sweep");
        let _ = writeln!(text, "# GEN_VERSION={GEN_VERSION}");
        let _ = writeln!(
            text,
            "# seed=0x{:016x} samples={}",
            self.bounds.seed, self.bounds.samples
        );
        let _ = writeln!(
            text,
            "# family={} chunk={} topology={}",
            FAMILIES[self.family], self.chunk, self.topology
        );
        let _ = writeln!(
            text,
            "# origin {} {} {}",
            origin_word(x.origin.kind),
            x.origin.date,
            x.origin.text
        );
        let _ = writeln!(
            text,
            "# no hardware claim: generated frames on a {} topology",
            origin_word(x.origin.kind)
        );
        text.push_str(&body);
        self.out.push(FrameSet {
            family: FAMILIES[self.family],
            chunk: self.chunk,
            xcheck: x,
            text,
        });
        self.chunk += 1;
    }
}

/// Generates every family's frames for the topology of `x` (its own frames
/// and buffers are not used) and packs them into files: one family per file,
/// at most 250 frames and [`MAX_BUFFERS`] buffers each. Deterministic in
/// `(x, topology, bounds)`.
pub fn generate(x: &XCheck, topology: &str, bounds: Bounds) -> Vec<FrameSet> {
    let all = frames(x, topology, bounds);
    let mut sets = Vec::new();
    for family in 0..FAMILIES.len() {
        let mut p = Packer {
            x,
            topology,
            bounds,
            family,
            chunk: 0,
            buffers: Vec::new(),
            frames: Vec::new(),
            out: Vec::new(),
        };
        for f in all.iter().filter(|f| f.family == family) {
            p.add(f);
        }
        p.flush();
        sets.extend(p.out);
    }
    sets
}

/// A file holding only `frame` of `x` and the buffers it uses (ids kept),
/// for replaying one finding.
pub fn single_frame(x: &XCheck, frame: &str) -> Option<XCheck> {
    let f = x.frames.iter().find(|f| f.name == frame)?.clone();
    let mut s = x.clone();
    s.buffers
        .retain(|b| f.layers.iter().any(|l| l.buffer == b.id));
    s.frames = vec![f];
    Some(s)
}
