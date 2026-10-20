//! La carátula de la PC-1500 pintada de forma procedural: la vista
//! principal del emulador. Caja, panel de aluminio cepillado, banda
//! superior color bronce (la de la PC-1500 original; la PC-1500A la
//! llevaba negra), el LCD dentro de su marco (pintado por
//! [`lcd_view::paint`]), la tira de las teclas de función y las 65 teclas
//! en su disposición real, con sus colores y los rótulos de `SHIFT`
//! impresos en el panel.
//!
//! Todo se dibuja con primitivas de egui: no se carga ninguna imagen ni
//! hay dependencias nuevas, y no se reproduce ninguna foto ni logotipo
//! (la marca se escribe como texto genérico "PC-1500"). Las únicas dos
//! texturas son generadas aquí mismo una sola vez: las vetas del
//! aluminio cepillado ([`brushed_pixels`]) y el degradado de la banda
//! ([`band_gradient_pixels`]), que como textura respeta las esquinas
//! redondeadas.
//!
//! # Geometría
//! Toda la geometría está en un espacio de referencia de
//! [`REF_W`]×[`REF_H`] unidades: el de una foto de la unidad real (195 ×
//! 86 mm) sobre la que se midieron las posiciones de las teclas. Al
//! pintar, [`Layout::fit`] escala ese espacio al hueco disponible
//! conservando las proporciones y lo centra. Las funciones de geometría
//! ([`cap_rect`], [`CAPS`], [`Layout`]) son puras y tienen tests: cada
//! tecla tiene exactamente una tapa, no se solapan y caben en el panel.
//!
//! # Interacción
//! Cada tapa es una zona de egui (`Ui::interact`, sin foco de teclado).
//! [`Faceplate::show`] solo devuelve sobre qué tecla está pulsado ahora
//! el botón del ratón ([`FaceplateOutput::pointer_key`]); quien llama
//! convierte los cambios en pulsaciones y sueltas del alimentador de
//! teclas del núcleo (`host_key_down`/`host_key_up` con `MOUSE_ID`, ver
//! `Pc1500App::update_mouse_key`). Aquí NO se impone ninguna pulsación
//! mínima en tiempo del host: el alimentador ya garantiza en tiempo
//! emulado que la ROM vea cada tecla, a cualquier velocidad.

use ceres_core::Key;
use eframe::egui::{
    self, Align2, Color32, ColorImage, CornerRadius, FontId, Id, Mesh, Painter, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, TextureHandle, TextureId, TextureOptions, Ui, Vec2, epaint::RectShape, pos2, vec2,
};

use crate::lcd_view::{self, LcdFrame, LcdGeometry, STN_GREY};

/// Ancho y alto del espacio de referencia (ver la documentación del
/// módulo).
pub const REF_W: f32 = 679.0;
pub const REF_H: f32 = 299.0;

// ---------------------------------------------------------------------
// Colores (sacados de fotos de la PC-1500 original).
// ---------------------------------------------------------------------

/// Canto de la caja (gris casi negro).
const CASE_EDGE: Color32 = Color32::from_rgb(0x2A, 0x2C, 0x2E);
/// Sombra de la caja sobre el fondo de la ventana.
const CASE_SHADOW: Color32 = Color32::from_rgba_unmultiplied_const(0, 0, 0, 70);
/// Brillo y sombra de los bordes del panel de aluminio.
const PANEL_HIGHLIGHT: Color32 = Color32::from_rgb(0xE2, 0xE4, 0xE3);
const PANEL_SHADOW: Color32 = Color32::from_rgb(0xB4, 0xB8, 0xB8);
/// Labio oscuro en el borde inferior del panel.
const PANEL_LIP: Color32 = Color32::from_rgb(0x8E, 0x92, 0x92);
/// Texto impreso en el panel (`POWER`, `BREAK`, rótulos de la tira).
const PANEL_TEXT: Color32 = Color32::from_rgb(0x2B, 0x2D, 0x2F);
/// Rótulos de `SHIFT` impresos sobre las teclas.
const LEGEND_ORANGE: Color32 = Color32::from_rgb(0xC8, 0x69, 0x1E);

