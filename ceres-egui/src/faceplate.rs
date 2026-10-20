//! The PC-1500's faceplate painted procedurally: the emulator's main view. Case, brushed aluminium
//! panel, bronze top band (the original PC-1500's; the PC-1500A had it black), the LCD inside its
//! frame (painted by [`lcd_view::paint`]), the strip of the function keys and the 65 keys in their
//! real layout, with their colors and the `SHIFT` labels printed on the panel.
//!
//! Everything is drawn with egui primitives: no image is loaded and there are no new dependencies,
//! and no photo or logo is reproduced (the brand is written as the generic text "PC-1500"). The
//! only two textures are generated right here, only once: the grain of the brushed aluminium
//! ([`brushed_pixels`]) and the gradient of the band ([`band_gradient_pixels`]), which as a texture
//! keeps the rounded corners.
//!
//! # Geometry
//! All the geometry is in a reference space of [`REF_W`]×[`REF_H`] units: that of a photo of the
//! real unit (195 × 86 mm) on which the positions of the keys were measured. When painting,
//! [`Layout::fit`] scales that space to the room available keeping the proportions, and centers it.
//! The geometry functions ([`cap_rect`], [`CAPS`], [`Layout`]) are pure and have tests: each key
//! has exactly one cap, they do not overlap and they fit in the panel.
//!
//! # Interaction
//! Each cap is an egui area (`Ui::interact`, without keyboard focus). [`Faceplate::show`] only
//! returns which key the mouse button is pressing now ([`FaceplateOutput::pointer_key`]); the
//! caller turns the changes into presses and releases of the core's key feeder
//! (`host_key_down`/`host_key_up` with `MOUSE_ID`, see `Pc1500App::update_mouse_key`). NO minimum
//! key press in host time is imposed here: the feeder already guarantees in emulated time that the
//! ROM sees each key, at any speed.

use ceres_core::Key;
use eframe::egui::{
    self, Align2, Color32, ColorImage, CornerRadius, FontId, Id, Mesh, Painter, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, TextureHandle, TextureId, TextureOptions, Ui, Vec2, epaint::RectShape, pos2, vec2,
};

use crate::lcd_view::{self, LcdFrame, LcdGeometry, STN_GREY};

/// Width and height of the reference space (see the module's documentation).
pub const REF_W: f32 = 679.0;
pub const REF_H: f32 = 299.0;

// ---------------------------------------------------------------------
// Colors (taken from photos of the original PC-1500).
// ---------------------------------------------------------------------

/// Edge of the case (almost black grey).
const CASE_EDGE: Color32 = Color32::from_rgb(0x2A, 0x2C, 0x2E);
/// Shadow of the case on the window's background.
const CASE_SHADOW: Color32 = Color32::from_rgba_unmultiplied_const(0, 0, 0, 70);
/// Highlight and shadow of the edges of the aluminium panel.
const PANEL_HIGHLIGHT: Color32 = Color32::from_rgb(0xE2, 0xE4, 0xE3);
const PANEL_SHADOW: Color32 = Color32::from_rgb(0xB4, 0xB8, 0xB8);
/// Dark lip on the bottom edge of the panel.
const PANEL_LIP: Color32 = Color32::from_rgb(0x8E, 0x92, 0x92);
/// Text printed on the panel (`POWER`, `BREAK`, labels of the strip).
const PANEL_TEXT: Color32 = Color32::from_rgb(0x2B, 0x2D, 0x2F);
/// `SHIFT` labels printed above the keys.
const LEGEND_ORANGE: Color32 = Color32::from_rgb(0xC8, 0x69, 0x1E);

/// Bronze top band, at the top and bottom of the gradient.
const TOP_BAND_HI: Color32 = Color32::from_rgb(0x57, 0x4E, 0x3E);
const TOP_BAND_LO: Color32 = Color32::from_rgb(0x36, 0x30, 0x2A);
/// Thin line of the band and its text.
const TOP_BAND_RULE: Color32 = Color32::from_rgb(0x7A, 0x71, 0x5F);
const TOP_BAND_TEXT: Color32 = Color32::from_rgb(0xD9, 0xD6, 0xCC);
/// Brand and subtitle of the band.
const BRAND_TEXT: Color32 = Color32::from_rgb(0xDA, 0xDD, 0xDC);
const BRAND_SUBTEXT: Color32 = Color32::from_rgb(0xD0, 0xD3, 0xD2);

/// Silver frame of the LCD and the dark background between frame and glass.
const LCD_FRAME: Color32 = Color32::from_rgb(0xB9, 0xBD, 0xBB);
const LCD_BEZEL: Color32 = Color32::from_rgb(0x22, 0x1F, 0x1B);
/// Notice of the machine being off, written faintly on the glass.
const LCD_OFF_TEXT: Color32 = Color32::from_rgba_unmultiplied_const(0x1F, 0x26, 0x28, 130);

/// Strip of the function keys and its separators.
const STRIP_FILL: Color32 = Color32::from_rgb(0xB4, 0xB8, 0xB7);
const STRIP_DIVIDER: Color32 = Color32::from_rgb(0x9A, 0x9E, 0x9D);

/// Outline of a latched prefix key (`SHIFT`, `SML`, `DEF` waiting for the next key).
const LATCH_GLOW: Color32 = Color32::from_rgb(0xFF, 0xE2, 0x7A);

// ---------------------------------------------------------------------
// Reference geometry.
// ---------------------------------------------------------------------

/// Whole case and aluminium panel.
const BODY: Rect = ref_rect(0.0, 0.0, REF_W, REF_H);
const BODY_RADIUS: f32 = 12.0;
const PANEL: Rect = ref_rect(4.0, 4.0, 675.0, 293.0);
const PANEL_RADIUS: f32 = 9.0;
/// Height of the panel's dark lip.
const PANEL_LIP_Y: f32 = 292.0;

/// Top band, its line and the text on the left.
const TOP_BAND: Rect = ref_rect(8.0, 8.0, 671.0, 92.0);
const TOP_BAND_RADIUS: f32 = 4.0;
const TOP_BAND_RULE_Y: f32 = 22.0;
const TOP_BAND_RULE_X: (f32, f32) = (14.0, 665.0);
const TOP_BAND_TEXT_POS: Pos2 = pos2(66.0, 16.0);

