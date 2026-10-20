//! Painting the PC-1500's LCD as a real grey STN LCD: a greenish grey glass with a slight gradient,
//! almost black dots with their shadow on the background (the glass's thickness), optional "ghosts"
//! of the unlit dots, and the indicators ("annunciators") INSIDE the glass, above the matrix, in
//! the same positions and with the same colors as on the real machine.
//!
//! The GUI used to create every frame an RGBA texture in pure black and white from
//! `DisplayController::rgba_buffer` and wrote the indicators as normal egui labels in a separate
//! row, outside the LCD (with names that are not the machine's: "SML", "DEG", "RSV"...). Now
//! everything is painted procedurally from the 156 columns of `DisplayController::columns`, without
//! textures: a single `Mesh` per layer (ghosts, shadows, lit dots), so the cost is negligible even
//! at maximum speed.
//!
//! The geometry of the indicators comes from the real machine's measurements (expressed again here
//! in our own table, see [`ANNUNCIATORS`]); the 3x5-cell type is our own (see [`glyph`]).

use std::sync::OnceLock;

use ceres_core::display::{DISPLAY_HEIGHT, DISPLAY_WIDTH, DisplayController, SYMBOL_COUNT, Symbol};
use eframe::egui::{
    Color32, CornerRadius, Mesh, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
    pos2, vec2,
};

/// What has to be painted of the LCD in a repaint: a snapshot of `DisplayController` taken only
/// once per update of the emulator (see `Pc1500App::update_emulator`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LcdFrame {
    /// Whether the LCD shows lit (`DisplayController::is_on`). Off, no lit dot or indicator is
    /// painted (the ghosts are, because the physical segments can still be seen a little).
    pub on: bool,
    /// The 156 columns (bit 0 = top row), see `DisplayController::columns`.
    pub columns: [u8; DISPLAY_WIDTH],
    /// State of each indicator, indexed by `Symbol as usize`.
    pub symbols: [bool; SYMBOL_COUNT],
}

impl LcdFrame {
    /// LCD off and empty (before the first emulated frame).
    pub const BLANK: Self = Self {
        on: false,
        columns: [0; DISPLAY_WIDTH],
        symbols: [false; SYMBOL_COUNT],
    };

    /// Copies what `display` shows right now.
    #[must_use]
    pub fn from_display(display: &DisplayController) -> Self {
        Self {
            on: display.is_on(),
            columns: *display.columns(),
            symbols: Symbol::ALL.map(|s| display.is_symbol_on(s)),
        }
    }

    /// `true` if the dot (`x`, `y`) shows lit.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> bool {
        self.on && y < DISPLAY_HEIGHT && self.columns.get(x).is_some_and(|&c| (c >> y) & 1 != 0)
    }

    /// `true` if the indicator `symbol` shows lit.
    #[must_use]
    pub const fn symbol(&self, symbol: Symbol) -> bool {
        self.on && self.symbols[symbol as usize]
    }
}

impl Default for LcdFrame {
    fn default() -> Self {
        Self::BLANK
    }
}

/// Colors of the LCD.
#[derive(Clone, Copy, Debug)]
pub struct LcdPalette {
    /// Background of the glass at the top and at the bottom (vertical gradient).
    pub glass_top: Color32,
    pub glass_bottom: Color32,
    /// Lit dot or indicator.
    pub on: Color32,
    /// Unlit dot or indicator, barely visible ("ghost").
    pub ghost: Color32,
    /// Shadow that each lit dot casts on the background, because of the glass's thickness.
    pub shadow: Color32,
    /// Silver frame around the glass.
    pub frame: Color32,
}

/// The palette of a grey STN LCD like the PC-1500's.
pub const STN_GREY: LcdPalette = LcdPalette {
    glass_top: Color32::from_rgb(0xB2, 0xBB, 0xA8),
    glass_bottom: Color32::from_rgb(0xA1, 0xAB, 0x97),
    on: Color32::from_rgb(0x1F, 0x26, 0x28),
    ghost: Color32::from_rgba_unmultiplied_const(0x1F, 0x26, 0x28, 18),
    shadow: Color32::from_rgba_unmultiplied_const(0, 0, 0, 28),
    frame: Color32::from_rgb(0xB9, 0xBD, 0xBB),
};