/// Banda superior color bronce, arriba y abajo del degradado.
const TOP_BAND_HI: Color32 = Color32::from_rgb(0x57, 0x4E, 0x3E);
const TOP_BAND_LO: Color32 = Color32::from_rgb(0x36, 0x30, 0x2A);
/// Filete fino de la banda y su texto.
const TOP_BAND_RULE: Color32 = Color32::from_rgb(0x7A, 0x71, 0x5F);
const TOP_BAND_TEXT: Color32 = Color32::from_rgb(0xD9, 0xD6, 0xCC);
/// Marca y subtítulo de la banda.
const BRAND_TEXT: Color32 = Color32::from_rgb(0xDA, 0xDD, 0xDC);
const BRAND_SUBTEXT: Color32 = Color32::from_rgb(0xD0, 0xD3, 0xD2);

/// Marco plateado del LCD y el fondo oscuro entre marco y cristal.
const LCD_FRAME: Color32 = Color32::from_rgb(0xB9, 0xBD, 0xBB);
const LCD_BEZEL: Color32 = Color32::from_rgb(0x22, 0x1F, 0x1B);
/// Aviso de máquina apagada, escrito tenue sobre el cristal.
const LCD_OFF_TEXT: Color32 = Color32::from_rgba_unmultiplied_const(0x1F, 0x26, 0x28, 130);

/// Tira de las teclas de función y sus separadores.
const STRIP_FILL: Color32 = Color32::from_rgb(0xB4, 0xB8, 0xB7);
const STRIP_DIVIDER: Color32 = Color32::from_rgb(0x9A, 0x9E, 0x9D);

/// Contorno de una tecla de prefijo enganchada (`SHIFT`, `SML`, `DEF`
/// pendientes de la tecla siguiente).
const LATCH_GLOW: Color32 = Color32::from_rgb(0xFF, 0xE2, 0x7A);

// ---------------------------------------------------------------------
// Geometría de referencia.
// ---------------------------------------------------------------------

/// Caja entera y panel de aluminio.
const BODY: Rect = ref_rect(0.0, 0.0, REF_W, REF_H);
const BODY_RADIUS: f32 = 12.0;
const PANEL: Rect = ref_rect(4.0, 4.0, 675.0, 293.0);
const PANEL_RADIUS: f32 = 9.0;
/// Altura del labio oscuro del panel.
const PANEL_LIP_Y: f32 = 292.0;

/// Banda superior, su filete y el texto de la izquierda.
const TOP_BAND: Rect = ref_rect(8.0, 8.0, 671.0, 92.0);
const TOP_BAND_RADIUS: f32 = 4.0;
const TOP_BAND_RULE_Y: f32 = 22.0;
const TOP_BAND_RULE_X: (f32, f32) = (14.0, 665.0);
const TOP_BAND_TEXT_POS: Pos2 = pos2(66.0, 16.0);

/// Marco del LCD, cristal y origen y paso de la matriz de 156×7 (la
/// matriz ocupa 93..461 × 61..77.5 y la banda de indicadores 53..59).
const LCD_FRAME_RECT: Rect = ref_rect(76.0, 38.0, 486.0, 86.0);
const LCD_GLASS: Rect = ref_rect(79.0, 41.0, 483.0, 83.0);
const LCD_MATRIX_ORIGIN: Pos2 = pos2(93.0, 61.0);
const LCD_PITCH: f32 = 2.36;

/// Marca: nombre del modelo y subtítulo, centrados a la derecha del LCD.
const BRAND_X: f32 = 592.0;
const BRAND_Y: f32 = 50.0;
const BRAND_SUB_Y: f32 = 70.0;

/// Tira de las teclas de función (F1..F6), con separadores entre teclas
/// y los rótulos de `SHIFT` (`! " # $ % &`) en la parte de arriba.
const STRIP: Rect = ref_rect(101.0, 95.0, 440.0, 139.0);
const STRIP_DIVIDERS: [f32; 5] = [158.0, 214.5, 271.0, 327.5, 383.5];
const STRIP_LABEL_Y: f32 = 105.0;

/// Grupo de encendido: `POWER` con su raya bajo `OFF`/`ON` y `BREAK`
/// encima de `ON`.
const POWER_LABEL: Pos2 = pos2(548.0, 131.0);
const POWER_LINE_Y: f32 = 139.0;
const POWER_LINE_X: (f32, f32) = (532.0, 652.0);
const BREAK_LABEL: Pos2 = pos2(635.5, 113.0);

/// Distancia, por encima del centro de la tecla, a la que va el rótulo
/// de `SHIFT` impreso en el panel.
const SHIFT_LEGEND_DY: f32 = 15.0;

/// Desplazamiento hacia abajo del canto de la tecla (su sombra) y
/// hundimiento de la cara al pulsarla.
const CAP_RIM_DROP: f32 = 1.5;
const CAP_PRESS_DROP: f32 = 1.0;
const CAP_RIM_RADIUS: f32 = 2.5;
const CAP_FACE_RADIUS: f32 = 2.0;

