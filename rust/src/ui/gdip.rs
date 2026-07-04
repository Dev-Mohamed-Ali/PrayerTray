//! Safe RAII wrappers over the GDI+ flat API — the same renderer the C# build uses
//! (System.Drawing is a thin layer over these calls), so painting ports 1:1 and
//! Arabic/Urdu shaping matches. All GDI+ unsafe lives here.

#![allow(clippy::too_many_arguments)]

use std::sync::Once;
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::Graphics::GdiPlus::*;

static INIT: Once = Once::new();

/// Process-wide GDI+ startup (never shut down; the OS reclaims at exit).
pub fn init() {
    INIT.call_once(|| unsafe {
        let mut token = 0usize;
        let input = GdiplusStartupInput {
            GdiplusVersion: 1,
            ..Default::default()
        };
        let mut output = GdiplusStartupOutput::default();
        let _ = GdiplusStartup(&mut token, &input, &mut output);
    });
}

pub struct Bitmap(pub *mut GpBitmap);

impl Bitmap {
    pub fn new(w: i32, h: i32) -> Self {
        init();
        let mut bmp: *mut GpBitmap = std::ptr::null_mut();
        unsafe {
            // PixelFormat32bppARGB = 0x26200A
            let _ = GdipCreateBitmapFromScan0(w.max(1), h.max(1), 0, 0x26200A, None, &mut bmp);
        }
        Self(bmp)
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { GdipDisposeImage(self.0 as *mut GpImage) };
        }
    }
}

pub struct Graphics(pub *mut GpGraphics);

impl Graphics {
    pub fn from_bitmap(bmp: &Bitmap) -> Self {
        let mut g: *mut GpGraphics = std::ptr::null_mut();
        unsafe {
            let _ = GdipGetImageGraphicsContext(bmp.0 as *mut GpImage, &mut g);
            let _ = GdipSetSmoothingMode(g, SmoothingModeAntiAlias);
            let _ = GdipSetTextRenderingHint(g, TextRenderingHintClearTypeGridFit);
        }
        Self(g)
    }

    pub fn from_hdc(hdc: HDC) -> Self {
        init();
        let mut g: *mut GpGraphics = std::ptr::null_mut();
        unsafe {
            let _ = GdipCreateFromHDC(hdc, &mut g);
        }
        Self(g)
    }

    pub fn clear(&self, color: u32) {
        unsafe {
            let _ = GdipGraphicsClear(self.0, color);
        }
    }

    pub fn fill_ellipse(&self, brush: &SolidBrush, x: f32, y: f32, w: f32, h: f32) {
        unsafe {
            let _ = GdipFillEllipse(self.0, brush.0 as *mut GpBrush, x, y, w, h);
        }
    }

    pub fn fill_rect(&self, brush: &SolidBrush, x: f32, y: f32, w: f32, h: f32) {
        unsafe {
            let _ = GdipFillRectangle(self.0, brush.0 as *mut GpBrush, x, y, w, h);
        }
    }

    pub fn draw_string(&self, s: &str, font: &Font, brush: &SolidBrush, rect: RectF, fmt: &StringFormat) {
        let wide: Vec<u16> = s.encode_utf16().collect();
        unsafe {
            let _ = GdipDrawString(
                self.0,
                windows::core::PCWSTR(wide.as_ptr()),
                wide.len() as i32,
                font.0,
                &rect,
                fmt.0,
                brush.0 as *mut GpBrush,
            );
        }
    }

    pub fn measure_string(&self, s: &str, font: &Font) -> (f32, f32) {
        let wide: Vec<u16> = s.encode_utf16().collect();
        let layout = RectF {
            X: 0.0,
            Y: 0.0,
            Width: 100_000.0,
            Height: 100_000.0,
        };
        let mut out = RectF::default();
        unsafe {
            let _ = GdipMeasureString(
                self.0,
                windows::core::PCWSTR(wide.as_ptr()),
                wide.len() as i32,
                font.0,
                &layout,
                std::ptr::null(),
                &mut out,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
        }
        (out.Width, out.Height)
    }

    /// Blit a bitmap 1:1 at (x, y) — DrawImageUnscaled equivalent.
    pub fn draw_bitmap(&self, bmp: &Bitmap, x: i32, y: i32) {
        unsafe {
            let mut w = 0.0f32;
            let mut h = 0.0f32;
            let _ = GdipGetImageDimension(bmp.0 as *mut GpImage, &mut w, &mut h);
            let _ = GdipDrawImageRectI(self.0, bmp.0 as *mut GpImage, x, y, w as i32, h as i32);
        }
    }
}

impl Drop for Graphics {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { GdipDeleteGraphics(self.0) };
        }
    }
}

pub struct SolidBrush(pub *mut GpSolidFill);

impl SolidBrush {
    pub fn new(argb: u32) -> Self {
        let mut b: *mut GpSolidFill = std::ptr::null_mut();
        unsafe {
            let _ = GdipCreateSolidFill(argb, &mut b);
        }
        Self(b)
    }
}

impl Drop for SolidBrush {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { GdipDeleteBrush(self.0 as *mut GpBrush) };
        }
    }
}

pub struct Pen(pub *mut GpPen);