/// Frame of the LCD, glass, and origin and pitch of the 156×7 matrix (the matrix takes 93..461 ×
/// 61..77.5 and the band of indicators 53..59).
const LCD_FRAME_RECT: Rect = ref_rect(76.0, 38.0, 486.0, 86.0);
const LCD_GLASS: Rect = ref_rect(79.0, 41.0, 483.0, 83.0);
const LCD_MATRIX_ORIGIN: Pos2 = pos2(93.0, 61.0);
const LCD_PITCH: f32 = 2.36;

/// Brand: model name and subtitle, centered to the right of the LCD.
const BRAND_X: f32 = 592.0;
const BRAND_Y: f32 = 50.0;
const BRAND_SUB_Y: f32 = 70.0;

/// Strip of the function keys (F1..F6), with separators between keys and the `SHIFT` labels
/// (`! " # $ % &`) at the top.
const STRIP: Rect = ref_rect(101.0, 95.0, 440.0, 139.0);
const STRIP_DIVIDERS: [f32; 5] = [158.0, 214.5, 271.0, 327.5, 383.5];
const STRIP_LABEL_Y: f32 = 105.0;

/// Power group: `POWER` with its line under `OFF`/`ON` and `BREAK` above `ON`.
const POWER_LABEL: Pos2 = pos2(548.0, 131.0);
const POWER_LINE_Y: f32 = 139.0;
const POWER_LINE_X: (f32, f32) = (532.0, 652.0);
const BREAK_LABEL: Pos2 = pos2(635.5, 113.0);

/// Distance, above the center of the key, at which the `SHIFT` label printed on the panel goes.
const SHIFT_LEGEND_DY: f32 = 15.0;

/// Downward offset of the key's edge (its shadow) and how far the face sinks when it is pressed.
const CAP_RIM_DROP: f32 = 1.5;
const CAP_PRESS_DROP: f32 = 1.0;
const CAP_RIM_RADIUS: f32 = 2.5;
const CAP_FACE_RADIUS: f32 = 2.0;

/// Sizes of the texts, in reference units.
const FONT_PANEL: f32 = 5.5;
const FONT_STRIP: f32 = 7.0;
const FONT_SHIFT_LEGEND: f32 = 6.0;
const FONT_CAP_CHAR: f32 = 10.0;
const FONT_CAP_WORD: f32 = 6.5;
const FONT_BRAND: f32 = 15.0;
const FONT_BRAND_SUB: f32 = 6.5;
const FONT_LCD_OFF: f32 = 7.0;

/// Reference rectangle from its corners (`const`, unlike `Rect::from_min_max`).
const fn ref_rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect { min: pos2(x0, y0), max: pos2(x1, y1) }
}

/// Height (center) of each row of keys.
const ROW_TOP: f32 = 128.0;
const ROW_Q: f32 = 166.5;
const ROW_A: f32 = 199.5;
const ROW_Z: f32 = 233.0;
const ROW_BOTTOM: f32 = 266.0;

/// Sizes of caps: the normal one, that of the top row (lower) and the wide ones (`SPACE`, `ENTER`).
const SIZE_STD: Vec2 = vec2(31.0, 20.0);
const SIZE_TOP: Vec2 = vec2(26.0, 14.0);
const SIZE_WIDE: Vec2 = vec2(73.0, 20.0);

/// A key of the faceplate: its center and size in reference units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapDef {
    pub key: Key,
    pub center: Pos2,
    pub size: Vec2,
}

impl CapDef {
    /// Rectangle of the key's face (without the edge), in reference units.
    #[must_use]
    pub fn rect(&self) -> Rect {
        Rect::from_center_size(self.center, self.size)
    }
}

const fn cap(key: Key, cx: f32, cy: f32, size: Vec2) -> CapDef {
    CapDef { key, center: pos2(cx, cy), size }
}