/// Tamaños de los textos, en unidades de referencia.
const FONT_PANEL: f32 = 5.5;
const FONT_STRIP: f32 = 7.0;
const FONT_SHIFT_LEGEND: f32 = 6.0;
const FONT_CAP_CHAR: f32 = 10.0;
const FONT_CAP_WORD: f32 = 6.5;
const FONT_BRAND: f32 = 15.0;
const FONT_BRAND_SUB: f32 = 6.5;
const FONT_LCD_OFF: f32 = 7.0;

/// Rectángulo de referencia a partir de sus esquinas (`const`, al
/// contrario que `Rect::from_min_max`).
const fn ref_rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect { min: pos2(x0, y0), max: pos2(x1, y1) }
}

/// Altura (centro) de cada fila de teclas.
const ROW_TOP: f32 = 128.0;
const ROW_Q: f32 = 166.5;
const ROW_A: f32 = 199.5;
const ROW_Z: f32 = 233.0;
const ROW_BOTTOM: f32 = 266.0;

/// Tamaños de tapa: la normal, la de la fila de arriba (más baja) y las
/// anchas (`SPACE`, `ENTER`).
const SIZE_STD: Vec2 = vec2(31.0, 20.0);
const SIZE_TOP: Vec2 = vec2(26.0, 14.0);
const SIZE_WIDE: Vec2 = vec2(73.0, 20.0);

/// Una tecla de la carátula: su centro y tamaño en unidades de
/// referencia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapDef {
    pub key: Key,
    pub center: Pos2,
    pub size: Vec2,
}

impl CapDef {
    /// Rectángulo de la cara de la tecla (sin el canto), en unidades de
    /// referencia.
    #[must_use]
    pub fn rect(&self) -> Rect {
        Rect::from_center_size(self.center, self.size)
    }
}

const fn cap(key: Key, cx: f32, cy: f32, size: Vec2) -> CapDef {
    CapDef { key, center: pos2(cx, cy), size }
}