impl Pen {
    pub fn new(argb: u32, width: f32) -> Self {
        init();
        let mut p: *mut GpPen = std::ptr::null_mut();
        unsafe {
            // Unit 2 = UnitPixel (matches C# `new Pen(color, width)`)
            let _ = GdipCreatePen1(argb, width, Unit(2), &mut p);
        }
        Self(p)
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { GdipDeletePen(self.0) };
        }
    }
}

impl Graphics {
    pub fn draw_line(&self, pen: &Pen, x1: f32, y1: f32, x2: f32, y2: f32) {
        unsafe {
            let _ = GdipDrawLine(self.0, pen.0, x1, y1, x2, y2);
        }
    }

    pub fn draw_ellipse(&self, pen: &Pen, x: f32, y: f32, w: f32, h: f32) {
        unsafe {
            let _ = GdipDrawEllipse(self.0, pen.0, x, y, w, h);
        }
    }

    /// Rounded-rect fill via a GraphicsPath of four arcs (port of the C# FillRounded helper).
    pub fn fill_rounded(&self, brush: &SolidBrush, r: RectF, radius: f32) {
        let d = radius * 2.0;
        unsafe {
            let mut path: *mut GpPath = std::ptr::null_mut();
            // FillModeAlternate = 0
            if GdipCreatePath(FillMode(0), &mut path) != Ok || path.is_null() {
                return;
            }
            let _ = GdipAddPathArc(path, r.X, r.Y, d, d, 180.0, 90.0);
            let _ = GdipAddPathArc(path, r.X + r.Width - d, r.Y, d, d, 270.0, 90.0);
            let _ = GdipAddPathArc(path, r.X + r.Width - d, r.Y + r.Height - d, d, d, 0.0, 90.0);
            let _ = GdipAddPathArc(path, r.X, r.Y + r.Height - d, d, d, 90.0, 90.0);
            let _ = GdipClosePathFigure(path);
            let _ = GdipFillPath(self.0, brush.0 as *mut GpBrush, path);
            let _ = GdipDeletePath(path);
        }
    }
}

pub const STYLE_REGULAR: i32 = 0;
pub const STYLE_BOLD: i32 = 1;

pub struct Font(pub *mut GpFont);

impl Font {
    /// Pixel-unit font like the C# `new Font(family, px, style, GraphicsUnit.Pixel)`;
    /// unknown family falls back to the generic sans-serif (Segoe UI-ish).
    pub fn new(family: &str, px: f32, style: i32) -> Self {
        init();
        let wide: Vec<u16> = family.encode_utf16().chain(std::iter::once(0)).collect();
        let mut fam: *mut GpFontFamily = std::ptr::null_mut();
        unsafe {
            let status =
                GdipCreateFontFamilyFromName(windows::core::PCWSTR(wide.as_ptr()), std::ptr::null_mut(), &mut fam);
            if status != Ok || fam.is_null() {
                let _ = GdipGetGenericFontFamilySansSerif(&mut fam);
            }
            let mut font: *mut GpFont = std::ptr::null_mut();
            // Unit 2 = UnitPixel
            let _ = GdipCreateFont(fam, px, style, Unit(2), &mut font);
            let _ = GdipDeleteFontFamily(fam);
            Self(font)
        }
    }

    /// Point-unit font like the C# `new Font(family, pt, style)` default.
    pub fn new_pt(family: &str, pt: f32, style: i32) -> Self {
        init();
        let wide: Vec<u16> = family.encode_utf16().chain(std::iter::once(0)).collect();
        let mut fam: *mut GpFontFamily = std::ptr::null_mut();
        unsafe {
            let status =
                GdipCreateFontFamilyFromName(windows::core::PCWSTR(wide.as_ptr()), std::ptr::null_mut(), &mut fam);
            if status != Ok || fam.is_null() {
                let _ = GdipGetGenericFontFamilySansSerif(&mut fam);
            }
            let mut font: *mut GpFont = std::ptr::null_mut();
            // Unit 3 = UnitPoint
            let _ = GdipCreateFont(fam, pt, style, Unit(3), &mut font);
            let _ = GdipDeleteFontFamily(fam);
            Self(font)
        }
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { GdipDeleteFont(self.0) };
        }
    }
}

pub fn rectf(x: f32, y: f32, w: f32, h: f32) -> RectF {
    RectF { X: x, Y: y, Width: w, Height: h }
}

pub const ALIGN_NEAR: StringAlignment = StringAlignmentNear;
pub const ALIGN_CENTER: StringAlignment = StringAlignmentCenter;
pub const ALIGN_FAR: StringAlignment = StringAlignmentFar;

pub struct StringFormat(pub *mut GpStringFormat);

impl StringFormat {
    pub fn new(align: StringAlignment, line_align: StringAlignment) -> Self {
        init();
        let mut f: *mut GpStringFormat = std::ptr::null_mut();
        unsafe {
            let _ = GdipCreateStringFormat(0, 0, &mut f);
            let _ = GdipSetStringFormatAlign(f, align);
            let _ = GdipSetStringFormatLineAlign(f, line_align);
        }
        Self(f)
    }
}

impl Drop for StringFormat {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { GdipDeleteStringFormat(self.0) };
        }
    }
}