/// The PC-1500's 65 keys in their place: the 64 of the matrix and `ON/BRK`. `DEF` is
/// `Key::Control`. Measured on the photo of the real unit (centers of the caps).
pub const CAPS: [CapDef; Key::COUNT] = {
    use Key as K;
    [
        // Top row: DEF, F1..F6 (in the strip), SHIFT, OFF, ON.
        cap(K::Control, 43.0, ROW_TOP, SIZE_TOP),
        cap(K::F1, 129.5, ROW_TOP, SIZE_TOP),
        cap(K::F2, 186.5, ROW_TOP, SIZE_TOP),
        cap(K::F3, 242.5, ROW_TOP, SIZE_TOP),
        cap(K::F4, 299.5, ROW_TOP, SIZE_TOP),
        cap(K::F5, 355.5, ROW_TOP, SIZE_TOP),
        cap(K::F6, 411.5, ROW_TOP, SIZE_TOP),
        cap(K::Shift, 506.5, ROW_TOP, SIZE_TOP),
        cap(K::Off, 593.5, ROW_TOP, SIZE_TOP),
        cap(K::On, 635.5, ROW_TOP, SIZE_TOP),
        // Fila Q.
        cap(K::Q, 43.5, ROW_Q, SIZE_STD),
        cap(K::W, 85.5, ROW_Q, SIZE_STD),
        cap(K::E, 127.5, ROW_Q, SIZE_STD),
        cap(K::R, 170.5, ROW_Q, SIZE_STD),
        cap(K::T, 212.5, ROW_Q, SIZE_STD),
        cap(K::Y, 255.5, ROW_Q, SIZE_STD),
        cap(K::U, 296.5, ROW_Q, SIZE_STD),
        cap(K::I, 339.5, ROW_Q, SIZE_STD),
        cap(K::O, 380.5, ROW_Q, SIZE_STD),
        cap(K::P, 423.5, ROW_Q, SIZE_STD),
        cap(K::Seven, 465.5, ROW_Q, SIZE_STD),
        cap(K::Eight, 508.5, ROW_Q, SIZE_STD),
        cap(K::Nine, 551.5, ROW_Q, SIZE_STD),
        cap(K::Slash, 592.5, ROW_Q, SIZE_STD),
        cap(K::Cl, 635.5, ROW_Q, SIZE_STD),
        // Fila A.
        cap(K::A, 54.5, ROW_A, SIZE_STD),
        cap(K::S, 95.5, ROW_A, SIZE_STD),
        cap(K::D, 138.5, ROW_A, SIZE_STD),
        cap(K::F, 180.5, ROW_A, SIZE_STD),
        cap(K::G, 222.5, ROW_A, SIZE_STD),
        cap(K::H, 264.5, ROW_A, SIZE_STD),
        cap(K::J, 307.5, ROW_A, SIZE_STD),
        cap(K::K, 348.5, ROW_A, SIZE_STD),
        cap(K::L, 390.5, ROW_A, SIZE_STD),
        cap(K::Four, 466.5, ROW_A, SIZE_STD),
        cap(K::Five, 508.5, ROW_A, SIZE_STD),
        cap(K::Six, 551.5, ROW_A, SIZE_STD),
        cap(K::Asterisk, 594.5, ROW_A, SIZE_STD),
        cap(K::Mode, 636.5, ROW_A, SIZE_STD),
        // Fila Z.
        cap(K::Z, 76.5, ROW_Z, SIZE_STD),
        cap(K::X, 117.5, ROW_Z, SIZE_STD),
        cap(K::C, 159.5, ROW_Z, SIZE_STD),
        cap(K::V, 200.5, ROW_Z, SIZE_STD),
        cap(K::B, 243.5, ROW_Z, SIZE_STD),
        cap(K::N, 287.5, ROW_Z, SIZE_STD),
        cap(K::M, 327.5, ROW_Z, SIZE_STD),
        cap(K::LeftParen, 371.5, ROW_Z, SIZE_STD),
        cap(K::RightParen, 413.5, ROW_Z, SIZE_STD),
        cap(K::One, 465.5, ROW_Z, SIZE_STD),
        cap(K::Two, 508.5, ROW_Z, SIZE_STD),
        cap(K::Three, 551.5, ROW_Z, SIZE_STD),
        cap(K::Minus, 592.5, ROW_Z, SIZE_STD),
        cap(K::Left, 634.5, ROW_Z, SIZE_STD),
        // Bottom row.
        cap(K::Sml, 75.5, ROW_BOTTOM, SIZE_STD),
        cap(K::Rsv, 117.5, ROW_BOTTOM, SIZE_STD),
        cap(K::Rcl, 157.5, ROW_BOTTOM, SIZE_STD),
        cap(K::Space, 224.0, ROW_BOTTOM, SIZE_WIDE),
        cap(K::Down, 286.5, ROW_BOTTOM, SIZE_STD),
        cap(K::Up, 328.5, ROW_BOTTOM, SIZE_STD),
        cap(K::Enter, 392.0, ROW_BOTTOM, SIZE_WIDE),
        cap(K::Zero, 466.5, ROW_BOTTOM, SIZE_STD),
        cap(K::Dot, 508.5, ROW_BOTTOM, SIZE_STD),
        cap(K::Equals, 550.5, ROW_BOTTOM, SIZE_STD),
        cap(K::Plus, 592.5, ROW_BOTTOM, SIZE_STD),
        cap(K::Right, 635.5, ROW_BOTTOM, SIZE_STD),
    ]
};

/// The face of the key `key` in reference units (see [`CAPS`]). Every key has one; `None` should
/// never happen (the tests check it).
#[must_use]
pub fn cap_rect(key: Key) -> Option<Rect> {
    CAPS.iter().find(|c| c.key == key).map(CapDef::rect)
}

/// What a key takes including its edge (which shows below it).
fn cap_outline(rect: Rect) -> Rect {
    Rect::from_min_max(rect.min, rect.max + vec2(0.0, CAP_RIM_DROP))
}

// ---------------------------------------------------------------------
// Look of the keys.
// ---------------------------------------------------------------------

/// The PC-1500's four kinds of cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapStyle {
    /// Letters, parentheses and `SPACE`: almost black caps, white label.
    Dark,
    /// Functions, numbers, operators and arrows: grey, black label.
    Grey,
    /// `CL`: roja.
    Red,
    /// `SHIFT`: naranja.
    Orange,
}

/// Colors of a kind of cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CapColors {
    face: Color32,
    rim: Color32,
    highlight: Color32,
    legend: Color32,
}

impl CapStyle {
    const fn colors(self) -> CapColors {
        match self {
            Self::Dark => CapColors {
                face: Color32::from_rgb(0x2D, 0x2B, 0x29),
                rim: Color32::from_rgb(0x12, 0x11, 0x10),
                highlight: Color32::from_rgb(0x4A, 0x47, 0x44),
                legend: Color32::from_rgb(0xF4, 0xF4, 0xF2),
            },
            Self::Grey => CapColors {
                face: Color32::from_rgb(0x9A, 0x9E, 0xA2),
                rim: Color32::from_rgb(0x3A, 0x3D, 0x40),
                highlight: Color32::from_rgb(0xB9, 0xBD, 0xC0),
                legend: Color32::from_rgb(0x15, 0x16, 0x17),
            },
            Self::Red => CapColors {
                face: Color32::from_rgb(0xC0, 0x26, 0x1F),
                rim: Color32::from_rgb(0x5E, 0x12, 0x0F),
                highlight: Color32::from_rgb(0xD9, 0x5A, 0x52),
                legend: Color32::WHITE,
            },
            Self::Orange => CapColors {
                face: Color32::from_rgb(0xE0, 0x8A, 0x22),
                rim: Color32::from_rgb(0x6B, 0x3E, 0x0C),
                highlight: Color32::from_rgb(0xF0, 0xB0, 0x60),
                legend: Color32::from_rgb(0x2A, 0x1A, 0x08),
            },
        }
    }
}

/// The kind of cap of each key, as in the photos of the machine.
#[must_use]
pub const fn cap_style(key: Key) -> CapStyle {
    match key {
        Key::Cl => CapStyle::Red,
        Key::Shift => CapStyle::Orange,
        Key::LeftParen | Key::RightParen | Key::Space => CapStyle::Dark,
        k if k.is_letter() => CapStyle::Dark,
        _ => CapStyle::Grey,
    }
}

/// Direction of a drawn arrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dir {
    Left,
    Right,
    Up,
    Down,
}

impl Dir {
    const fn unit(self) -> Vec2 {
        match self {
            Self::Left => vec2(-1.0, 0.0),
            Self::Right => vec2(1.0, 0.0),
            Self::Up => vec2(0.0, -1.0),
            Self::Down => vec2(0.0, 1.0),
        }
    }
}

