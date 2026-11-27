//! The i.MX 95 DPU's tables for the two-plane backend (`ce_twoplane_hw` and
//! `ce_twoplane_port` of `ce_twoplane.h`), as `docs/imx95-twoplane-port.md`
//! designs them (M2-05).
//!
//! This is data and pure checks for a future `display_port_imx95.c`: which
//! DPU units carry each plane, which pixel formats the application plane
//! takes, the alignment rules, the latch and busy bits, the blend modes, and
//! for every hook and port member whether it is generic or SoC-specific.
//! Nothing here touches hardware, and nothing was run on a board. No value is
//! *observed*.
//!
//! Every value carries the tag of the design doc:
//!
//! - [`Tag::ReadFromSource`]: checked in the file the design doc names. The
//!   source refs are the design doc's: `Z` upstream Zephyr at
//!   `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (`display_socionext_dpu.c`,
//!   `nxp_imx95_m7.dtsi`), `H` `hal_nxp` at
//!   `1a8937f3f8dfe5c68f59cc2a0c0c0fccdb096650` (`fsl_dpu.{h,c}`,
//!   `PERI_DISPLAY_SEERIS.h`), `B` this repository's `ce_twoplane.h` and
//!   `docs/twoplane-backend.md`, `L` `docs/imx95-dpu-source-6.6.md`.
//! - [`Tag::Inferred`]: the design doc's reasoning, not checked in a source
//!   or on hardware.
//! - [`Tag::Open`]: the design doc leaves the value open. Such values are
//!   named placeholders (`*_OPEN`) and listed in [`OPEN`].
//!
//! The AM62P port is not here and is not changed by this module.

/// How a value is known (the tags of `docs/imx95-twoplane-port.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// Checked in the source file the design doc names.
    ReadFromSource,
    /// Reasoned from those sources; not checked in a source or on hardware.
    Inferred,
    /// Left open by the design doc; the value is a placeholder.
    Open,
}

/// A table value with its tag and where it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sourced<T> {
    /// The value.
    pub value: T,
    /// How it is known.
    pub tag: Tag,
    /// The design doc's ref and file (or section) it comes from.
    pub source: &'static str,
}

const fn src<T>(value: T, tag: Tag, source: &'static str) -> Sourced<T> {
    Sourced { value, tag, source }
}

/// `CE_FOURCC` of `composition_engine.h`.
const fn fourcc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}

/// `CE_FORMAT_RGB565` (`composition_engine.h`).
pub const CE_FORMAT_RGB565: u32 = fourcc(b'R', b'G', b'1', b'6');
/// `CE_FORMAT_XRGB8888` (`composition_engine.h`).
pub const CE_FORMAT_XRGB8888: u32 = fourcc(b'X', b'R', b'2', b'4');
/// `CE_FORMAT_ARGB8888` (`composition_engine.h`).
pub const CE_FORMAT_ARGB8888: u32 = fourcc(b'A', b'R', b'2', b'4');
/// `CE_FORMAT_NV12` (`composition_engine.h`).
pub const CE_FORMAT_NV12: u32 = fourcc(b'N', b'V', b'1', b'2');

// ---------------------------------------------------------------------------
// Pixel formats of the application plane
// ---------------------------------------------------------------------------

/// A pixel format of the HAL (`kDPU_PixelFormat…`, H `fsl_dpu.h`), by name.
/// The HAL's numeric values are not in the design doc and are not recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DpuFormat {
    /// RGB565, in the stock driver's `dpu_supported_fmts` (Z).
    Rgb565,
    /// RGB888, in the stock driver's `dpu_supported_fmts` (Z). No
    /// `CE_FORMAT_*` maps to it.
    Rgb888,
    /// ARGB8888, in the stock driver's `dpu_supported_fmts` (Z).
    Argb8888,
    /// Placeholder: the design doc leaves this mapping open.
    Open(&'static str),
}

/// Placeholder for the HAL pixel format of `CE_FORMAT_XRGB8888`. The stock
/// driver's list lacks one; whether `dpu_1` has one is not checked
/// (design doc, `app_format_ok` row).
pub const XRGB8888_DPU_FORMAT_OPEN: DpuFormat = DpuFormat::Open("XRGB8888_DPU_FORMAT_OPEN");

/// One row of the application plane's format table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppFormat {
    /// `CE_FORMAT_*`.
    pub ce_format: u32,
    /// The HAL format the fetch unit is programmed with.
    pub dpu: Sourced<DpuFormat>,
    /// Bytes per pixel (`bitsPerPixel / 8` of the source buffer config).
    pub bytes_per_pixel: u32,
}