/// Dark line on the top and left edges of the glass, to give a sense of depth.
const GLASS_INNER_EDGE: Color32 = Color32::from_rgba_unmultiplied_const(0, 0, 0, 60);

/// Fraction of the grid's pitch that each dot takes (the rest is the gap between dots).
const DOT_FILL: f32 = 0.86;

/// Offset of each dot's shadow, as a fraction of the pitch.
const SHADOW_OFFSET: f32 = 0.22;

/// Distance (in pitches) from the top edge of the matrix to the top edge of the band of indicators.
/// Measured on the real machine: the band of indicators is 8 units above the matrix with a pitch of
/// 2.36 units.
const BAND_TOP: f32 = 3.4;

/// Rows of the indicators' type.
const GLYPH_ROWS: u8 = 5;

/// Margin of the glass around matrix and indicators in [`LcdGeometry::fit`], in pitches: left,
/// right, top (above the band of indicators) and bottom.
const GLASS_MARGIN_LEFT: f32 = 3.0;
const GLASS_MARGIN_RIGHT: f32 = 3.0;
const GLASS_MARGIN_TOP: f32 = 1.6;
const GLASS_MARGIN_BOTTOM: f32 = 2.5;

/// Total width, in pitches, that matrix and indicators take: the battery indicator (`BATTERY`)
/// starts at half cell 314 and is 4 wide, that is, it reaches pitch 159, beyond the end of the
/// matrix (156).
const CONTENT_WIDTH: f32 = 159.0;

/// Size of the LCD in [`LcdGeometry::fit`], in pitches.
const FIT_WIDTH: f32 = GLASS_MARGIN_LEFT + CONTENT_WIDTH + GLASS_MARGIN_RIGHT;
const FIT_HEIGHT: f32 = GLASS_MARGIN_TOP + BAND_TOP + 7.0 + GLASS_MARGIN_BOTTOM;

/// Where the LCD is painted, in egui's logical points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LcdGeometry {
    /// Rectangle of the glass (the frame is painted just outside).
    pub glass: Rect,
    /// Nominal top left corner of the 156x7 matrix.
    pub matrix_origin: Pos2,
    /// Nominal distance between consecutive dots.
    pub pitch: f32,
}

impl LcdGeometry {
    /// Geometry of a "standalone" LCD that takes exactly `rect` (of the proportions of
    /// [`LcdGeometry::fit_size`]).
    #[must_use]
    pub fn fit(rect: Rect) -> Self {
        let pitch = (rect.width() / FIT_WIDTH).min(rect.height() / FIT_HEIGHT);
        let size = Self::fit_size(pitch);
        let glass = Rect::from_center_size(rect.center(), size);
        let matrix_origin =
            glass.min + vec2(GLASS_MARGIN_LEFT * pitch, (GLASS_MARGIN_TOP + BAND_TOP) * pitch);
        Self { glass, matrix_origin, pitch }
    }

    /// Size of the standalone LCD of [`LcdGeometry::fit`] for a given pitch.
    #[must_use]
    pub fn fit_size(pitch: f32) -> Vec2 {
        vec2(FIT_WIDTH * pitch, FIT_HEIGHT * pitch)
    }
}

/// Grid of the matrix fitted to physical pixels, in logical points.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Grid {
    /// Real top left corner of the matrix (already centered and aligned to the physical pixel).
    origin: Pos2,
    /// Real pitch between dots.
    pitch: f32,
    /// Side of each dot.
    dot: f32,
}

/// Fits the grid to physical pixels so that the dots do not "wobble" (some a pixel bigger than
/// others) at fractional scales: a whole pitch in physical pixels (at least 2, so that there is
/// always a gap), a dot of [`DOT_FILL`] of the pitch (at least 1 pixel and always at least 1 pixel
/// of gap), and the resulting matrix centered inside the nominal rectangle of `156 × pitch` by
/// `7 × pitch`.
fn snap_grid(origin: Pos2, pitch: f32, ppp: f32) -> Grid {
    let pitch_px = (pitch * ppp).floor().max(2.0);
    let dot_px = (pitch_px * DOT_FILL).round().clamp(1.0, pitch_px - 1.0);
    let snapped_pitch = pitch_px / ppp;
    let dot = dot_px / ppp;
    let width = f32::from(u8::try_from(DISPLAY_WIDTH).unwrap_or(u8::MAX));
    let height = f32::from(u8::try_from(DISPLAY_HEIGHT).unwrap_or(u8::MAX));
    let slack = vec2((pitch - snapped_pitch) * width, (pitch - snapped_pitch) * height) / 2.0;
    let centered = origin + slack;
    Grid {
        origin: pos2(snap(centered.x, ppp), snap(centered.y, ppp)),
        pitch: snapped_pitch,
        dot,
    }
}

