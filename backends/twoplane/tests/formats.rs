//! Pixel formats and pitch of the two-plane backend, through the public API
//! of the crate (`composition_twoplane::imx95`).
//!
//! Host reference only, not a board result. The backend itself is C and is
//! reachable from Rust only through the C programs of `tests/twoplane.rs`;
//! the Rust API is the i.MX 95 port's tables and its generic `set_app_buffer`
//! checks (`check_app_buffer`), which is what is tested here.
//!
//! Not testable from outside, recorded for the report: the C backend's own
//! plane-0 format list (`ce_twoplane.c`: RGB565, XRGB8888, ARGB8888, filtered
//! by the driver's `app_format_ok`) and the refusals of its `set_app_buffer`
//! hook are driver behaviour behind a function table; the Rust API has no
//! handle on them. `check_app_buffer` also does not check the order of
//! refusals against the C code; only the order documented in its source.

#![forbid(unsafe_code)]

use composition_twoplane::imx95::{
    app_format, app_format_ok, check_app_buffer, safety_stride, AppBuffer, Refusal, Tag,
    APP_FORMATS, CE_FORMAT_ARGB8888, CE_FORMAT_NV12, CE_FORMAT_RGB565, CE_FORMAT_XRGB8888,
    FB_ADDR_ALIGN, FB_PITCH_ALIGN,
};

const W: u32 = 800;
const H: u32 = 480;

fn round_up(v: u32, a: u32) -> u32 {
    v.div_ceil(a) * a
}

fn buf(format: u32, bpp: u32) -> AppBuffer {
    AppBuffer {
        pa: 0x8000_0000,
        width: W,
        height: H,
        stride_bytes: round_up(W * bpp, FB_PITCH_ALIGN.value),
        format,
    }
}

#[test]
fn format_table_bytes_per_pixel() {
    assert_eq!(APP_FORMATS.len(), 3);
    let bpp = |f| app_format(f).map(|r| r.bytes_per_pixel);
    assert_eq!(bpp(CE_FORMAT_RGB565), Some(2));
    assert_eq!(bpp(CE_FORMAT_XRGB8888), Some(4));
    assert_eq!(bpp(CE_FORMAT_ARGB8888), Some(4));
    assert_eq!(bpp(CE_FORMAT_NV12), None);
}

#[test]
fn settled_formats_are_accepted_with_their_pitch() {
    for f in APP_FORMATS.iter().filter(|f| f.dpu.tag != Tag::Open) {
        assert!(app_format_ok(f.ce_format));
        let b = buf(f.ce_format, f.bytes_per_pixel);
        assert_eq!(
            check_app_buffer(W, H, &b),
            Ok(()),
            "format {:#x}",
            f.ce_format
        );
        // The tightest accepted pitch is a row rounded up to the alignment.
        assert_eq!(b.stride_bytes % FB_PITCH_ALIGN.value, 0);
        assert!(b.stride_bytes >= W * f.bytes_per_pixel);
    }
}

#[test]
fn open_format_is_refused_as_format() {
    for f in APP_FORMATS.iter().filter(|f| f.dpu.tag == Tag::Open) {
        assert!(!app_format_ok(f.ce_format));
        assert_eq!(
            check_app_buffer(W, H, &buf(f.ce_format, f.bytes_per_pixel)),
            Err(Refusal::Format)
        );
    }
}

#[test]
fn unsupported_formats_are_refused() {
    for fmt in [CE_FORMAT_NV12, 0, u32::MAX, 0x3231_564e ^ 1] {
        assert!(!app_format_ok(fmt));
        assert_eq!(check_app_buffer(W, H, &buf(fmt, 4)), Err(Refusal::Format));
    }
}

#[test]
fn zero_and_short_pitch_are_refused() {
    for f in APP_FORMATS.iter().filter(|f| app_format_ok(f.ce_format)) {
        let mut b = buf(f.ce_format, f.bytes_per_pixel);
        b.stride_bytes = 0;
        assert_eq!(check_app_buffer(W, H, &b), Err(Refusal::StrideShort));
        // Aligned but one alignment step below a row of pixels.
        let row = W * f.bytes_per_pixel;
        b.stride_bytes = round_up(row, FB_PITCH_ALIGN.value) - FB_PITCH_ALIGN.value;
        if b.stride_bytes < row {
            assert_eq!(check_app_buffer(W, H, &b), Err(Refusal::StrideShort));
        }
    }
}

#[test]
fn unaligned_pitch_is_refused() {
    for f in APP_FORMATS.iter().filter(|f| app_format_ok(f.ce_format)) {
        let mut b = buf(f.ce_format, f.bytes_per_pixel);
        for delta in [1, 4, FB_PITCH_ALIGN.value - 1] {
            b.stride_bytes = buf(f.ce_format, f.bytes_per_pixel).stride_bytes + delta;
            assert_eq!(check_app_buffer(W, H, &b), Err(Refusal::StrideAlign));
        }
        // A larger aligned pitch is fine.
        b.stride_bytes = buf(f.ce_format, f.bytes_per_pixel).stride_bytes + FB_PITCH_ALIGN.value;
        assert_eq!(check_app_buffer(W, H, &b), Ok(()));
    }
}

#[test]
fn misaligned_and_wide_addresses_are_refused() {
    let f = APP_FORMATS
        .iter()
        .find(|f| app_format_ok(f.ce_format))
        .expect("a settled format");
    let mut b = buf(f.ce_format, f.bytes_per_pixel);
    for off in [1u64, 4, u64::from(FB_ADDR_ALIGN.value) - 1] {
        b.pa = 0x8000_0000 + off;
        assert_eq!(check_app_buffer(W, H, &b), Err(Refusal::AddrAlign));
    }
    b.pa = 0x1_0000_0000;
    assert_eq!(check_app_buffer(W, H, &b), Err(Refusal::AddrWidth));
}

#[test]
fn wrong_size_is_refused() {
    let f = &APP_FORMATS[0];
    let mut b = buf(f.ce_format, f.bytes_per_pixel);
    b.width = W - 1;
    assert_eq!(check_app_buffer(W, H, &b), Err(Refusal::Size));
    b.width = W;
    b.height = H + 1;
    assert_eq!(check_app_buffer(W, H, &b), Err(Refusal::Size));
}

#[test]
fn safety_stride_is_rounded_argb8888_row() {
    assert_eq!(
        safety_stride(W),
        Some(round_up(W * 4, FB_PITCH_ALIGN.value))
    );
    assert_eq!(safety_stride(1), Some(FB_PITCH_ALIGN.value));
    assert_eq!(safety_stride(0), Some(0));
    assert_eq!(safety_stride(u32::MAX), None);
    for w in [1, 7, 8, 9, 640, 1023] {
        let s = safety_stride(w).unwrap();
        assert_eq!(s % FB_PITCH_ALIGN.value, 0);
        assert!(s >= w * 4 && s < w * 4 + FB_PITCH_ALIGN.value);
    }
}