/// Las 65 teclas de la PC-1500 en su sitio: las 64 de la matriz y
/// `ON/BRK`. `DEF` es `Key::Control`. Medidas sobre la foto de la
/// unidad real (centros de las tapas).
pub const CAPS: [CapDef; Key::COUNT] = {
    use Key as K;
    [
        // Fila superior: DEF, F1..F6 (en la tira), SHIFT, OFF, ON.
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
        // Fila inferior.
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

/// La cara de la tecla `key` en unidades de referencia (ver [`CAPS`]).
/// Todas las teclas tienen una; `None` no debería pasar nunca (lo
/// comprueban los tests).
#[must_use]
pub fn cap_rect(key: Key) -> Option<Rect> {
    CAPS.iter().find(|c| c.key == key).map(CapDef::rect)
}

/// Lo que ocupa una tecla incluyendo su canto (que asoma por debajo).
fn cap_outline(rect: Rect) -> Rect {
    Rect::from_min_max(rect.min, rect.max + vec2(0.0, CAP_RIM_DROP))
}

// ---------------------------------------------------------------------
// Aspecto de las teclas.
// ---------------------------------------------------------------------

/// Los cuatro tipos de tapa de la PC-1500.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapStyle {
    /// Letras, paréntesis y `SPACE`: tapas casi negras, rótulo blanco.
    Dark,
    /// Funciones, números, operadores y flechas: gris, rótulo negro.
    Grey,
    /// `CL`: roja.
    Red,
    /// `SHIFT`: naranja.
    Orange,
}

/// Colores de un tipo de tapa.
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

/// El tipo de tapa de cada tecla, como en las fotos de la máquina.
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

/// Dirección de una flecha dibujada.
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

/// El rótulo impreso en una tapa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Legend {
    /// Un solo carácter (letra, número, operador), en grande.
    Char(&'static str),
    /// Una palabra (`DEF`, `SHIFT`, `ENTER`...), en pequeño y negrita.
    Word(&'static str),
    /// Una flecha, dibujada con polígonos (sin depender de la fuente).
    Arrow(Dir),
    /// La doble flecha `⇕` de `RSV`, también dibujada.
    UpDown,
    /// Sin rótulo: F1..F6 lo llevan en la tira de encima.
    Blank,
}

/// El rótulo de cada tecla.
const fn legend(key: Key) -> Legend {
    match key {
        Key::F1 | Key::F2 | Key::F3 | Key::F4 | Key::F5 | Key::F6 => Legend::Blank,
        Key::Left => Legend::Arrow(Dir::Left),
        Key::Right => Legend::Arrow(Dir::Right),
        Key::Up => Legend::Arrow(Dir::Up),
        Key::Down => Legend::Arrow(Dir::Down),
        Key::Rsv => Legend::UpDown,
        // `Key::label` dice "ON/BRK"; en la tapa solo pone ON (BREAK va
        // impreso encima, en el panel).
        Key::On => Legend::Word("ON"),
        Key::Space | Key::Enter | Key::Cl | Key::Control | Key::Mode | Key::Off | Key::Rcl | Key::Shift | Key::Sml => {
            Legend::Word(key.label())
        }
        _ => Legend::Char(key.label()),
    }
}

/// Un rótulo de `SHIFT` impreso en el panel encima de una tecla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanelLegend {
    Text(&'static str),
    /// `√`, dibujado con segmentos (la fuente por defecto puede no
    /// tenerlo).
    Sqrt,
    /// `π`, también dibujado.
    Pi,
}

/// Los rótulos de `SHIFT` impresos en el panel (los de F1..F6 van en la
/// tira, ver [`STRIP_LABEL_Y`]). Los de texto coinciden con
/// `Key::shift_label` (lo comprueban los tests). Encima de ↑ y ↓ la
/// carcasa lleva √ y π: son los códigos $5B y $5D de `KB_LAYOUT_SH`, que
/// desde el teclado del PC se teclean como `[` y `]`. `RESERVE`
/// (`SHIFT`+`MODE`) no va impreso en la carcasa.
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

/// Las teclas de función, en el orden de la tira.
const FUNCTION_KEYS: [Key; 6] = [Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6];

// ---------------------------------------------------------------------
// Escalado.
// ---------------------------------------------------------------------

/// Cómo se coloca el espacio de referencia en la pantalla: origen (la
/// esquina superior izquierda de la caja) y escala (puntos de egui por
/// unidad de referencia).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub origin: Pos2,
    pub scale: f32,
}

impl Layout {
    /// La caja lo más grande posible dentro de `avail`, sin deformarla y
    /// centrada.
    #[must_use]
    pub fn fit(avail: Rect) -> Self {
        let scale = (avail.width() / REF_W).min(avail.height() / REF_H).max(0.0);
        let size = vec2(REF_W, REF_H) * scale;
        Self { origin: avail.center() - size / 2.0, scale }
    }

    /// Un punto de referencia en la pantalla.
    #[must_use]
    pub fn pos(self, p: Pos2) -> Pos2 {
        self.origin + p.to_vec2() * self.scale
    }

    /// Un rectángulo de referencia en la pantalla.
    #[must_use]
    pub fn rect(self, r: Rect) -> Rect {
        Rect::from_min_max(self.pos(r.min), self.pos(r.max))
    }

    /// Una longitud de referencia en puntos.
    #[must_use]
    pub fn len(self, v: f32) -> f32 {
        v * self.scale
    }

    /// Un desplazamiento de referencia en puntos.
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

/// Tamaño de la textura del aluminio cepillado.
pub const BRUSHED_W: usize = 512;
pub const BRUSHED_H: usize = 128;

/// Generador congruencial lineal (el de Knuth, MMIX): determinista, así
/// la carátula es siempre la misma.
struct Lcg(u64);

impl Lcg {
    const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    /// Un número en `0..n`, con los bits altos (los buenos de un LCG) y
    /// sin módulo.
    fn below(&mut self, n: u32) -> u32 {
        let high = self.next() >> 32;
        u32::try_from((high * u64::from(n)) >> 32).unwrap_or(0)
    }
}

/// Píxeles (fila a fila) del aluminio cepillado, [`BRUSHED_W`] ×
/// [`BRUSHED_H`]: cada fila es una veta horizontal con su propio gris
/// (200..214), modulado a lo largo de la fila por una onda lenta de ±3
/// con fase al azar, un grano de ±1 por píxel y un toque cálido (+2 en
/// rojo). Se estira sobre todo el panel.
#[must_use]
pub fn brushed_pixels() -> Vec<Color32> {
    let mut rng = Lcg(0x5043_2D31_3530_3021);
    let width = u32::try_from(BRUSHED_W).unwrap_or(u32::MAX);
    let mut pixels = Vec::with_capacity(BRUSHED_W * BRUSHED_H);
    for _ in 0..BRUSHED_H {
        let grey = 200 + i32::try_from(rng.below(15)).unwrap_or(0);
        let phase = rng.below(width);
        for x in 0..width {
            // Onda triangular de periodo `width`: 0..=256..0 → -3..=3.
            let t = i32::try_from((x + phase) & (width - 1)).unwrap_or(0);
            let wave = ((t - 256).abs() * 6) / 256 - 3;
            let grain = i32::try_from(rng.below(3)).unwrap_or(1) - 1;
            let v = grey + wave + grain;
            pixels.push(Color32::from_rgb(channel(v + 2), channel(v), channel(v)));
        }
    }
    pixels
}

/// Píxeles (una columna de [`BAND_STEPS`]) del degradado vertical de la
/// banda superior, de [`TOP_BAND_HI`] a [`TOP_BAND_LO`].
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

/// Alto de la textura del degradado de la banda.
pub const BAND_STEPS: usize = 64;

/// Un canal de color recortado a 0..=255.
fn channel(v: i32) -> u8 {
    u8::try_from(v.clamp(0, 255)).unwrap_or(u8::MAX)
}

// ---------------------------------------------------------------------
// Pintado.
// ---------------------------------------------------------------------

/// Lo que necesita la carátula para pintarse en un repintado.
pub struct FaceplateInput<'a> {
    /// Lo que muestra el LCD.
    pub lcd: &'a LcdFrame,
    /// Fantasmas del LCD (ver `lcd_view::paint`).
    pub ghost: bool,
    /// La máquina está apagada: se escribe un aviso tenue en el cristal.
    pub powered_off: Option<&'a str>,
    /// Teclas que se ven hundidas (pulsadas en el teclado emulado, con
    /// el ratón o con el teclado del PC), indexadas por `Key as usize`.
    pub held: &'a [bool; Key::COUNT],
    /// Prefijos enganchados (`SHIFT`, `SML`, `DEF`), con un contorno.
    pub latched: &'a [bool; Key::COUNT],
    /// Texto de ayuda de cada tecla (qué tecla del PC la teclea).
    pub hint: &'a dyn Fn(Key) -> Option<String>,
}

/// Resultado de [`Faceplate::show`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FaceplateOutput {
    /// La tecla sobre la que está pulsado ahora el botón del ratón (se
    /// mantiene aunque el puntero salga de ella sin soltar).
    pub pointer_key: Option<Key>,
}

