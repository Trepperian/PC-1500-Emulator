//! Pintado del LCD de la PC-1500 como un LCD STN gris de verdad: cristal
//! gris verdoso con un ligero degradado, puntos casi negros con su sombra
//! sobre el fondo (el grosor del cristal), "fantasmas" opcionales de los
//! puntos apagados y los indicadores ("annunciators") DENTRO del cristal,
//! encima de la matriz, en las mismas posiciones y con los mismos colores
//! que en la máquina real.
//!
//! Antes la GUI creaba cada frame una textura RGBA en blanco y negro puro
//! a partir de `DisplayController::rgba_buffer` y escribía los
//! indicadores como etiquetas normales de egui en una fila aparte, fuera
//! del LCD (con nombres que no son los de la máquina: "SML", "DEG",
//! "RSV"...). Ahora todo se pinta de forma procedural a partir de las 156
//! columnas de `DisplayController::columns`, sin texturas: un único
//! `Mesh` por capa (fantasmas, sombras, puntos encendidos), así que el
//! coste es despreciable incluso a velocidad máxima.
//!
//! La geometría de los indicadores sale de las medidas de la máquina real
//! (re-expresadas aquí en nuestra propia tabla, ver [`ANNUNCIATORS`]); la
//! tipografía de 3x5 celdas es propia (ver [`glyph`]).

use std::sync::OnceLock;

use ceres_core::display::{DISPLAY_HEIGHT, DISPLAY_WIDTH, DisplayController, SYMBOL_COUNT, Symbol};
use eframe::egui::{
    Color32, CornerRadius, Mesh, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
    pos2, vec2,
};

/// Lo que hay que pintar del LCD en un repintado: una instantánea de
/// `DisplayController` tomada una sola vez por actualización del
/// emulador (ver `Pc1500App::update_emulator`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LcdFrame {
    /// Si el LCD se ve encendido (`DisplayController::is_on`). Apagado, no
    /// se pinta ningún punto ni indicador encendido (los fantasmas sí,
    /// porque los segmentos físicos se siguen viendo un poco).
    pub on: bool,
    /// Las 156 columnas (bit 0 = fila superior), ver
    /// `DisplayController::columns`.
    pub columns: [u8; DISPLAY_WIDTH],
    /// Estado de cada indicador, indexado por `Symbol as usize`.
    pub symbols: [bool; SYMBOL_COUNT],
}

impl LcdFrame {
    /// LCD apagado y vacío (antes del primer frame emulado).
    pub const BLANK: Self = Self {
        on: false,
        columns: [0; DISPLAY_WIDTH],
        symbols: [false; SYMBOL_COUNT],
    };

    /// Copia lo que se ve ahora mismo en `display`.
    #[must_use]
    pub fn from_display(display: &DisplayController) -> Self {
        Self {
            on: display.is_on(),
            columns: *display.columns(),
            symbols: Symbol::ALL.map(|s| display.is_symbol_on(s)),
        }
    }

    /// `true` si el punto (`x`, `y`) se ve encendido.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> bool {
        self.on && y < DISPLAY_HEIGHT && self.columns.get(x).is_some_and(|&c| (c >> y) & 1 != 0)
    }

    /// `true` si el indicador `symbol` se ve encendido.
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

/// Colores del LCD.
#[derive(Clone, Copy, Debug)]
pub struct LcdPalette {
    /// Fondo del cristal arriba y abajo (degradado vertical).
    pub glass_top: Color32,
    pub glass_bottom: Color32,
    /// Punto o indicador encendido.
    pub on: Color32,
    /// Punto o indicador apagado, apenas visible ("fantasma").
    pub ghost: Color32,
    /// Sombra que proyecta cada punto encendido sobre el fondo, por el
    /// grosor del cristal.
    pub shadow: Color32,
    /// Marco plateado alrededor del cristal.
    pub frame: Color32,
}

/// La paleta de un LCD STN gris como el de la PC-1500.
pub const STN_GREY: LcdPalette = LcdPalette {
    glass_top: Color32::from_rgb(0xB2, 0xBB, 0xA8),
    glass_bottom: Color32::from_rgb(0xA1, 0xAB, 0x97),
    on: Color32::from_rgb(0x1F, 0x26, 0x28),
    ghost: Color32::from_rgba_unmultiplied_const(0x1F, 0x26, 0x28, 18),
    shadow: Color32::from_rgba_unmultiplied_const(0, 0, 0, 28),
    frame: Color32::from_rgb(0xB9, 0xBD, 0xBB),
};

/// Línea oscura en los bordes superior e izquierdo del cristal, para dar
/// sensación de profundidad.
const GLASS_INNER_EDGE: Color32 = Color32::from_rgba_unmultiplied_const(0, 0, 0, 60);

/// Fracción del paso de la rejilla que ocupa cada punto (el resto es la
/// separación entre puntos).
const DOT_FILL: f32 = 0.86;