/// The label printed on a cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Legend {
    /// A single character (letter, number, operator), large.
    Char(&'static str),
    /// A word (`DEF`, `SHIFT`, `ENTER`...), small and bold.
    Word(&'static str),
    /// An arrow, drawn with polygons (without depending on the font).
    Arrow(Dir),
    /// The double arrow `⇕` of `RSV`, also drawn.
    UpDown,
    /// No label: F1..F6 have theirs on the strip above.
    Blank,
}

/// The label of each key.
const fn legend(key: Key) -> Legend {
    match key {
        Key::F1 | Key::F2 | Key::F3 | Key::F4 | Key::F5 | Key::F6 => Legend::Blank,
        Key::Left => Legend::Arrow(Dir::Left),
        Key::Right => Legend::Arrow(Dir::Right),
        Key::Up => Legend::Arrow(Dir::Up),
        Key::Down => Legend::Arrow(Dir::Down),
        Key::Rsv => Legend::UpDown,
        // `Key::label` says "ON/BRK"; the cap only says ON (BREAK is printed above, on the panel).
        Key::On => Legend::Word("ON"),
        Key::Space | Key::Enter | Key::Cl | Key::Control | Key::Mode | Key::Off | Key::Rcl | Key::Shift | Key::Sml => {
            Legend::Word(key.label())
        }
        _ => Legend::Char(key.label()),
    }
}

/// A `SHIFT` label printed on the panel above a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanelLegend {
    Text(&'static str),
    /// `√`, drawn with segments (the default font may not have it).
    Sqrt,
    /// `π`, also drawn.
    Pi,
}

/// The `SHIFT` labels printed on the panel (those of F1..F6 go on the strip, see
/// [`STRIP_LABEL_Y`]). The text ones match `Key::shift_label` (the tests check it). Above ↑ and ↓
/// the case has √ and π: they are the codes $5B and $5D of `KB_LAYOUT_SH`, which from the PC's
/// keyboard are typed as `[` and `]`. `RESERVE` (`SHIFT`+`MODE`) is not printed on the case.
const SHIFT_LEGENDS: [(Key, PanelLegend); 13] = [
    (Key::Slash, PanelLegend::Text("?")),
    (Key::Cl, PanelLegend::Text("CA")),
    (Key::Asterisk, PanelLegend::Text(":")),
    (Key::Minus, PanelLegend::Text(",")),
    (Key::Left, PanelLegend::Text("DEL")),
    (Key::Equals, PanelLegend::Text("@")),
    (Key::Plus, PanelLegend::Text(";")),
    (Key::Right, PanelLegend::Text("INS")),
    (Key::LeftParen, PanelLegend::Text("<")),
    (Key::RightParen, PanelLegend::Text(">")),
    (Key::Space, PanelLegend::Text("^")),
    (Key::Down, PanelLegend::Pi),
    (Key::Up, PanelLegend::Sqrt),
];

/// The function keys, in the order of the strip.
const FUNCTION_KEYS: [Key; 6] = [Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6];

// ---------------------------------------------------------------------
// Escalado.
// ---------------------------------------------------------------------

/// How the reference space is placed on the screen: origin (the top left corner of the case) and
/// scale (egui points per reference unit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub origin: Pos2,
    pub scale: f32,
}

impl Layout {
    /// The case as large as possible inside `avail`, without distorting it, and centered.
    #[must_use]
    pub fn fit(avail: Rect) -> Self {
        let scale = (avail.width() / REF_W).min(avail.height() / REF_H).max(0.0);
        let size = vec2(REF_W, REF_H) * scale;
        Self { origin: avail.center() - size / 2.0, scale }
    }

    /// A reference point on the screen.
    #[must_use]
    pub fn pos(self, p: Pos2) -> Pos2 {
        self.origin + p.to_vec2() * self.scale
    }

    /// A reference rectangle on the screen.
    #[must_use]
    pub fn rect(self, r: Rect) -> Rect {
        Rect::from_min_max(self.pos(r.min), self.pos(r.max))
    }

    /// A reference length in points.
    #[must_use]
    pub fn len(self, v: f32) -> f32 {
        v * self.scale
    }

    /// A reference offset in points.
    #[must_use]
    pub fn vec(self, v: Vec2) -> Vec2 {
        v * self.scale
    }

    fn font(self, size: f32) -> FontId {
        FontId::proportional(self.len(size))
    }
}

// ---------------------------------------------------------------------
// Texturas procedurales.
// ---------------------------------------------------------------------

/// Size of the texture of the brushed aluminium.
pub const BRUSHED_W: usize = 512;
pub const BRUSHED_H: usize = 128;

/// Linear congruential generator (Knuth's, MMIX): deterministic, so the faceplate is always the
/// same.
struct Lcg(u64);

impl Lcg {
    const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    /// A number in `0..n`, with the high bits (the good ones of an LCG) and without a modulo.
    fn below(&mut self, n: u32) -> u32 {
        let high = self.next() >> 32;
        u32::try_from((high * u64::from(n)) >> 32).unwrap_or(0)
    }
}

/// Pixels (row by row) of the brushed aluminium, [`BRUSHED_W`] × [`BRUSHED_H`]: each row is a
/// horizontal grain with its own grey (200..214), modulated along the row by a slow wave of ±3 with
/// a random phase, a grain of ±1 per pixel and a warm touch (+2 in red). It is stretched over the
/// whole panel.
#[must_use]
pub fn brushed_pixels() -> Vec<Color32> {
    let mut rng = Lcg(0x5043_2D31_3530_3021);
    let width = u32::try_from(BRUSHED_W).unwrap_or(u32::MAX);
    let mut pixels = Vec::with_capacity(BRUSHED_W * BRUSHED_H);
    for _ in 0..BRUSHED_H {
        let grey = 200 + i32::try_from(rng.below(15)).unwrap_or(0);
        let phase = rng.below(width);
        for x in 0..width {
            // Triangle wave of period `width`: 0..=256..0 → -3..=3.
            let t = i32::try_from((x + phase) & (width - 1)).unwrap_or(0);
            let wave = ((t - 256).abs() * 6) / 256 - 3;
            let grain = i32::try_from(rng.below(3)).unwrap_or(1) - 1;
            let v = grey + wave + grain;
            pixels.push(Color32::from_rgb(channel(v + 2), channel(v), channel(v)));
        }
    }
    pixels
}