/// The application plane's formats: the `CE_FORMAT_*` the backend's plane 0
/// can carry (`ce_twoplane.h`: RGB565, XRGB8888, ARGB8888) mapped to the
/// HAL. NV12 is not a plane-0 format of the backend and is not listed.
pub const APP_FORMATS: [AppFormat; 3] = [
    AppFormat {
        ce_format: CE_FORMAT_RGB565,
        // Z `dpu_supported_fmts`; L lists RGB565 too.
        dpu: src(
            DpuFormat::Rgb565,
            Tag::ReadFromSource,
            "Z display_socionext_dpu.c dpu_supported_fmts; L",
        ),
        bytes_per_pixel: 2,
    },
    AppFormat {
        ce_format: CE_FORMAT_XRGB8888,
        // Linux takes XRGB8888 (L); the Zephyr/HAL mapping is open.
        dpu: src(
            XRGB8888_DPU_FORMAT_OPEN,
            Tag::Open,
            "design doc app_format_ok row: XRGB8888 mapping inferred, dpu_1 format not checked",
        ),
        bytes_per_pixel: 4,
    },
    AppFormat {
        ce_format: CE_FORMAT_ARGB8888,
        // Z `dpu_supported_fmts`; L lists ARGB8888 too.
        dpu: src(
            DpuFormat::Argb8888,
            Tag::ReadFromSource,
            "Z display_socionext_dpu.c dpu_supported_fmts; L",
        ),
        bytes_per_pixel: 4,
    },
];

/// The row of [`APP_FORMATS`] for a `CE_FORMAT_*`, settled or not.
pub fn app_format(ce_format: u32) -> Option<&'static AppFormat> {
    APP_FORMATS.iter().find(|f| f.ce_format == ce_format)
}

/// `app_format_ok` of the i.MX 95 port: a pure table lookup. Only formats
/// whose HAL mapping is settled are taken; XRGB8888 is refused until
/// [`XRGB8888_DPU_FORMAT_OPEN`] is resolved.
pub fn app_format_ok(ce_format: u32) -> bool {
    matches!(app_format(ce_format), Some(f) if f.dpu.tag != Tag::Open)
}

// ---------------------------------------------------------------------------
// Buffers
// ---------------------------------------------------------------------------

/// Stride alignment of a scanout buffer in bytes: `DPU_FB_PITCH_ALIGN` of the
/// stock driver (read from source, Z `display_socionext_dpu.c`).
pub const FB_PITCH_ALIGN: Sourced<u32> = src(
    32,
    Tag::ReadFromSource,
    "Z display_socionext_dpu.c DPU_FB_PITCH_ALIGN",
);

/// Base address alignment of a scanout buffer in bytes: the stock driver's
/// full-panel path wants a 32-byte-aligned buffer (read from source, Z
/// `dpu_write`, `dpu_frame_buffer_<n>`).
pub const FB_ADDR_ALIGN: Sourced<u32> = src(
    32,
    Tag::ReadFromSource,
    "Z display_socionext_dpu.c dpu_write, dpu_frame_buffer_<n>",
);

/// Placeholder: whether `ce_twoplane_buf.pa` is the DPU's bus address or the
/// M7's CPU address of the buffer (design doc, `set_app_buffer` and
/// `lookup` rows; the `m7-fb-cache` item). [`check_app_buffer`] does not
/// translate addresses.
pub const PA_ADDRESS_SPACE_OPEN: &str = "PA_ADDRESS_SPACE_OPEN";

/// The fields of `ce_twoplane_buf` the i.MX 95 checks read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppBuffer {
    /// `pa`, in the address space of [`PA_ADDRESS_SPACE_OPEN`].
    pub pa: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Stride in bytes.
    pub stride_bytes: u32,
    /// `CE_FORMAT_*`.
    pub format: u32,
}

/// Why `set_app_buffer` refuses a buffer (it then writes nothing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The format is not in [`APP_FORMATS`] or its mapping is open.
    Format,
    /// Width or height is not the panel's.
    Size,
    /// The stride is not a multiple of [`FB_PITCH_ALIGN`].
    StrideAlign,
    /// The stride is less than a row of pixels.
    StrideShort,
    /// The address is not a multiple of [`FB_ADDR_ALIGN`].
    AddrAlign,
    /// The address does not fit the 32 bits the stock driver programs
    /// (`(uint32_t)(uintptr_t)`, Z `dpu_write`).
    AddrWidth,
}