/// Desplazamiento de la sombra de cada punto, en fracción del paso.
const SHADOW_OFFSET: f32 = 0.22;

/// Distancia (en pasos) desde el borde superior de la matriz hasta el
/// borde superior de la banda de indicadores. Medido sobre la máquina
/// real: la banda de indicadores está 8 unidades por encima de la matriz
/// con un paso de 2.36 unidades.
const BAND_TOP: f32 = 3.4;

/// Filas de la tipografía de los indicadores.
const GLYPH_ROWS: u8 = 5;

/// Margen del cristal alrededor de matriz e indicadores en
/// [`LcdGeometry::fit`], en pasos: izquierda, derecha, arriba (sobre la
/// banda de indicadores) y abajo.
const GLASS_MARGIN_LEFT: f32 = 3.0;
const GLASS_MARGIN_RIGHT: f32 = 3.0;
const GLASS_MARGIN_TOP: f32 = 1.6;
const GLASS_MARGIN_BOTTOM: f32 = 2.5;

/// Ancho total, en pasos, que ocupan matriz e indicadores: el indicador
/// de pila (`BATTERY`) empieza en la media celda 314 y mide 4, es decir
/// llega al paso 159, más allá del final de la matriz (156).
const CONTENT_WIDTH: f32 = 159.0;

/// Tamaño del LCD en [`LcdGeometry::fit`], en pasos.
const FIT_WIDTH: f32 = GLASS_MARGIN_LEFT + CONTENT_WIDTH + GLASS_MARGIN_RIGHT;
const FIT_HEIGHT: f32 = GLASS_MARGIN_TOP + BAND_TOP + 7.0 + GLASS_MARGIN_BOTTOM;

/// Dónde se pinta el LCD, en puntos lógicos de egui.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LcdGeometry {
    /// Rectángulo del cristal (el marco se pinta justo por fuera).
    pub glass: Rect,
    /// Esquina superior izquierda nominal de la matriz de 156x7.
    pub matrix_origin: Pos2,
    /// Distancia nominal entre puntos consecutivos.
    pub pitch: f32,
}

impl LcdGeometry {
    /// Geometría de un LCD "suelto" que ocupa exactamente `rect` (de
    /// proporciones [`LcdGeometry::fit_size`]).
    #[must_use]
    pub fn fit(rect: Rect) -> Self {
        let pitch = (rect.width() / FIT_WIDTH).min(rect.height() / FIT_HEIGHT);
        let size = Self::fit_size(pitch);
        let glass = Rect::from_center_size(rect.center(), size);
        let matrix_origin =
            glass.min + vec2(GLASS_MARGIN_LEFT * pitch, (GLASS_MARGIN_TOP + BAND_TOP) * pitch);
        Self { glass, matrix_origin, pitch }
    }

    /// Tamaño del LCD suelto de [`LcdGeometry::fit`] para un paso dado.
    #[must_use]
    pub fn fit_size(pitch: f32) -> Vec2 {
        vec2(FIT_WIDTH * pitch, FIT_HEIGHT * pitch)
    }
}

/// Rejilla de la matriz ajustada a píxeles físicos, en puntos lógicos.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Grid {
    /// Esquina superior izquierda real de la matriz (ya centrada y
    /// alineada a píxel físico).
    origin: Pos2,
    /// Paso real entre puntos.
    pitch: f32,
    /// Lado de cada punto.
    dot: f32,
}

/// Ajusta la rejilla a píxeles físicos para que los puntos no "tiemblen"
/// (unos de un píxel más que otros) a escalas fraccionarias: paso entero
/// en píxeles físicos (mínimo 2, para que siempre haya separación),
/// punto de [`DOT_FILL`] del paso (mínimo 1 píxel y siempre al menos 1
/// píxel de separación), y la matriz resultante centrada dentro del
/// rectángulo nominal de `156 × pitch` por `7 × pitch`.
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

/// Redondea una coordenada al píxel físico más cercano.
fn snap(v: f32, ppp: f32) -> f32 {
    (v * ppp).round() / ppp
}

/// Un glifo de la tipografía de los indicadores: `width` celdas de ancho
/// y 5 filas; cada fila es una máscara de bits con el bit más
/// significativo (de los `width`) a la izquierda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Glyph {
    width: u8,
    rows: [u8; GLYPH_ROWS as usize],
}

const fn g3(rows: [u8; 5]) -> Glyph {
    Glyph { width: 3, rows }
}