/// Pixels (a column of [`BAND_STEPS`]) of the vertical gradient of the top band, from
/// [`TOP_BAND_HI`] to [`TOP_BAND_LO`].
#[must_use]
pub fn band_gradient_pixels() -> Vec<Color32> {
    let last = i32::try_from(BAND_STEPS - 1).unwrap_or(1);
    (0..=last)
        .map(|i| {
            let mix = |hi: u8, lo: u8| channel((i32::from(hi) * (last - i) + i32::from(lo) * i) / last);
            Color32::from_rgb(
                mix(TOP_BAND_HI.r(), TOP_BAND_LO.r()),
                mix(TOP_BAND_HI.g(), TOP_BAND_LO.g()),
                mix(TOP_BAND_HI.b(), TOP_BAND_LO.b()),
            )
        })
        .collect()
}

/// Height of the texture of the band's gradient.
pub const BAND_STEPS: usize = 64;

/// A color channel clamped to 0..=255.
fn channel(v: i32) -> u8 {
    u8::try_from(v.clamp(0, 255)).unwrap_or(u8::MAX)
}

// ---------------------------------------------------------------------
// Pintado.
// ---------------------------------------------------------------------

/// What the faceplate needs to paint itself in a repaint.
pub struct FaceplateInput<'a> {
    /// What the LCD shows.
    pub lcd: &'a LcdFrame,
    /// Ghosts of the LCD (see `lcd_view::paint`).
    pub ghost: bool,
    /// The machine is off: a faint notice is written on the glass.
    pub powered_off: Option<&'a str>,
    /// Keys that show pressed (down in the emulated keyboard, with the mouse or with the PC's
    /// keyboard), indexed by `Key as usize`.
    pub held: &'a [bool; Key::COUNT],
    /// Latched prefixes (`SHIFT`, `SML`, `DEF`), with an outline.
    pub latched: &'a [bool; Key::COUNT],
    /// Help text of each key (which PC key types it).
    pub hint: &'a dyn Fn(Key) -> Option<String>,
}

/// Result of [`Faceplate::show`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FaceplateOutput {
    /// The key the mouse button is pressing now (it is kept even if the pointer leaves it without
    /// releasing).
    pub pointer_key: Option<Key>,
}

/// The faceplate, with its textures generated the first time it is painted.
#[derive(Default)]
pub struct Faceplate {
    brushed: Option<TextureHandle>,
    band: Option<TextureHandle>,
}

impl Faceplate {
    /// Paints the faceplate taking all the free room of `ui` and returns which key the mouse has
    /// pressed.
    pub fn show(&mut self, ui: &mut Ui, input: &FaceplateInput<'_>) -> FaceplateOutput {
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let layout = Layout::fit(rect);
        if layout.scale < 0.05 {
            return FaceplateOutput::default();
        }
        let (brushed, band) = self.textures(ui.ctx());
        let painter = ui.painter_at(rect);

        paint_body(&painter, layout, brushed);
        paint_top_band(&painter, layout, band);
        paint_lcd(&painter, layout, input);
        paint_panel_text(&painter, layout);

        let mut output = FaceplateOutput::default();
        for cap in &CAPS {
            let key = cap.key;
            let screen = layout.rect(cap_outline(cap.rect()));
            let response = ui
                .interact(screen, Id::new(("pc1500key", key as u8)), Sense::CLICK)
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let down = response.is_pointer_button_down_on();
            if down {
                output.pointer_key = Some(key);
            }
            let state = CapState {
                pressed: down || input.held[key as usize],
                latched: input.latched[key as usize],
                hovered: response.hovered() && !down,
            };
            paint_cap(&painter, layout, cap, state);
            if state.hovered
                && let Some(text) = (input.hint)(key)
            {
                response.on_hover_text(text);
            }
        }
        output
    }

    /// The two procedural textures, created only once.
    fn textures(&mut self, ctx: &egui::Context) -> (TextureId, TextureId) {
        let brushed = self
            .brushed
            .get_or_insert_with(|| {
                let image = ColorImage::new([BRUSHED_W, BRUSHED_H], brushed_pixels());
                ctx.load_texture("pc1500-brushed", image, TextureOptions::LINEAR)
            })
            .id();
        let band = self
            .band
            .get_or_insert_with(|| {
                let image = ColorImage::new([1, BAND_STEPS], band_gradient_pixels());
                ctx.load_texture("pc1500-band", image, TextureOptions::LINEAR)
            })
            .id();
        (brushed, band)
    }
}

/// UV of a whole texture.
const FULL_UV: Rect = ref_rect(0.0, 0.0, 1.0, 1.0);

/// Corner radius in points (egui keeps them in `u8`).
fn radius(layout: Layout, r: f32) -> CornerRadius {
    CornerRadius::from(layout.len(r))
}

/// Case with its shadow, brushed aluminium panel with a highlight at the top, a shadow at the
/// bottom and the dark lip of the bottom edge.
fn paint_body(painter: &Painter, layout: Layout, brushed: TextureId) {
    let body = layout.rect(BODY);
    painter.add(
        RectShape::filled(body.translate(vec2(0.0, layout.len(3.0))), radius(layout, BODY_RADIUS), CASE_SHADOW)
            .with_blur_width(layout.len(6.0)),
    );
    painter.rect_filled(body, radius(layout, BODY_RADIUS), CASE_EDGE);

    let panel = layout.rect(PANEL);
    painter.add(RectShape::filled(panel, radius(layout, PANEL_RADIUS), Color32::WHITE).with_texture(brushed, FULL_UV));
    let inset = layout.len(PANEL_RADIUS);
    let hairline = layout.len(1.0).max(1.0);
    painter.line_segment(
        [pos2(panel.left() + inset, panel.top() + hairline / 2.0), pos2(panel.right() - inset, panel.top() + hairline / 2.0)],
        Stroke::new(hairline, PANEL_HIGHLIGHT),
    );
    let lip = layout.pos(pos2(0.0, PANEL_LIP_Y)).y;
    painter.line_segment(
        [pos2(panel.left() + inset, lip - hairline), pos2(panel.right() - inset, lip - hairline)],
        Stroke::new(hairline, PANEL_SHADOW),
    );
    painter.line_segment(
        [pos2(panel.left() + inset, lip), pos2(panel.right() - inset, lip)],
        Stroke::new(hairline, PANEL_LIP),
    );
}