/// The generic checks of `set_app_buffer` (design doc: "the checks are
/// generic"): format, full panel, stride and address alignment. The register
/// sequence that follows is the C port's.
pub fn check_app_buffer(panel_w: u32, panel_h: u32, buf: &AppBuffer) -> Result<(), Refusal> {
    if !app_format_ok(buf.format) {
        return Err(Refusal::Format);
    }
    if buf.width != panel_w || buf.height != panel_h {
        return Err(Refusal::Size);
    }
    if buf.stride_bytes % FB_PITCH_ALIGN.value != 0 {
        return Err(Refusal::StrideAlign);
    }
    let bpp = match app_format(buf.format) {
        Some(f) => f.bytes_per_pixel,
        None => return Err(Refusal::Format),
    };
    if u64::from(buf.stride_bytes) < u64::from(buf.width) * u64::from(bpp) {
        return Err(Refusal::StrideShort);
    }
    if buf.pa % u64::from(FB_ADDR_ALIGN.value) != 0 {
        return Err(Refusal::AddrAlign);
    }
    if buf.pa > u64::from(u32::MAX) {
        return Err(Refusal::AddrWidth);
    }
    Ok(())
}

/// Stride of a safety buffer (ARGB8888, panel width): `ROUND_UP(w * 4, 32)`
/// (design doc, `safety_buffer` row; inferred from Z's buffer declaration).
/// `None` if it does not fit 32 bits.
pub fn safety_stride(width: u32) -> Option<u32> {
    let align = FB_PITCH_ALIGN.value;
    width
        .checked_mul(4)?
        .checked_add(align - 1)
        .map(|v| v / align * align)
}

/// The number of safety buffers the driver owns (design doc,
/// `safety_buffer` row: two ARGB8888 panel-sized buffers; inferred).
pub const SAFETY_BUFFERS: Sourced<u32> = src(
    2,
    Tag::Inferred,
    "design doc safety_buffer row; ce_twoplane.h plane 1",
);

// ---------------------------------------------------------------------------
// Pipeline: which DPU units carry the planes
// ---------------------------------------------------------------------------

/// A DPU processing unit, named as in H `fsl_dpu.h` (FetchYuv0..3,
/// FetchLayer0..1, LayerBlend1..6, ConstFrame0/1/4/5, ExtDst0/1/4/5). Only
/// the units the design doc uses are listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// `kDPU_ConstFrame0`.
    ConstFrame0,
    /// `kDPU_FetchYuv0`.
    FetchYuv0,
    /// `kDPU_FetchYuv1`.
    FetchYuv1,
    /// `kDPU_FetchLayer0`.
    FetchLayer0,
    /// `kDPU_LayerBlend1`.
    LayerBlend1,
    /// `kDPU_LayerBlend2`.
    LayerBlend2,
    /// ExtDst0 (`kDPU_PipelineExtDst0` for the shadow-load trigger).
    ExtDst0,
    /// Placeholder: the design doc leaves this unit open.
    Open(&'static str),
}

/// Placeholder for the safety plane's fetch unit: FetchYUV1 or FetchLayer0
/// ([`SAFETY_FETCH_CANDIDATES`]); whether either reads ARGB8888 is open
/// (design doc, "What is still unknown" 3).
pub const SAFETY_FETCH_OPEN: Unit = Unit::Open("SAFETY_FETCH_OPEN");

/// The candidates the design doc names for [`SAFETY_FETCH_OPEN`] (inferred).
pub const SAFETY_FETCH_CANDIDATES: [Unit; 2] = [Unit::FetchYuv1, Unit::FetchLayer0];

/// The units of the two-plane pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pipeline {
    /// LayerBlend1's primary input: black under the application plane.
    pub background: Sourced<Unit>,
    /// The application plane's (plane 0) fetch unit.
    pub app_fetch: Sourced<Unit>,
    /// The LayerBlend that puts the application plane over the background.
    pub app_blend: Sourced<Unit>,
    /// The safety plane's (plane 1) fetch unit.
    pub safety_fetch: Sourced<Unit>,
    /// The LayerBlend that puts the safety plane over `app_blend`.
    pub safety_blend: Sourced<Unit>,
    /// The external destination whose shadow load is the latch.
    pub ext_dst: Sourced<Unit>,
}