/// Rounds a coordinate to the nearest physical pixel.
fn snap(v: f32, ppp: f32) -> f32 {
    (v * ppp).round() / ppp
}

/// A glyph of the indicators' type: `width` cells wide and 5 rows; each row is a bit mask with the
/// most significant bit (of the `width`) on the left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Glyph {
    width: u8,
    rows: [u8; GLYPH_ROWS as usize],
}

const fn g3(rows: [u8; 5]) -> Glyph {
    Glyph { width: 3, rows }
}

/// Our own type of 3x5 cells (some letters wider), designed for these indicators — it is NOT
/// `PockEmul`'s (GPL). It only includes the letters needed; any other is a gap.
const fn glyph(c: u8) -> Glyph {
    match c {
        b'A' => g3([0b010, 0b101, 0b111, 0b101, 0b101]),
        b'B' => g3([0b110, 0b101, 0b110, 0b101, 0b110]),
        b'D' => g3([0b110, 0b101, 0b101, 0b101, 0b110]),
        b'E' => g3([0b111, 0b100, 0b110, 0b100, 0b111]),
        b'F' => g3([0b111, 0b100, 0b110, 0b100, 0b100]),
        b'G' => Glyph { width: 4, rows: [0b0111, 0b1000, 0b1011, 0b1001, 0b0110] },
        b'H' => g3([0b101, 0b101, 0b111, 0b101, 0b101]),
        // The "I" inside a word (SHIFT) is a plain stroke.
        b'I' => Glyph { width: 1, rows: [1, 1, 1, 1, 1] },
        b'K' => g3([0b101, 0b101, 0b110, 0b101, 0b101]),
        b'L' => g3([0b100, 0b100, 0b100, 0b100, 0b111]),
        b'M' => Glyph { width: 5, rows: [0b10001, 0b11011, 0b10101, 0b10001, 0b10001] },
        b'N' => Glyph { width: 4, rows: [0b1001, 0b1101, 0b1011, 0b1001, 0b1001] },
        b'O' => g3([0b010, 0b101, 0b101, 0b101, 0b010]),
        b'P' => g3([0b110, 0b101, 0b110, 0b100, 0b100]),
        b'R' => g3([0b110, 0b101, 0b110, 0b101, 0b101]),
        b'S' => g3([0b011, 0b100, 0b010, 0b001, 0b110]),
        b'T' => g3([0b111, 0b010, 0b010, 0b010, 0b010]),
        b'U' => g3([0b101, 0b101, 0b101, 0b101, 0b111]),
        b'V' => g3([0b101, 0b101, 0b101, 0b101, 0b010]),
        b'Y' => g3([0b101, 0b101, 0b010, 0b010, 0b010]),
        _ => g3([0; 5]),
    }
}

/// Roman numerals of the RESERVE levels, with their serif bars above and below.
const ROMAN_I: Glyph = g3([0b111, 0b010, 0b010, 0b010, 0b111]);
const ROMAN_II: Glyph = Glyph { width: 5, rows: [0b11111, 0b01010, 0b01010, 0b01010, 0b11111] };
const ROMAN_III: Glyph = Glyph {
    width: 7,
    rows: [0b111_1111, 0b010_1010, 0b010_1010, 0b010_1010, 0b111_1111],
};
/// Battery symbol.
const BATTERY: Glyph = Glyph { width: 4, rows: [0b0110, 0b1111, 0b1001, 0b1001, 0b1111] };