/// La carátula, con sus texturas generadas la primera vez que se pinta.
#[derive(Default)]
pub struct Faceplate {
    brushed: Option<TextureHandle>,
    band: Option<TextureHandle>,
}

impl Faceplate {
    /// Pinta la carátula ocupando todo el espacio libre de `ui` y
    /// devuelve qué tecla tiene pulsada el ratón.
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

    /// Las dos texturas procedurales, creadas una sola vez.
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

/// UV de una textura entera.
const FULL_UV: Rect = ref_rect(0.0, 0.0, 1.0, 1.0);

/// Radio de esquina en puntos (egui los guarda en `u8`).
fn radius(layout: Layout, r: f32) -> CornerRadius {
    CornerRadius::from(layout.len(r))
}

/// Caja con su sombra, panel de aluminio cepillado con brillo arriba,
/// sombra abajo y el labio oscuro del borde inferior.
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

/// Banda superior bronce (degradado como textura, para conservar las
/// esquinas redondeadas), su filete y el texto de la izquierda.
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

/// Geometría del LCD de la carátula en la pantalla.
fn lcd_geometry(layout: Layout) -> LcdGeometry {
    LcdGeometry {
        glass: layout.rect(LCD_GLASS),
        matrix_origin: layout.pos(LCD_MATRIX_ORIGIN),
        pitch: layout.len(LCD_PITCH),
    }
}

/// Marco plateado, cristal y contenido del LCD (y el aviso de apagada).
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

/// Lo impreso en el panel: la tira de las teclas de función con sus
/// rótulos, el grupo de encendido y los rótulos de `SHIFT`.
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

/// `√` y `π` como segmentos, relativos al centro del rótulo.
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

/// Texto en negrita "falsa": dos veces, la segunda medio paso a la
/// derecha (la fuente por defecto de egui no tiene negrita).
fn bold_text(painter: &Painter, layout: Layout, pos: Pos2, text: &str, size: f32, color: Color32) {
    let p = layout.pos(pos);
    let font = layout.font(size);
    painter.text(p, Align2::CENTER_CENTER, text, font.clone(), color);
    let dx = layout.len(size * 0.035).max(0.5);
    painter.text(p + vec2(dx, 0.0), Align2::CENTER_CENTER, text, font, color);
}

/// Cómo se ve una tecla en este repintado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CapState {
    pressed: bool,
    latched: bool,
    hovered: bool,
}