/// The pipeline of the design doc ("Hooks of `ce_twoplane_hw`"): the stock
/// driver's ConstFrame0 + FetchYUV0 → LayerBlend1 (read from source, Z
/// `dpu_init`), extended with a second fetch unit → LayerBlend2 → ExtDst0
/// (inferred; whether LayerBlend1 is a legal primary of LayerBlend2 is not
/// verified, "What is still unknown" 3).
pub const PIPELINE: Pipeline = Pipeline {
    background: src(
        Unit::ConstFrame0,
        Tag::ReadFromSource,
        "Z display_socionext_dpu.c dpu_init",
    ),
    app_fetch: src(
        Unit::FetchYuv0,
        Tag::ReadFromSource,
        "Z display_socionext_dpu.c dpu_init",
    ),
    app_blend: src(
        Unit::LayerBlend1,
        Tag::ReadFromSource,
        "Z display_socionext_dpu.c dpu_init",
    ),
    safety_fetch: src(
        SAFETY_FETCH_OPEN,
        Tag::Open,
        "design doc hooks section; unknown 3",
    ),
    safety_blend: src(
        Unit::LayerBlend2,
        Tag::Inferred,
        "design doc hooks section (route not verified)",
    ),
    ext_dst: src(
        Unit::ExtDst0,
        Tag::ReadFromSource,
        "Z display_socionext_dpu.c dpu_init; H fsl_dpu.c",
    ),
};

// ---------------------------------------------------------------------------
// Latch, busy and interrupts
// ---------------------------------------------------------------------------

/// `EXTDST0_TRIGGER` `Sync_Trigger`, bit 0: writing it requests the shadow
/// load (`DPU_TriggerPipelineShadowLoad`). Read from source (H `fsl_dpu.c`,
/// `PERI_DISPLAY_SEERIS.h`).
pub const EXTDST0_SYNC_TRIGGER_MASK: Sourced<u32> = src(
    1 << 0,
    Tag::ReadFromSource,
    "H fsl_dpu.c DPU_TriggerPipelineShadowLoad; PERI_DISPLAY_SEERIS.h",
);

/// `EXTDST0_STATUS` `sync_busy`, bit 8 (mask `0x100`). The bit is read from
/// source (H `PERI_DISPLAY_SEERIS.h`); its meaning, set from the trigger
/// until the shadow registers are loaded, is inferred from its name.
pub const EXTDST0_SYNC_BUSY_MASK: Sourced<u32> = src(
    1 << 8,
    Tag::ReadFromSource,
    "H PERI_DISPLAY_SEERIS.h EXTDST0_STATUS sync_busy (meaning inferred)",
);

/// `kDPU_Group0ExtDst0ShadowLoadInterrupt`: bit 3 of interrupt group 0,
/// counted as the latch. Read from source (H `fsl_dpu.h`; Z's ISR uses it as
/// "frame sent").
pub const LATCH_IRQ_GROUP: Sourced<u32> = src(0, Tag::ReadFromSource, "H fsl_dpu.h");
/// Bit of [`LATCH_IRQ_GROUP`] for the ExtDst0 shadow-load interrupt.
pub const LATCH_IRQ_BIT: Sourced<u32> = src(3, Tag::ReadFromSource, "H fsl_dpu.h");

/// The DPU's interrupt line in the M7 devicetree (`interrupts = <215 0>`,
/// read from source, Z `nxp_imx95_m7.dtsi`). One ISR serves every group bit.
pub const DPU_IRQ: Sourced<u32> = src(215, Tag::ReadFromSource, "Z nxp_imx95_m7.dtsi");

/// The DPU node's register base in the M7 devicetree (read from source, Z
/// `nxp_imx95_m7.dtsi`). HAL register offsets are relative to it (inferred).
pub const DPU_DT_BASE: Sourced<u32> = src(0x4b40_0000, Tag::ReadFromSource, "Z nxp_imx95_m7.dtsi");

/// `APP_DPU_DC` of the stock driver's ISR (read from source, Z). It differs
/// from [`DPU_DT_BASE`]; which is the interrupt registers' real base is not
/// settled. The port must not use it (design doc, inferred).
pub const STOCK_ISR_BASE_DO_NOT_USE: Sourced<u32> = src(
    0x4b78_0000,
    Tag::ReadFromSource,
    "Z display_socionext_dpu.c APP_DPU_DC",
);

/// Placeholder: whether the ExtDst0 shadow load happens at the next frame
/// start (what the backend's contract needs) or at once on the trigger.
/// **Not measured** (design doc, "Latch source", "What is still unknown" 2).
pub const SHADOW_LOAD_TIMING_OPEN: &str = "SHADOW_LOAD_TIMING_OPEN";

/// A source of a once-a-frame interrupt for `set_vsync_callback`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VsyncSource {
    /// `kDPU_Group0FrameGen0Int0..3Interrupt` (H `fsl_dpu.h`), configured by
    /// FrameGen compare registers; not examined.
    FrameGen0Int(u8),
    /// `kDPU_Group0Display0…` frame-complete bits: a name pattern in H, not
    /// checked on this SoC.
    Display0FrameComplete,
    /// Placeholder: the design doc leaves the choice open.
    Open(&'static str),
}