/// How an indicator is drawn.
#[derive(Clone, Copy, Debug)]
enum Label {
    /// A word with the type of [`glyph`], a blank cell between letters.
    Word(&'static str),
    /// A single special glyph.
    Glyph(Glyph),
}

/// An indicator: which symbol it is, where it starts (in half cells = half pitches of the matrix,
/// from its left edge), how it is drawn and whether its ghost shows.
#[derive(Clone, Copy, Debug)]
struct Annunciator {
    symbol: Symbol,
    x: u16,
    label: Label,
    /// `false` for the segments this model never lights (KANA on the export model, which is the one
    /// emulated; the battery, which we do not emulate): they are painted only when lit, never as a
    /// ghost, so as not to show a segment the user would not recognize.
    ghost: bool,
}

/// The 15 indicators in their real position. `DE`, `G` and `RAD` go together (one cell of gap) to
/// read as "DEG" (`DE`+`G`), "RAD" or "GRAD" (`G`+`RAD`), as on the machine.
const ANNUNCIATORS: [Annunciator; SYMBOL_COUNT] = [
    ann(Symbol::Busy, 2, Label::Word("BUSY")),
    ann(Symbol::Shift, 32, Label::Word("SHIFT")),
    Annunciator { symbol: Symbol::Kana, x: 62, label: Label::Word("KANA"), ghost: false },
    ann(Symbol::Small, 82, Label::Word("SMALL")),
    ann(Symbol::De, 114, Label::Word("DE")),
    ann(Symbol::G, 122, Label::Word("G")),
    ann(Symbol::Rad, 127, Label::Word("RAD")),
    ann(Symbol::Run, 162, Label::Word("RUN")),
    ann(Symbol::Pro, 192, Label::Word("PRO")),
    ann(Symbol::Reserve, 212, Label::Word("RESERVE")),
    ann(Symbol::Def, 252, Label::Word("DEF")),
    ann(Symbol::RomanI, 278, Label::Glyph(ROMAN_I)),
    ann(Symbol::RomanII, 284, Label::Glyph(ROMAN_II)),
    ann(Symbol::RomanIII, 294, Label::Glyph(ROMAN_III)),
    Annunciator { symbol: Symbol::Battery, x: 314, label: Label::Glyph(BATTERY), ghost: false },
];

const fn ann(symbol: Symbol, x: u16, label: Label) -> Annunciator {
    Annunciator { symbol, x, label, ghost: true }
}

/// Lit cells `(x, y)` of an indicator, relative to its top left corner, and its total width in
/// cells.
fn label_cells(label: Label) -> (Vec<(u16, u8)>, u16) {
    let glyphs: Vec<Glyph> = match label {
        Label::Word(word) => word.bytes().map(glyph).collect(),
        Label::Glyph(g) => vec![g],
    };
    let mut cells = Vec::new();
    let mut x0: u16 = 0;
    for (i, g) in glyphs.iter().enumerate() {
        if i > 0 {
            x0 += 1;
        }
        for (y, row) in (0..).zip(g.rows) {
            for col in 0..g.width {
                if (row >> (g.width - 1 - col)) & 1 != 0 {
                    cells.push((x0 + u16::from(col), y));
                }
            }
        }
        x0 += u16::from(g.width);
    }
    (cells, x0)
}

/// Cells of each indicator of [`ANNUNCIATORS`], already in band coordinates (half cells from the
/// matrix's left edge), worked out only once.
fn annunciator_cells() -> &'static [Vec<(u16, u8)>; SYMBOL_COUNT] {
    static CELLS: OnceLock<[Vec<(u16, u8)>; SYMBOL_COUNT]> = OnceLock::new();
    CELLS.get_or_init(|| {
        ANNUNCIATORS.map(|a| {
            label_cells(a.label)
                .0
                .into_iter()
                .map(|(x, y)| (a.x + x, y))
                .collect()
        })
    })
}