/// Tipografía propia de 3x5 celdas (algunas letras más anchas), diseñada
/// para estos indicadores — NO es la de `PockEmul` (GPL). Solo incluye las
/// letras que hacen falta; cualquier otra es un hueco.
const fn glyph(c: u8) -> Glyph {
    match c {
        b'A' => g3([0b010, 0b101, 0b111, 0b101, 0b101]),
        b'B' => g3([0b110, 0b101, 0b110, 0b101, 0b110]),
        b'D' => g3([0b110, 0b101, 0b101, 0b101, 0b110]),
        b'E' => g3([0b111, 0b100, 0b110, 0b100, 0b111]),
        b'F' => g3([0b111, 0b100, 0b110, 0b100, 0b100]),
        b'G' => Glyph { width: 4, rows: [0b0111, 0b1000, 0b1011, 0b1001, 0b0110] },
        b'H' => g3([0b101, 0b101, 0b111, 0b101, 0b101]),
        // La "I" dentro de una palabra (SHIFT) es un simple palo.
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

/// Números romanos de los niveles de RESERVE, con sus barras de remate
/// arriba y abajo.
const ROMAN_I: Glyph = g3([0b111, 0b010, 0b010, 0b010, 0b111]);
const ROMAN_II: Glyph = Glyph { width: 5, rows: [0b11111, 0b01010, 0b01010, 0b01010, 0b11111] };
const ROMAN_III: Glyph = Glyph {
    width: 7,
    rows: [0b111_1111, 0b010_1010, 0b010_1010, 0b010_1010, 0b111_1111],
};
/// Símbolo de pila.
const BATTERY: Glyph = Glyph { width: 4, rows: [0b0110, 0b1111, 0b1001, 0b1001, 0b1111] };

/// Cómo se dibuja un indicador.
#[derive(Clone, Copy, Debug)]
enum Label {
    /// Una palabra con la tipografía de [`glyph`], una celda en blanco
    /// entre letras.
    Word(&'static str),
    /// Un único glifo especial.
    Glyph(Glyph),
}

/// Un indicador: qué símbolo es, dónde empieza (en medias celdas = medios
/// pasos de la matriz, desde su borde izquierdo), cómo se dibuja y si su
/// fantasma se muestra.
#[derive(Clone, Copy, Debug)]
struct Annunciator {
    symbol: Symbol,
    x: u16,
    label: Label,
    /// `false` para los segmentos que este modelo nunca enciende (KANA en
    /// el modelo de exportación, que es el emulado; la pila, que no
    /// emulamos): se pintan solo encendidos, nunca como fantasma, para no
    /// enseñar un segmento que el usuario no reconocería.
    ghost: bool,
}

/// Los 15 indicadores en su posición real. `DE`, `G` y `RAD` van pegados
/// (una celda de separación) para leerse como "DEG" (`DE`+`G`), "RAD" o
/// "GRAD" (`G`+`RAD`), como en la máquina.
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

/// Celdas encendidas `(x, y)` de un indicador, relativas a su esquina
/// superior izquierda, y su ancho total en celdas.
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

/// Celdas de cada indicador de [`ANNUNCIATORS`], ya en coordenadas de
/// banda (medias celdas desde el borde izquierdo de la matriz),
/// calculadas una sola vez.
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

/// Pinta el LCD entero (cristal, matriz e indicadores) en `geometry`.
/// `ghost` activa los fantasmas de los puntos e indicadores apagados.
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
                // Apagado y sin fantasmas: solo se ve el cristal.
            }
        }
    }

    // Indicadores: celdas de medio paso, pegadas unas a otras (son
    // segmentos impresos macizos, no puntos), con sus bordes alineados a
    // píxel físico para que no queden huecos ni solapes.
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
                // Apagado y sin fantasma: no se pinta nada.
            }
        }
    }

    for mesh in [ghost_mesh, shadow_mesh, on_mesh] {
        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }
}

/// Cristal: degradado vertical, marco plateado y una línea oscura arriba
/// y a la izquierda para dar profundidad.
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

/// Reserva en `ui` un LCD suelto de `pitch` puntos por paso (o menos, si
/// no cabe a lo ancho) y lo pinta con [`STN_GREY`].
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
            assert!(a.x >= previous_end, "{:?} solapa con el anterior", a.symbol);
            previous_end = a.x + width_of(a.label);
        }
        // `CONTENT_WIDTH` pasos = el doble en medias celdas.
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
        // D: 110 / 101 / 101 / 101 / 110 y E desde la columna 4.
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
                assert!((pitch_px - pitch_px.round()).abs() < 1e-4, "paso {pitch_px} px");
                assert!((dot_px - dot_px.round()).abs() < 1e-4, "punto {dot_px} px");
                assert!(pitch_px >= 2.0 && dot_px >= 1.0 && dot_px < pitch_px);
                assert!(grid.pitch <= pitch.max(2.0 / ppp));
                let ox = grid.origin.x * ppp;
                assert!((ox - ox.round()).abs() < 1e-3, "origen {ox} px");
            }
        }
    }

    #[test]
    fn grid_is_centered_in_the_nominal_rect() {
        let grid = snap_grid(pos2(0.0, 0.0), 6.5, 1.0);
        assert!((grid.pitch - 6.0).abs() < 1e-6);
        // 156 × 0.5 / 2 = 39 puntos de holgura a cada lado.
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