/// Placeholder for the per-frame interrupt of `set_vsync_callback`. The
/// ExtDst0 shadow-load interrupt fires only after a trigger, so it cannot be
/// the vsync (design doc, `set_vsync_callback` row, "What is still unknown"
/// 5).
pub const VSYNC_SOURCE_OPEN: VsyncSource = VsyncSource::Open("VSYNC_SOURCE_OPEN");

/// The vsync source the port uses, open.
pub const VSYNC_SOURCE: Sourced<VsyncSource> = src(
    VSYNC_SOURCE_OPEN,
    Tag::Open,
    "design doc set_vsync_callback row; unknown 5",
);

/// The candidates the design doc names for [`VSYNC_SOURCE_OPEN`] (inferred).
pub const VSYNC_CANDIDATES: [VsyncSource; 5] = [
    VsyncSource::FrameGen0Int(0),
    VsyncSource::FrameGen0Int(1),
    VsyncSource::FrameGen0Int(2),
    VsyncSource::FrameGen0Int(3),
    VsyncSource::Display0FrameComplete,
];

/// Whether a hook may write the ExtDst0 trigger now, given the value of
/// `EXTDST0_STATUS`. The design doc's latch model (inferred): never trigger
/// while `sync_busy` is set, since a trigger then may be lost or merged and
/// `latched_count` would not reach `commit_count`.
pub fn trigger_allowed(extdst0_status: u32) -> bool {
    extdst0_status & EXTDST0_SYNC_BUSY_MASK.value == 0
}

/// The two counters of the design doc's latch model (inferred, **not
/// measured**): `commit_count` bumped by each hook after its writes and
/// trigger, `latched_count` bumped in the ExtDst0 shadow-load interrupt.
/// Both wrap at 32 bits, as the backend compares them (`reached` in
/// `ce_twoplane.c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LatchCounters {
    /// `commit_count()`.
    pub commits: u32,
    /// `latched_count()`.
    pub latched: u32,
}

impl LatchCounters {
    /// A hook wrote its registers and triggered the shadow load.
    pub fn commit(&mut self) {
        self.commits = self.commits.wrapping_add(1);
    }

    /// The ExtDst0 shadow-load interrupt ([`LATCH_IRQ_BIT`]) fired. Commits
    /// merged into one load count as one: the counter catches up to
    /// `commits` and never passes it.
    pub fn shadow_loaded(&mut self) {
        self.latched = self.commits;
    }

    /// The backend's test (`reached` in `ce_twoplane.c`): whether a commit
    /// whose `commit_count` was `count` is latched.
    pub fn reached(&self, count: u32) -> bool {
        (self.latched.wrapping_sub(count) as i32) >= 0
    }
}

// ---------------------------------------------------------------------------
// Blend modes of the safety plane
// ---------------------------------------------------------------------------

/// A LayerBlend colour blend factor, named as `dpu_blend_mode_t` (H
/// `fsl_dpu.h`). Only those the design doc uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    /// `kDPU_BlendZero`.
    Zero,
    /// `kDPU_BlendOne`.
    One,
    /// `kDPU_BlendSecAlpha`.
    SecAlpha,
    /// `kDPU_BlendSecAlphaInv`.
    SecAlphaInv,
}

/// The colour factors of a LayerBlend (`DPU_SetLayerBlendConfig`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlendFactors {
    /// Factor of the secondary (upper) input's colour.
    pub secondary: BlendMode,
    /// Factor of the primary (lower) input's colour.
    pub primary: BlendMode,
}

/// The stock driver's fixed mode: opaque replace (read from source, Z).
pub const STOCK_BLEND: Sourced<BlendFactors> = src(
    BlendFactors {
        secondary: BlendMode::One,
        primary: BlendMode::Zero,
    },
    Tag::ReadFromSource,
    "Z display_socionext_dpu.c dpu_init",
);

/// LayerBlend2's factors for `safety_premultiplied(on)`. The enum names are
/// read from source (H); the formula is inferred (design doc,
/// `safety_premultiplied` row).
pub fn safety_blend(premultiplied: bool) -> Sourced<BlendFactors> {
    let secondary = if premultiplied {
        BlendMode::One
    } else {
        BlendMode::SecAlpha
    };
    src(
        BlendFactors {
            secondary,
            primary: BlendMode::SecAlphaInv,
        },
        Tag::Inferred,
        "design doc safety_premultiplied row; H dpu_blend_mode_t",
    )
}

// ---------------------------------------------------------------------------
// Hooks and port members
// ---------------------------------------------------------------------------

/// Whether a hook's i.MX 95 version is shared with other SoCs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Same logic on the AM62P and the i.MX 95.
    Generic,
    /// The register sequence or table is the i.MX 95's.
    SocSpecific,
}