/// Paints the whole LCD (glass, matrix and indicators) in `geometry`. `ghost` turns on the ghosts
/// of the unlit dots and indicators.
pub fn paint(painter: &Painter, geometry: &LcdGeometry, frame: &LcdFrame, pal: &LcdPalette, ghost: bool) {
    let ppp = painter.pixels_per_point();
    paint_glass(painter, geometry.glass, pal);

    let grid = snap_grid(geometry.matrix_origin, geometry.pitch, ppp);
    let dot = Vec2::splat(grid.dot);
    let shadow = Vec2::splat(snap(SHADOW_OFFSET * grid.pitch, ppp).max(1.0 / ppp));
    let dot_min = |x: u8, y: u8| grid.origin + vec2(f32::from(x), f32::from(y)) * grid.pitch;

    let mut ghost_mesh = Mesh::default();
    let mut shadow_mesh = Mesh::default();
    let mut on_mesh = Mesh::default();

    for x in 0..u8::try_from(DISPLAY_WIDTH).unwrap_or(u8::MAX) {
        for y in 0..u8::try_from(DISPLAY_HEIGHT).unwrap_or(u8::MAX) {
            let min = dot_min(x, y);
            if frame.pixel(usize::from(x), usize::from(y)) {
                shadow_mesh.add_colored_rect(Rect::from_min_size(min + shadow, dot), pal.shadow);
                on_mesh.add_colored_rect(Rect::from_min_size(min, dot), pal.on);
            } else if ghost {
                ghost_mesh.add_colored_rect(Rect::from_min_size(min, dot), pal.ghost);
            } else {
                // Off and without ghosts: only the glass shows.
            }
        }
    }

    // Indicators: cells of half a pitch, next to each other (they are solid printed segments, not
    // dots), with their edges aligned to the physical pixel so that there are no gaps or overlaps.
    let cell = grid.pitch / 2.0;
    let band_top = BAND_TOP.mul_add(-grid.pitch, grid.origin.y);
    let cell_rect = |x: u16, y: u8| {
        let x0 = f32::from(x).mul_add(cell, grid.origin.x);
        let y0 = f32::from(y).mul_add(cell, band_top);
        Rect::from_min_max(
            pos2(snap(x0, ppp), snap(y0, ppp)),
            pos2(snap(x0 + cell, ppp), snap(y0 + cell, ppp)),
        )
    };
    for (a, cells) in ANNUNCIATORS.iter().zip(annunciator_cells()) {
        let lit = frame.symbol(a.symbol);
        for &(x, y) in cells {
            let rect = cell_rect(x, y);
            if lit {
                shadow_mesh.add_colored_rect(rect.translate(shadow), pal.shadow);
                on_mesh.add_colored_rect(rect, pal.on);
            } else if ghost && a.ghost {
                ghost_mesh.add_colored_rect(rect, pal.ghost);
            } else {
                // Off and without a ghost: nothing is painted.
            }
        }
    }

    for mesh in [ghost_mesh, shadow_mesh, on_mesh] {
        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }
}

/// Glass: vertical gradient, silver frame and a dark line at the top and on the left to give depth.
fn paint_glass(painter: &Painter, glass: Rect, pal: &LcdPalette) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(glass.left_top(), pal.glass_top);
    mesh.colored_vertex(glass.right_top(), pal.glass_top);
    mesh.colored_vertex(glass.right_bottom(), pal.glass_bottom);
    mesh.colored_vertex(glass.left_bottom(), pal.glass_bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));

    let edge = Stroke::new(1.0, GLASS_INNER_EDGE);
    let inner = glass.shrink(0.5);
    painter.line_segment([inner.left_top(), inner.right_top()], edge);
    painter.line_segment([inner.left_top(), inner.left_bottom()], edge);
    painter.rect_stroke(glass, CornerRadius::ZERO, Stroke::new(1.0, pal.frame), StrokeKind::Outside);
}