/// Bronze top band (a gradient as a texture, to keep the rounded corners), its line and the text on
/// the left.
fn paint_top_band(painter: &Painter, layout: Layout, band: TextureId) {
    let rect = layout.rect(TOP_BAND);
    painter.add(RectShape::filled(rect, radius(layout, TOP_BAND_RADIUS), Color32::WHITE).with_texture(band, FULL_UV));
    let y = layout.pos(pos2(0.0, TOP_BAND_RULE_Y)).y;
    painter.line_segment(
        [pos2(layout.pos(pos2(TOP_BAND_RULE_X.0, 0.0)).x, y), pos2(layout.pos(pos2(TOP_BAND_RULE_X.1, 0.0)).x, y)],
        Stroke::new(layout.len(0.6).max(1.0), TOP_BAND_RULE),
    );
    painter.text(
        layout.pos(TOP_BAND_TEXT_POS),
        Align2::LEFT_CENTER,
        "MEMORY SAFE GUARD / AUTO POWER OFF",
        layout.font(FONT_PANEL),
        TOP_BAND_TEXT,
    );
    bold_text(painter, layout, pos2(BRAND_X, BRAND_Y), "PC-1500", FONT_BRAND, BRAND_TEXT);
    painter.text(
        layout.pos(pos2(BRAND_X, BRAND_SUB_Y)),
        Align2::CENTER_CENTER,
        "POCKET COMPUTER",
        layout.font(FONT_BRAND_SUB),
        BRAND_SUBTEXT,
    );
}

/// Geometry of the faceplate's LCD on the screen.
fn lcd_geometry(layout: Layout) -> LcdGeometry {
    LcdGeometry {
        glass: layout.rect(LCD_GLASS),
        matrix_origin: layout.pos(LCD_MATRIX_ORIGIN),
        pitch: layout.len(LCD_PITCH),
    }
}

/// Silver frame, glass and contents of the LCD (and the notice of being off).
fn paint_lcd(painter: &Painter, layout: Layout, input: &FaceplateInput<'_>) {
    let frame = layout.rect(LCD_FRAME_RECT);
    painter.rect_filled(frame, CornerRadius::ZERO, LCD_BEZEL);
    painter.rect_stroke(frame, CornerRadius::ZERO, Stroke::new(layout.len(2.0), LCD_FRAME), StrokeKind::Inside);
    let geometry = lcd_geometry(layout);
    lcd_view::paint(painter, &geometry, input.lcd, &STN_GREY, input.ghost);
    if let Some(text) = input.powered_off {
        painter.text(geometry.glass.center(), Align2::CENTER_CENTER, text, layout.font(FONT_LCD_OFF), LCD_OFF_TEXT);
    }
}

/// What is printed on the panel: the strip of the function keys with its labels, the power group
/// and the `SHIFT` labels.
fn paint_panel_text(painter: &Painter, layout: Layout) {
    let strip = layout.rect(STRIP);
    painter.rect_filled(strip, radius(layout, 1.5), STRIP_FILL);
    let divider = Stroke::new(layout.len(1.0).max(1.0), STRIP_DIVIDER);
    for x in STRIP_DIVIDERS {
        painter.line_segment(
            [layout.pos(pos2(x, STRIP.top() + 2.0)), layout.pos(pos2(x, STRIP.bottom() - 2.0))],
            divider,
        );
    }
    for key in FUNCTION_KEYS {
        if let (Some(rect), Some(label)) = (cap_rect(key), key.shift_label()) {
            painter.text(
                layout.pos(pos2(rect.center().x, STRIP_LABEL_Y)),
                Align2::CENTER_CENTER,
                label,
                layout.font(FONT_STRIP),
                PANEL_TEXT,
            );
        }
    }

    painter.text(layout.pos(POWER_LABEL), Align2::CENTER_CENTER, "POWER", layout.font(FONT_PANEL), PANEL_TEXT);
    painter.line_segment(
        [layout.pos(pos2(POWER_LINE_X.0, POWER_LINE_Y)), layout.pos(pos2(POWER_LINE_X.1, POWER_LINE_Y))],
        Stroke::new(layout.len(1.0).max(1.0), PANEL_TEXT),
    );
    painter.text(layout.pos(BREAK_LABEL), Align2::CENTER_CENTER, "BREAK", layout.font(FONT_PANEL), PANEL_TEXT);

    for (key, legend) in SHIFT_LEGENDS {
        let Some(rect) = cap_rect(key) else { continue };
        let center = rect.center() - vec2(0.0, SHIFT_LEGEND_DY);
        match legend {
            PanelLegend::Text(text) => {
                painter.text(
                    layout.pos(center),
                    Align2::CENTER_CENTER,
                    text,
                    layout.font(FONT_SHIFT_LEGEND),
                    LEGEND_ORANGE,
                );
            }
            PanelLegend::Sqrt => paint_polyline(painter, layout, center, &SQRT_POINTS, LEGEND_ORANGE),
            PanelLegend::Pi => {
                for segment in PI_SEGMENTS {
                    paint_polyline(painter, layout, center, &segment, LEGEND_ORANGE);
                }
            }
        }
    }
}

/// `√` and `π` as segments, relative to the center of the label.
const SQRT_POINTS: [Vec2; 5] = [vec2(-3.0, 0.4), vec2(-2.2, 0.0), vec2(-1.0, 2.6), vec2(0.8, -2.8), vec2(3.2, -2.8)];
const PI_SEGMENTS: [[Vec2; 2]; 3] = [
    [vec2(-3.0, -2.0), vec2(3.0, -2.2)],
    [vec2(-1.3, -2.0), vec2(-1.9, 2.6)],
    [vec2(1.3, -2.1), vec2(1.7, 2.6)],
];

fn paint_polyline(painter: &Painter, layout: Layout, center: Pos2, points: &[Vec2], color: Color32) {
    let points: Vec<Pos2> = points.iter().map(|&p| layout.pos(center + p)).collect();
    painter.add(Shape::line(points, Stroke::new(layout.len(0.9).max(1.0), color)));
}