/// One row of the design doc's hook or port-member table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    /// The member's name in `ce_twoplane.h`.
    pub name: &'static str,
    /// Generic or SoC-specific.
    pub scope: Scope,
    /// Whether it writes DPU registers (and so is one commit).
    pub writes_planes: bool,
    /// The tag of the row as a whole (the weakest of its parts).
    pub tag: Tag,
}

const fn member(name: &'static str, scope: Scope, writes_planes: bool, tag: Tag) -> Member {
    Member {
        name,
        scope,
        writes_planes,
        tag,
    }
}

/// `ce_twoplane_hw` for the i.MX 95 (design doc, "Hooks of
/// `ce_twoplane_hw`"). Every row needs code against the HAL that the stock
/// driver lacks; none is a stock-Zephyr claim.
pub const HW_HOOKS: [Member; 10] = [
    // A table lookup (APP_FORMATS); XRGB8888 mapping inferred.
    member("app_format_ok", Scope::SocSpecific, false, Tag::Inferred),
    // Checks generic, sequence SoC-specific; pa address space inferred.
    member("set_app_buffer", Scope::SocSpecific, true, Tag::Inferred),
    // LayerBlend1 disable vs fetch-unit disable: choice inferred.
    member("enable_app_plane", Scope::SocSpecific, true, Tag::Inferred),
    // Second fetch unit not in the stock driver.
    member("safety_flip", Scope::SocSpecific, true, Tag::Inferred),
    // Not hardware; sizes from DT width/height.
    member("safety_buffer", Scope::Generic, false, Tag::Inferred),
    member("safety_front", Scope::Generic, false, Tag::Inferred),
    // Writes BLENDCONTROL only if the mode changes.
    member(
        "safety_premultiplied",
        Scope::SocSpecific,
        true,
        Tag::Inferred,
    ),
    member("commit_count", Scope::Generic, false, Tag::Inferred),
    // Interrupt bit read from source; counting inferred; timing not measured.
    member("latched_count", Scope::SocSpecific, false, Tag::Inferred),
    // Per-frame source open (VSYNC_SOURCE_OPEN).
    member("set_vsync_callback", Scope::SocSpecific, false, Tag::Open),
];

/// `ce_twoplane_port` for the i.MX 95 M7 under Zephyr (design doc, "Members
/// of `ce_twoplane_port`"). None touches the DPU.
pub const PORT_MEMBERS: [Member; 5] = [
    // irq_lock() of Zephyr (B).
    member("irq_lock", Scope::Generic, false, Tag::ReadFromSource),
    // irq_unlock(key) (B).
    member("irq_unlock", Scope::Generic, false, Tag::ReadFromSource),
    // k_sem_give from the ISR, wake_arg the semaphore (B).
    member("wake", Scope::Generic, false, Tag::ReadFromSource),
    // The integrator's buffer table; pa address space inferred.
    member("lookup", Scope::Generic, false, Tag::Inferred),
    // device_is_ready (B); the result depends on DPU ownership (inferred).
    member("device_ready", Scope::Generic, false, Tag::Inferred),
];