/// Reserves in `ui` a standalone LCD of `pitch` points per pitch (or less, if it does not fit
/// across) and paints it with [`STN_GREY`].
pub fn show(ui: &mut Ui, frame: &LcdFrame, ghost: bool, pitch: f32) -> Response {
    let pitch = pitch.min(ui.available_width() / FIT_WIDTH).max(1.0);
    let (rect, response) = ui.allocate_exact_size(LcdGeometry::fit_size(pitch), Sense::hover());
    paint(ui.painter(), &LcdGeometry::fit(rect), frame, &STN_GREY, ghost);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width_of(label: Label) -> u16 {
        label_cells(label).1
    }

    #[test]
    fn word_widths_match_the_design() {
        for (word, width) in [
            ("BUSY", 15),
            ("SHIFT", 17),
            ("KANA", 16),
            ("SMALL", 21),
            ("DE", 7),
            ("G", 4),
            ("RAD", 11),
            ("RUN", 12),
            ("PRO", 11),
            ("RESERVE", 27),
            ("DEF", 11),
        ] {
            assert_eq!(width_of(Label::Word(word)), width, "{word}");
        }
    }

    #[test]
    fn every_symbol_has_exactly_one_annunciator() {
        for (a, symbol) in ANNUNCIATORS.iter().zip(Symbol::ALL) {
            assert_eq!(a.symbol, symbol);
        }
    }

    #[test]
    fn annunciators_do_not_overlap_and_fit_the_band() {
        let mut previous_end = 0;
        for a in &ANNUNCIATORS {
            assert!(a.x >= previous_end, "{:?} overlaps the previous one", a.symbol);
            previous_end = a.x + width_of(a.label);
        }
        // `CONTENT_WIDTH` pitches = twice as many half cells.
        assert!(f32::from(previous_end) <= CONTENT_WIDTH * 2.0);
    }

    #[test]
    fn deg_and_grad_segments_are_packed_one_cell_apart() {
        let end = |s: Symbol| {
            let a = ANNUNCIATORS[s as usize];
            a.x + width_of(a.label)
        };
        assert_eq!(end(Symbol::De) + 1, ANNUNCIATORS[Symbol::G as usize].x);
        assert_eq!(end(Symbol::G) + 1, ANNUNCIATORS[Symbol::Rad as usize].x);
    }

    #[test]
    fn glyph_cells_follow_the_bitmaps() {
        let (cells, width) = label_cells(Label::Word("DE"));
        assert_eq!(width, 7);
        // D: 110 / 101 / 101 / 101 / 110 and E from column 4.
        assert!(cells.contains(&(0, 0)) && cells.contains(&(1, 0)) && !cells.contains(&(2, 0)));
        assert!(cells.contains(&(2, 1)) && !cells.contains(&(3, 1)));
        assert!(cells.contains(&(4, 0)) && cells.contains(&(6, 0)) && cells.contains(&(6, 4)));
        assert_eq!(label_cells(Label::Glyph(ROMAN_III)).0.len(), 7 + 3 * 3 + 7);
    }

    #[test]
    fn grid_snaps_to_physical_pixels() {
        for ppp in [1.0_f32, 1.25, 1.5, 2.0] {
            for pitch in [2.36_f32, 3.0, 5.5, 7.0] {
                let grid = snap_grid(pos2(10.3, 20.7), pitch, ppp);
                let pitch_px = grid.pitch * ppp;
                let dot_px = grid.dot * ppp;
                assert!((pitch_px - pitch_px.round()).abs() < 1e-4, "pitch {pitch_px} px");
                assert!((dot_px - dot_px.round()).abs() < 1e-4, "punto {dot_px} px");
                assert!(pitch_px >= 2.0 && dot_px >= 1.0 && dot_px < pitch_px);
                assert!(grid.pitch <= pitch.max(2.0 / ppp));
                let ox = grid.origin.x * ppp;
                assert!((ox - ox.round()).abs() < 1e-3, "origin {ox} px");
            }
        }
    }

    #[test]
    fn grid_is_centered_in_the_nominal_rect() {
        let grid = snap_grid(pos2(0.0, 0.0), 6.5, 1.0);
        assert!((grid.pitch - 6.0).abs() < 1e-6);
        // 156 × 0.5 / 2 = 39 points of slack on each side.
        assert!((grid.origin.x - 39.0).abs() <= 0.5);
    }

    #[test]
    fn frame_hides_everything_when_off() {
        let mut frame = LcdFrame { on: true, columns: [0x7F; DISPLAY_WIDTH], symbols: [true; SYMBOL_COUNT] };
        assert!(frame.pixel(0, 0) && frame.pixel(155, 6) && !frame.pixel(156, 0) && !frame.pixel(0, 7));
        assert!(frame.symbol(Symbol::Run));
        frame.on = false;
        assert!(!frame.pixel(0, 0));
        assert!(!frame.symbol(Symbol::Run));
    }

    #[test]
    fn fit_geometry_keeps_matrix_and_band_inside_the_glass() {
        let rect = Rect::from_min_size(pos2(5.0, 5.0), LcdGeometry::fit_size(4.0));
        let g = LcdGeometry::fit(rect);
        assert!((g.pitch - 4.0).abs() < 1e-4);
        let band_top = BAND_TOP.mul_add(-g.pitch, g.matrix_origin.y);
        assert!(band_top > g.glass.top());
        assert!(7.0_f32.mul_add(g.pitch, g.matrix_origin.y) < g.glass.bottom());
        assert!(g.matrix_origin.x > g.glass.left());
        assert!(CONTENT_WIDTH.mul_add(g.pitch, g.matrix_origin.x) < g.glass.right());
    }
}