/// Cara de una tecla en la pantalla: hundida si está pulsada.
fn face_rect(layout: Layout, cap: &CapDef, pressed: bool) -> Rect {
    let drop = if pressed { CAP_PRESS_DROP } else { 0.0 };
    layout.rect(cap.rect().translate(vec2(0.0, drop)))
}

/// Una tecla: canto (sombra) desplazado hacia abajo, cara, brillo en el
/// borde superior y rótulo. Pulsada, la cara baja y se oscurece (y el
/// brillo casi desaparece), como una tecla hundida.
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

/// El rótulo de una tapa, centrado en su cara.
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
            // Las dos líneas del cuerpo de `⇕`.
            let stroke = Stroke::new(layout.len(0.8).max(1.0), color);
            for dx in [-1.0, 1.0] {
                let x = layout.len(dx);
                painter.line_segment([center - gap + vec2(x, 0.0), center + gap + vec2(x, 0.0)], stroke);
            }
        }
        Legend::Blank => {}
    }
}

/// Flecha con astil: cabeza triangular más un rectángulo, centrada en
/// `center` y apuntando a `dir`.
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

/// Triángulo centrado en `center` (a lo largo de `dir`) apuntando a
/// `dir`.
fn triangle(center: Pos2, dir: Vec2, length: f32, half: f32, color: Color32) -> Shape {
    triangle_at(center - dir * (length / 2.0), dir, length, half, color)
}

/// Triángulo con la base centrada en `base` y la punta a `length` en la
/// dirección `dir`.
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
            assert_eq!(count, 1, "{key:?} en la car\u{e1}tula");
            assert!(cap_rect(key).is_some(), "{key:?} sin tapa");
        }
    }

    #[test]
    fn caps_do_not_overlap() {
        for (i, a) in CAPS.iter().enumerate() {
            for b in &CAPS[i + 1..] {
                assert!(
                    !outline(a).intersects(outline(b)),
                    "{:?} y {:?} se solapan",
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
            assert!(PANEL.contains_rect(r), "{:?} fuera del panel", cap.key);
            assert!(BODY.contains_rect(r), "{:?} fuera de la caja", cap.key);
            assert!(r.top() > TOP_BAND.bottom(), "{:?} pisa la banda superior", cap.key);
        }
    }

    #[test]
    fn function_keys_sit_on_the_strip_between_dividers() {
        for (i, key) in FUNCTION_KEYS.iter().enumerate() {
            let r = cap_rect(*key).map(cap_outline).unwrap_or(Rect::NOTHING);
            assert!(STRIP.contains_rect(r), "{key:?} fuera de la tira");
            let left = if i == 0 { STRIP.left() } else { STRIP_DIVIDERS[i - 1] };
            let right = STRIP_DIVIDERS.get(i).copied().unwrap_or(STRIP.right());
            assert!(r.left() > left && r.right() < right, "{key:?} pisa un separador");
            // Los rótulos de la tira, encima de la tecla.
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
                assert!(!outline(cap).intersects(area), "el r\u{f3}tulo de {key:?} pisa {:?}", cap.key);
            }
        }
    }

    #[test]
    fn lcd_matrix_and_annunciators_fit_the_glass() {
        assert!(LCD_FRAME_RECT.contains_rect(LCD_GLASS));
        assert!(TOP_BAND.contains_rect(LCD_FRAME_RECT));
        // 156 columnas de matriz; los indicadores llegan al paso 159 y
        // su banda empieza 3.4 pasos por encima.
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
        // Ningún rótulo de tapa depende de glifos no ASCII de la fuente.
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
        assert!((layout.scale - 2.0).abs() < 1e-4, "limita el ancho");
        let body = layout.rect(BODY);
        assert!((body.center() - avail.center()).length() < 1e-3);
        assert!(avail.expand(1e-3).contains_rect(body));

        let tall = Layout::fit(Rect::from_min_size(Pos2::ZERO, vec2(5000.0, 299.0)));
        assert!((tall.scale - 1.0).abs() < 1e-4, "limita el alto");
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
            assert_eq!(p.r(), p.g() + 2, "toque c\u{e1}lido");
            assert_eq!(p.b(), p.g());
        }
        // Vetas: no todas las filas son iguales.
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