/// Every placeholder of this module: what the design doc leaves open.
pub const OPEN: [&str; 5] = [
    // XRGB8888 → HAL pixel format.
    "XRGB8888_DPU_FORMAT_OPEN",
    // Safety plane's fetch unit and the LayerBlend1→LayerBlend2 route.
    "SAFETY_FETCH_OPEN",
    // DPU bus address or M7 CPU address in ce_twoplane_buf.pa.
    PA_ADDRESS_SPACE_OPEN,
    // ExtDst0 shadow load at frame start or on trigger.
    SHADOW_LOAD_TIMING_OPEN,
    // Per-frame interrupt for set_vsync_callback.
    "VSYNC_SOURCE_OPEN",
];

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = include_str!("../ce_twoplane.h");
    const ENGINE_HEADER: &str =
        include_str!("../../../crates/composition-ffi/include/composition_engine.h");

    /// The body of `struct <name> { ... };` in the header.
    fn struct_body(name: &str) -> &'static str {
        let start = HEADER.find(name).expect("struct in header");
        let rest = &HEADER[start..];
        let end = rest.find("};").expect("struct end");
        // 'static: HEADER is 'static.
        &HEADER[start..start + end]
    }

    #[test]
    fn formats_match_the_engine_header() {
        let cases = [
            (
                "CE_FORMAT_RGB565 CE_FOURCC('R', 'G', '1', '6')",
                CE_FORMAT_RGB565,
            ),
            (
                "CE_FORMAT_XRGB8888 CE_FOURCC('X', 'R', '2', '4')",
                CE_FORMAT_XRGB8888,
            ),
            (
                "CE_FORMAT_ARGB8888 CE_FOURCC('A', 'R', '2', '4')",
                CE_FORMAT_ARGB8888,
            ),
            (
                "CE_FORMAT_NV12 CE_FOURCC('N', 'V', '1', '2')",
                CE_FORMAT_NV12,
            ),
        ];
        for (define, _) in cases {
            assert!(ENGINE_HEADER.contains(define), "{define}");
        }
        assert_eq!(CE_FORMAT_RGB565, 0x3631_4752);
        assert_eq!(CE_FORMAT_ARGB8888, 0x3432_5241);
    }

    #[test]
    fn app_formats_settled_and_open() {
        assert!(app_format_ok(CE_FORMAT_RGB565));
        assert!(app_format_ok(CE_FORMAT_ARGB8888));
        // Open mapping: refused until settled.
        assert!(!app_format_ok(CE_FORMAT_XRGB8888));
        assert_eq!(app_format(CE_FORMAT_XRGB8888).unwrap().dpu.tag, Tag::Open);
        // Not a plane-0 format of the backend.
        assert!(!app_format_ok(CE_FORMAT_NV12));
        assert!(app_format(CE_FORMAT_NV12).is_none());
        for f in APP_FORMATS {
            match f.dpu.value {
                DpuFormat::Open(_) => assert_eq!(f.dpu.tag, Tag::Open),
                _ => assert_ne!(f.dpu.tag, Tag::Open),
            }
        }
        assert_eq!(app_format(CE_FORMAT_RGB565).unwrap().bytes_per_pixel, 2);
        assert_eq!(app_format(CE_FORMAT_ARGB8888).unwrap().bytes_per_pixel, 4);
    }

    fn good() -> AppBuffer {
        AppBuffer {
            pa: 0x8000_0000,
            width: 1280,
            height: 800,
            stride_bytes: 1280 * 4,
            format: CE_FORMAT_ARGB8888,
        }
    }

    #[test]
    fn app_buffer_checks() {
        assert_eq!(check_app_buffer(1280, 800, &good()), Ok(()));
        let b = AppBuffer {
            format: CE_FORMAT_XRGB8888,
            ..good()
        };
        assert_eq!(check_app_buffer(1280, 800, &b), Err(Refusal::Format));
        let b = AppBuffer {
            width: 1279,
            ..good()
        };
        assert_eq!(check_app_buffer(1280, 800, &b), Err(Refusal::Size));
        let b = AppBuffer {
            height: 801,
            ..good()
        };
        assert_eq!(check_app_buffer(1280, 800, &b), Err(Refusal::Size));
        let b = AppBuffer {
            stride_bytes: 1280 * 4 + 4,
            ..good()
        };
        assert_eq!(check_app_buffer(1280, 800, &b), Err(Refusal::StrideAlign));
        let b = AppBuffer {
            stride_bytes: 1280 * 4 - 32,
            ..good()
        };
        assert_eq!(check_app_buffer(1280, 800, &b), Err(Refusal::StrideShort));
        let b = AppBuffer {
            pa: 0x8000_0010,
            ..good()
        };
        assert_eq!(check_app_buffer(1280, 800, &b), Err(Refusal::AddrAlign));
        let b = AppBuffer {
            pa: 0x1_0000_0000,
            ..good()
        };
        assert_eq!(check_app_buffer(1280, 800, &b), Err(Refusal::AddrWidth));
        // RGB565: 2 bytes per pixel, 1280 * 2 = 2560 is 32-aligned.
        let b = AppBuffer {
            format: CE_FORMAT_RGB565,
            stride_bytes: 2560,
            ..good()
        };
        assert_eq!(check_app_buffer(1280, 800, &b), Ok(()));
    }

    #[test]
    fn safety_stride_rounds_to_32() {
        assert_eq!(safety_stride(1280), Some(5120));
        assert_eq!(safety_stride(1), Some(32));
        assert_eq!(safety_stride(9), Some(64));
        assert_eq!(safety_stride(0), Some(0));
        assert_eq!(safety_stride(u32::MAX / 4 + 1), None);
        assert_eq!(safety_stride(u32::MAX / 4), None);
    }

    #[test]
    fn pipeline_tags() {
        assert_eq!(PIPELINE.app_fetch.value, Unit::FetchYuv0);
        assert_eq!(PIPELINE.app_blend.value, Unit::LayerBlend1);
        assert_eq!(PIPELINE.background.value, Unit::ConstFrame0);
        assert_eq!(PIPELINE.ext_dst.value, Unit::ExtDst0);
        assert_eq!(PIPELINE.safety_fetch.tag, Tag::Open);
        assert_eq!(PIPELINE.safety_blend.tag, Tag::Inferred);
        // The safety plane must not share the application plane's units.
        for c in SAFETY_FETCH_CANDIDATES {
            assert_ne!(c, PIPELINE.app_fetch.value);
        }
        assert_ne!(PIPELINE.safety_blend.value, PIPELINE.app_blend.value);
    }

    #[test]
    fn latch_bits() {
        assert_eq!(EXTDST0_SYNC_TRIGGER_MASK.value, 0x1);
        assert_eq!(EXTDST0_SYNC_BUSY_MASK.value, 0x100);
        assert_eq!(LATCH_IRQ_GROUP.value, 0);
        assert_eq!(LATCH_IRQ_BIT.value, 3);
        assert!(trigger_allowed(0));
        assert!(trigger_allowed(!0x100));
        assert!(!trigger_allowed(0x100));
        assert_ne!(DPU_DT_BASE.value, STOCK_ISR_BASE_DO_NOT_USE.value);
        assert_eq!(VSYNC_SOURCE.tag, Tag::Open);
        assert!(!VSYNC_CANDIDATES.contains(&VSYNC_SOURCE.value));
    }

    #[test]
    fn latch_counters_merge_and_wrap() {
        let mut c = LatchCounters::default();
        c.commit();
        let first = c.commits;
        assert!(!c.reached(first));
        c.commit();
        let second = c.commits;
        // Two commits merged into one load: both latched, counted once.
        c.shadow_loaded();
        assert!(c.reached(first) && c.reached(second));
        assert_eq!(c.latched, c.commits);
        // A load with no new commit does not pass commit_count.
        c.shadow_loaded();
        assert_eq!(c.latched, second);
        // Wrap at 32 bits.
        let mut w = LatchCounters {
            commits: u32::MAX,
            latched: u32::MAX,
        };
        w.commit();
        assert_eq!(w.commits, 0);
        assert!(!w.reached(0));
        assert!(w.reached(u32::MAX));
        w.shadow_loaded();
        assert!(w.reached(0));
    }

    #[test]
    fn blend_modes() {
        let p = safety_blend(true);
        assert_eq!(p.value.secondary, BlendMode::One);
        assert_eq!(p.value.primary, BlendMode::SecAlphaInv);
        assert_eq!(p.tag, Tag::Inferred);
        let n = safety_blend(false);
        assert_eq!(n.value.secondary, BlendMode::SecAlpha);
        assert_eq!(n.value.primary, BlendMode::SecAlphaInv);
        assert_ne!(p.value, STOCK_BLEND.value);
    }

    #[test]
    fn hooks_match_the_header() {
        let hw = struct_body("struct ce_twoplane_hw {");
        for m in HW_HOOKS {
            assert!(hw.contains(&*["(*", m.name, ")"].concat()), "{}", m.name);
        }
        assert_eq!(hw.matches("(*").count(), HW_HOOKS.len());
        let port = struct_body("struct ce_twoplane_port {");
        for m in PORT_MEMBERS {
            assert!(port.contains(&*["(*", m.name, ")"].concat()), "{}", m.name);
            assert!(!m.writes_planes);
        }
        // Function-pointer members; wake_arg and lookup_arg go with them.
        assert_eq!(port.matches("(*").count(), PORT_MEMBERS.len());
    }

    #[test]
    fn hooks_that_write_are_the_commits() {
        let writers = HW_HOOKS.iter().filter(|m| m.writes_planes).map(|m| m.name);
        assert!(writers.eq([
            "set_app_buffer",
            "enable_app_plane",
            "safety_flip",
            "safety_premultiplied"
        ]));
        // No row claims more than the design doc: none is observed (there is
        // no such tag) and the open ones are marked.
        assert_eq!(
            HW_HOOKS
                .iter()
                .find(|m| m.name == "set_vsync_callback")
                .unwrap()
                .tag,
            Tag::Open
        );
    }

    #[test]
    fn open_list_is_complete() {
        assert!(OPEN.contains(&"XRGB8888_DPU_FORMAT_OPEN"));
        assert_eq!(XRGB8888_DPU_FORMAT_OPEN, DpuFormat::Open(OPEN[0]));
        assert_eq!(SAFETY_FETCH_OPEN, Unit::Open(OPEN[1]));
        assert_eq!(VSYNC_SOURCE_OPEN, VsyncSource::Open(OPEN[4]));
        assert!(OPEN.contains(&PA_ADDRESS_SPACE_OPEN));
        assert!(OPEN.contains(&SHADOW_LOAD_TIMING_OPEN));
        for (i, a) in OPEN.iter().enumerate() {
            assert!(a.ends_with("_OPEN"));
            assert!(!OPEN[i + 1..].contains(a));
        }
    }
}