/// "Fake" bold text: twice, the second half a step to the right (egui's default font has no bold).
fn bold_text(painter: &Painter, layout: Layout, pos: Pos2, text: &str, size: f32, color: Color32) {
    let p = layout.pos(pos);
    let font = layout.font(size);
    painter.text(p, Align2::CENTER_CENTER, text, font.clone(), color);
    let dx = layout.len(size * 0.035).max(0.5);
    painter.text(p + vec2(dx, 0.0), Align2::CENTER_CENTER, text, font, color);
}

/// How a key looks in this repaint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CapState {
    pressed: bool,
    latched: bool,
    hovered: bool,
}

/// Face of a key on the screen: sunk if it is pressed.
fn face_rect(layout: Layout, cap: &CapDef, pressed: bool) -> Rect {
    let drop = if pressed { CAP_PRESS_DROP } else { 0.0 };
    layout.rect(cap.rect().translate(vec2(0.0, drop)))
}

/// A key: edge (shadow) moved down, face, highlight on the top edge and label. Pressed, the face
/// goes down and darkens (and the highlight almost disappears), like a key pushed in.
fn paint_cap(painter: &Painter, layout: Layout, cap: &CapDef, state: CapState) {
    let colors = cap_style(cap.key).colors();
    let rim = layout.rect(cap.rect().translate(vec2(0.0, CAP_RIM_DROP)));
    painter.rect_filled(rim, radius(layout, CAP_RIM_RADIUS), colors.rim);

    let face = face_rect(layout, cap, state.pressed);
    let (fill, highlight) = if state.pressed {
        (colors.face.lerp_to_gamma(colors.rim, 0.22), colors.highlight.lerp_to_gamma(colors.face, 0.7))
    } else if state.hovered {
        (colors.face.lerp_to_gamma(colors.highlight, 0.15), colors.highlight)
    } else {
        (colors.face, colors.highlight)
    };
    painter.rect_filled(face, radius(layout, CAP_FACE_RADIUS), fill);
    let inset = layout.len(CAP_FACE_RADIUS);
    let y = face.top() + layout.len(0.8);
    painter.line_segment(
        [pos2(face.left() + inset, y), pos2(face.right() - inset, y)],
        Stroke::new(layout.len(1.0).max(1.0), highlight),
    );
    if state.latched {
        painter.rect_stroke(
            face,
            radius(layout, CAP_FACE_RADIUS),
            Stroke::new(layout.len(1.2).max(1.0), LATCH_GLOW),
            StrokeKind::Outside,
        );
    }
    paint_legend(painter, layout, face, legend(cap.key), colors.legend);
}

/// The label of a cap, centered on its face.
fn paint_legend(painter: &Painter, layout: Layout, face: Rect, legend: Legend, color: Color32) {
    let center = face.center();
    match legend {
        Legend::Char(text) => {
            painter.text(center, Align2::CENTER_CENTER, text, layout.font(FONT_CAP_CHAR), color);
        }
        Legend::Word(text) => {
            let font = layout.font(FONT_CAP_WORD);
            painter.text(center, Align2::CENTER_CENTER, text, font.clone(), color);
            let dx = layout.len(0.3).max(0.5);
            painter.text(center + vec2(dx, 0.0), Align2::CENTER_CENTER, text, font, color);
        }
        Legend::Arrow(dir) => {
            for shape in arrow_shapes(center, dir.unit(), layout, color) {
                painter.add(shape);
            }
        }
        Legend::UpDown => {
            let gap = layout.vec(vec2(0.0, 3.6));
            let head = layout.len(3.0);
            let half = layout.len(3.0);
            painter.add(triangle(center - gap, Dir::Up.unit(), head, half, color));
            painter.add(triangle(center + gap, Dir::Down.unit(), head, half, color));
            // The two lines of the body of `⇕`.
            let stroke = Stroke::new(layout.len(0.8).max(1.0), color);
            for dx in [-1.0, 1.0] {
                let x = layout.len(dx);
                painter.line_segment([center - gap + vec2(x, 0.0), center + gap + vec2(x, 0.0)], stroke);
            }
        }
        Legend::Blank => {}
    }
}

/// Arrow with a shaft: triangular head plus a rectangle, centered at `center` and pointing at
/// `dir`.
fn arrow_shapes(center: Pos2, dir: Vec2, layout: Layout, color: Color32) -> [Shape; 2] {
    let total = layout.len(10.0);
    let head = layout.len(4.6);
    let half = layout.len(3.4);
    let shaft_half = layout.len(0.85);
    let tip = center + dir * (total / 2.0);
    let base = tip - dir * head;
    let tail = center - dir * (total / 2.0);
    let normal = vec2(-dir.y, dir.x);
    let shaft = Shape::convex_polygon(
        vec![
            tail + normal * shaft_half,
            base + normal * shaft_half,
            base - normal * shaft_half,
            tail - normal * shaft_half,
        ],
        color,
        Stroke::NONE,
    );
    [shaft, triangle_at(base, dir, head, half, color)]
}

/// Triangle centered at `center` (along `dir`) pointing at `dir`.
fn triangle(center: Pos2, dir: Vec2, length: f32, half: f32, color: Color32) -> Shape {
    triangle_at(center - dir * (length / 2.0), dir, length, half, color)
}

/// Triangle with the base centered at `base` and the tip at `length` in the direction `dir`.
fn triangle_at(base: Pos2, dir: Vec2, length: f32, half: f32, color: Color32) -> Shape {
    let normal = vec2(-dir.y, dir.x);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(base + dir * length, color);
    mesh.colored_vertex(base + normal * half, color);
    mesh.colored_vertex(base - normal * half, color);
    mesh.add_triangle(0, 1, 2);
    Shape::mesh(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outline(cap: &CapDef) -> Rect {
        cap_outline(cap.rect())
    }

    #[test]
    fn every_key_has_exactly_one_cap() {
        for key in Key::ALL {
            let count = CAPS.iter().filter(|c| c.key == key).count();
            assert_eq!(count, 1, "{key:?} on the faceplate");
            assert!(cap_rect(key).is_some(), "{key:?} without a cap");
        }
    }

    #[test]
    fn caps_do_not_overlap() {
        for (i, a) in CAPS.iter().enumerate() {
            for b in &CAPS[i + 1..] {
                assert!(
                    !outline(a).intersects(outline(b)),
                    "{:?} and {:?} overlap",
                    a.key,
                    b.key
                );
            }
        }
    }

    #[test]
    fn caps_fit_inside_the_panel_and_below_the_top_band() {
        for cap in &CAPS {
            let r = outline(cap);
            assert!(PANEL.contains_rect(r), "{:?} outside the panel", cap.key);
            assert!(BODY.contains_rect(r), "{:?} outside the case", cap.key);
            assert!(r.top() > TOP_BAND.bottom(), "{:?} overlaps the top band", cap.key);
        }
    }

    #[test]
    fn function_keys_sit_on_the_strip_between_dividers() {
        for (i, key) in FUNCTION_KEYS.iter().enumerate() {
            let r = cap_rect(*key).map(cap_outline).unwrap_or(Rect::NOTHING);
            assert!(STRIP.contains_rect(r), "{key:?} outside the strip");
            let left = if i == 0 { STRIP.left() } else { STRIP_DIVIDERS[i - 1] };
            let right = STRIP_DIVIDERS.get(i).copied().unwrap_or(STRIP.right());
            assert!(r.left() > left && r.right() < right, "{key:?} overlaps a separator");
            // The labels of the strip, above the key.
            assert!(STRIP_LABEL_Y + FONT_STRIP / 2.0 < r.top());
            assert!(key.shift_label().is_some());
        }
    }

    #[test]
    fn shift_legends_match_the_core_and_do_not_touch_caps() {
        for (key, legend) in SHIFT_LEGENDS {
            if let PanelLegend::Text(text) = legend {
                assert_eq!(key.shift_label(), Some(text), "{key:?}");
            }
            let center = cap_rect(key).map(|r| r.center()).unwrap_or_default() - vec2(0.0, SHIFT_LEGEND_DY);
            let area = Rect::from_center_size(center, vec2(FONT_SHIFT_LEGEND * 2.0, FONT_SHIFT_LEGEND));
            assert!(PANEL.contains_rect(area), "{key:?}");
            for cap in &CAPS {
                assert!(!outline(cap).intersects(area), "the label of {key:?} overlaps {:?}", cap.key);
            }
        }
    }

    #[test]
    fn lcd_matrix_and_annunciators_fit_the_glass() {
        assert!(LCD_FRAME_RECT.contains_rect(LCD_GLASS));
        assert!(TOP_BAND.contains_rect(LCD_FRAME_RECT));
        // 156 columns of matrix; the indicators reach pitch 159 and their band starts 3.4 pitches
        // above.
        assert!(159.0_f32.mul_add(LCD_PITCH, LCD_MATRIX_ORIGIN.x) < LCD_GLASS.right());
        assert!(3.4_f32.mul_add(-LCD_PITCH, LCD_MATRIX_ORIGIN.y) > LCD_GLASS.top());
        assert!(7.0_f32.mul_add(LCD_PITCH, LCD_MATRIX_ORIGIN.y) < LCD_GLASS.bottom());
        assert!(LCD_MATRIX_ORIGIN.x > LCD_GLASS.left());
    }

    #[test]
    fn cap_styles_follow_the_real_machine() {
        assert_eq!(cap_style(Key::Cl), CapStyle::Red);
        assert_eq!(cap_style(Key::Shift), CapStyle::Orange);
        for key in [Key::A, Key::Z, Key::LeftParen, Key::RightParen, Key::Space] {
            assert_eq!(cap_style(key), CapStyle::Dark, "{key:?}");
        }
        for key in [Key::Control, Key::F1, Key::On, Key::Off, Key::Enter, Key::Zero, Key::Mode, Key::Left, Key::Rsv] {
            assert_eq!(cap_style(key), CapStyle::Grey, "{key:?}");
        }
    }

    #[test]
    fn legends() {
        assert_eq!(legend(Key::A), Legend::Char("A"));
        assert_eq!(legend(Key::Seven), Legend::Char("7"));
        assert_eq!(legend(Key::On), Legend::Word("ON"));
        assert_eq!(legend(Key::Control), Legend::Word("DEF"));
        assert_eq!(legend(Key::F3), Legend::Blank);
        assert_eq!(legend(Key::Up), Legend::Arrow(Dir::Up));
        assert_eq!(legend(Key::Rsv), Legend::UpDown);
        // No cap label depends on non-ASCII glyphs of the font.
        for key in Key::ALL {
            if let Legend::Char(text) | Legend::Word(text) = legend(key) {
                assert!(text.is_ascii(), "{key:?}: {text}");
            }
        }
    }

    #[test]
    fn layout_fits_and_centers_keeping_the_aspect_ratio() {
        let avail = Rect::from_min_size(pos2(10.0, 20.0), vec2(1358.0, 1000.0));
        let layout = Layout::fit(avail);
        assert!((layout.scale - 2.0).abs() < 1e-4, "the width limits it");
        let body = layout.rect(BODY);
        assert!((body.center() - avail.center()).length() < 1e-3);
        assert!(avail.expand(1e-3).contains_rect(body));

        let tall = Layout::fit(Rect::from_min_size(Pos2::ZERO, vec2(5000.0, 299.0)));
        assert!((tall.scale - 1.0).abs() < 1e-4, "the height limits it");
        assert!((tall.pos(pos2(REF_W, REF_H)).y - 299.0).abs() < 1e-3);

        assert!(Layout::fit(Rect::from_min_size(Pos2::ZERO, Vec2::ZERO)).scale.abs() < f32::EPSILON);
    }

    #[test]
    fn brushed_texture_is_deterministic_and_in_range() {
        let a = brushed_pixels();
        assert_eq!(a.len(), BRUSHED_W * BRUSHED_H);
        assert_eq!(a, brushed_pixels());
        for p in &a {
            assert!((196..=219).contains(&p.g()), "gris {}", p.g());
            assert_eq!(p.r(), p.g() + 2, "warm touch");
            assert_eq!(p.b(), p.g());
        }
        // Grain: not all rows are the same.
        let first: Vec<u8> = a.iter().step_by(BRUSHED_W).map(|p| p.g()).collect();
        assert!(first.windows(2).any(|w| w[0] != w[1]));
    }

    #[test]
    fn band_gradient_goes_from_hi_to_lo() {
        let g = band_gradient_pixels();
        assert_eq!(g.len(), BAND_STEPS);
        assert_eq!(g.first(), Some(&TOP_BAND_HI));
        assert_eq!(g.last(), Some(&TOP_BAND_LO));
        assert!(g.windows(2).all(|w| w[1].r() <= w[0].r()));
    }
}
