//! LCD de la PC-1500: la matriz de 156x7 puntos y los 15 indicadores
//! ("annunciators") impresos encima, tal como los vería alguien mirando
//! la máquina real.
//!
//! Todo sale de la RAM de pantalla ($7600-$764F y $7700-$774D) más dos
//! señales de la CPU: `BF` (máquina encendida) y `DISP` (LCD activado,
//! instrucciones `SDP`/`RDP`). Ver [`Pc1500::update_display_buffer`] y
//! [`Pc1500::display`] para cómo se combinan.

use crate::Pc1500;

pub const DISPLAY_WIDTH: usize = 156;
pub const DISPLAY_HEIGHT: usize = 7;
pub const RGBA_SIZE: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT * 4; // RGBA format

/// Color RGBA de un punto encendido en [`DisplayController::rgba_buffer`]:
/// el gris casi negro de un LCD STN real, no negro puro.
pub const LCD_RGBA_ON: [u8; 4] = [0x1F, 0x26, 0x28, 0xFF];

/// Color RGBA de un punto apagado (el fondo gris verdoso del cristal) en
/// [`DisplayController::rgba_buffer`]. Antes era blanco puro.
pub const LCD_RGBA_OFF: [u8; 4] = [0xA9, 0xB3, 0xA0, 0xFF];

/// Número de indicadores del LCD, incluido `Battery` (ver [`Symbol`]).
pub const SYMBOL_COUNT: usize = 15;

/// Los indicadores impresos en el cristal del LCD, encima de la matriz de
/// puntos, en el orden en que aparecen de izquierda a derecha.
///
/// `De`, `G` y `Rad` son TRES segmentos distintos, no dos: en la máquina
/// real están pegados uno a continuación del otro y se leen como "DEG"
/// (`De` + `G`), "RAD" (`Rad` solo) o "GRAD" (`G` + `Rad`). Antes el enum
/// solo tenía `Deg`/`Rad` y el bit de `G` acababa sobrescrito por el de
/// `RAD`, así que `GRAD` y `DEG` eran indistinguibles.
///
/// `Battery` (pila baja) no lo controla ningún bit de la RAM de pantalla
/// ni ninguna otra señal que emulemos: siempre está apagado. Se mantiene
/// la variante para que la GUI pueda dibujar su segmento (o soportarlo en
/// el futuro). Antes se forzaba siempre a encendido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Symbol {
    Busy,
    Shift,
    Kana,
    Small,
    De,
    G,
    Rad,
    Run,
    Pro,
    Reserve,
    Def,
    RomanI,
    RomanII,
    RomanIII,
    Battery,
}

impl Symbol {
    /// Todos los indicadores, en el mismo orden que el enum (`ALL[i] as
    /// usize == i`).
    pub const ALL: [Self; SYMBOL_COUNT] = [
        Self::Busy,
        Self::Shift,
        Self::Kana,
        Self::Small,
        Self::De,
        Self::G,
        Self::Rad,
        Self::Run,
        Self::Pro,
        Self::Reserve,
        Self::Def,
        Self::RomanI,
        Self::RomanII,
        Self::RomanIII,
        Self::Battery,
    ];
}

/// Qué bit de qué byte de la RAM de pantalla enciende cada indicador:
/// `(indicador, dirección, máscara)`. Un bit a 1 = indicador ENCENDIDO.
///
/// Antes `display.rs` lo interpretaba al revés (encendido con el bit a
/// 0), y al arrancar mostraba BUSY, SHIFT, KANA, SML, II, III, DEF, RAD,
/// RESERVE... justo lo contrario de la máquina real. Cada bit está
/// confirmado en la propia ROM A04:
///
/// - `RESET_5` ($E04D-$E054) escribe $41 en $764E (BUSY + I) y $43 en
///   $764F (RUN + DE + G); `COLD_START_2` ($CA24) escribe $23 en $764F
///   (PRO + DE + G). El prompt de arranque es, pues, `[$40, $23]`: "I",
///   "DEG" y "PRO".
/// - BUSY ($764E bit 0): `ORI (DISP_BUFF + $4E),$01` en $C8D5, $CA1D y
///   $CCC3 al empezar a ejecutar; `ANI ...,$FE` en $CA88 al terminar.
/// - SHIFT ($02) y DEF ($80): `AUTO_OFF_15` ($E3E8, `EAI $02` / `ANI
///   $7F`) conmuta SHIFT y apaga DEF; `AUTO_OFF_16` ($E3EF, `EAI $80` /
///   `ANI $FD`) al revés.
/// - KANA ($04) y SMALL ($08): `AUTO_OFF_17` ($E3F6) mira PB3 del LH5810
///   (modelo de exportación, que es el nuestro, ver `Pc1500::step`) y
///   entonces conmuta SMALL ($E40C, `EAI $08`) y nunca KANA.
/// - III ($10), II ($20), I ($40): `RSV_CHNG` ($CB69-$CB78) rota
///   $40 → $20 → $10 → $40; `RES_KEY_CODE` ($CE8E) y su tabla de $CEC4
///   confirman qué nivel es cada bit.
/// - DE ($764F bit 0), G ($02), RAD ($04): `BCMD_DEGREE` ($C697) escribe
///   $03, `BCMD_RADIAN` ($C6A4) $04 y `BCMD_GRAD` ($C6A8) $06; la propia
///   ROM vuelve a leerlos para saber la unidad angular
///   (`ARX_MINUS_ARV_20`, $F85A).
/// - RESERVE ($10), PRO ($20), RUN ($40): `BTN_MODE` ($CBA0) y
///   `BTN_SHMODE` ($CB9C). Los bits $08 y $80 de $764F no se usan.
///
/// Coincide con `Lcdc_pc1500.cpp` de `PockEmul` (`COLOR(bit && On)`).
const SYMBOL_BITS: [(Symbol, u32, u8); SYMBOL_COUNT - 1] = [
    (Symbol::Busy, 0x764E, 0x01),
    (Symbol::Shift, 0x764E, 0x02),
    (Symbol::Kana, 0x764E, 0x04),
    (Symbol::Small, 0x764E, 0x08),
    (Symbol::RomanIII, 0x764E, 0x10),
    (Symbol::RomanII, 0x764E, 0x20),
    (Symbol::RomanI, 0x764E, 0x40),
    (Symbol::Def, 0x764E, 0x80),
    (Symbol::De, 0x764F, 0x01),
    (Symbol::G, 0x764F, 0x02),
    (Symbol::Rad, 0x764F, 0x04),
    (Symbol::Reserve, 0x764F, 0x10),
    (Symbol::Pro, 0x764F, 0x20),
    (Symbol::Run, 0x764F, 0x40),
];

/// Decodifica las 156 columnas de la matriz del LCD a partir de la RAM de
/// pantalla, leída con `read` (dirección → byte).
///
/// Cada columna es un byte
/// con el bit 0 = fila superior y el bit 6 = fila inferior (el bit 7
/// siempre a 0), la misma convención que `Pc1500::lcd_columns` del arnés
/// y que la tabla de caracteres de la ROM.
///
/// La RAM de pantalla son dos bloques de 78 bytes, $7600-$764D y
/// $7700-$774D, en parejas par/impar: el byte par da las filas 0-3 y el
/// impar las 4-6, cada uno en un nibble. El nibble BAJO de la pareja
/// `ind`/`ind+1` es la columna `ind/2` (bloque $7600) o `ind/2 + 39`
/// (bloque $7700); el nibble ALTO, esa misma columna + 78. Es decir:
/// columnas 0-38 = nibbles bajos de $7600, 39-77 = bajos de $7700, 78-116
/// = altos de $7600 y 117-155 = altos de $7700. Idéntico a `PockEmul`.
pub fn decode_columns<F: Fn(u32) -> u8>(read: F) -> [u8; DISPLAY_WIDTH] {
    let mut columns = [0; DISPLAY_WIDTH];
    for ind in (0..0x4E_u32).step_by(2) {
        for (base, first_x) in [(0x7600_u32, 0_usize), (0x7700, 39)] {
            let even = read(base | ind);
            let odd = read((base | ind) + 1);
            let x = first_x + (ind as usize >> 1);
            columns[x] = ((even & 0x0F) | ((odd & 0x0F) << 4)) & 0x7F;
            columns[x + 78] = ((even >> 4) | (odd & 0xF0)) & 0x7F;
        }
    }
    columns
}

/// Estado visible del LCD tras la última llamada a
/// [`Pc1500::display`] / [`Pc1500::update_display_buffer`].
#[derive(Debug, Clone)]
pub struct DisplayController {
    /// Buffer RGBA (156x7, fila a fila) con la paleta [`LCD_RGBA_ON`] /
    /// [`LCD_RGBA_OFF`]. La GUI ya no lo usa (pinta el LCD directamente a
    /// partir de [`DisplayController::columns`]), pero se conserva para
    /// cualquier consumidor que quiera una textura.
    rgba_buffer: [u8; RGBA_SIZE],
    /// Columnas decodificadas de la RAM de pantalla (ver
    /// [`decode_columns`]), estén o no visibles.
    columns: [u8; DISPLAY_WIDTH],
    /// Estado de cada indicador según la RAM de pantalla, indexado por
    /// `Symbol as usize`, esté o no visible el LCD.
    symbol_buffer: [bool; SYMBOL_COUNT],
    /// Si el LCD entero (matriz e indicadores) está encendido.
    lcd_on: bool,
}

impl DisplayController {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rgba_buffer: [0; RGBA_SIZE],
            columns: [0; DISPLAY_WIDTH],
            symbol_buffer: [false; SYMBOL_COUNT],
            lcd_on: false,
        }
    }

    #[must_use]
    pub const fn rgba_buffer(&self) -> &[u8; RGBA_SIZE] {
        &self.rgba_buffer
    }

    /// `true` si el LCD se ve encendido: máquina encendida y `DISP`
    /// activado (ver [`Pc1500::display`]). Apagado, ni los puntos ni los
    /// indicadores se ven, aunque la RAM de pantalla conserve su
    /// contenido.
    #[must_use]
    pub const fn is_on(&self) -> bool {
        self.lcd_on
    }

    /// Las 156 columnas de la matriz tal como están en la RAM de pantalla
    /// (ver [`decode_columns`]). OJO: no tiene en cuenta
    /// [`DisplayController::is_on`]; para lo que realmente se ve, usar
    /// [`DisplayController::is_pixel_on`] o mirar también `is_on()`.
    #[must_use]
    pub const fn columns(&self) -> &[u8; DISPLAY_WIDTH] {
        &self.columns
    }

    /// `true` si el punto de la columna `x` (0-155, de izquierda a
    /// derecha) y la fila `y` (0-6, de arriba abajo) se ve encendido.
    /// Fuera de rango, `false`.
    #[must_use]
    pub fn is_pixel_on(&self, x: usize, y: usize) -> bool {
        self.lcd_on && y < DISPLAY_HEIGHT && self.columns.get(x).is_some_and(|&c| (c >> y) & 1 != 0)
    }

    /// `true` si el indicador `symbol` se ve encendido (siempre `false`
    /// con el LCD apagado, y para [`Symbol::Battery`]).
    #[must_use]
    pub const fn is_symbol_on(&self, symbol: Symbol) -> bool {
        self.lcd_on && self.symbol_buffer[symbol as usize]
    }

    /// Recalcula el estado visible a partir de las columnas e indicadores
    /// ya decodificados y de si el LCD está encendido.
    fn refresh(&mut self, columns: [u8; DISPLAY_WIDTH], symbols: [bool; SYMBOL_COUNT], lcd_on: bool) {
        self.columns = columns;
        self.symbol_buffer = symbols;
        self.lcd_on = lcd_on;
        for (i, pixel) in self.rgba_buffer.chunks_exact_mut(4).enumerate() {
            let (y, x) = (i / DISPLAY_WIDTH, i % DISPLAY_WIDTH);
            let color = if lcd_on && (columns[x] >> y) & 1 != 0 { LCD_RGBA_ON } else { LCD_RGBA_OFF };
            pixel.copy_from_slice(&color);
        }
    }
}

impl Pc1500 {
    /// Vuelve a leer la RAM de pantalla (matriz e indicadores) y deja en
    /// [`DisplayController`] lo que se ve, con el LCD encendido si
    /// `lcd_on`. Con `lcd_on == false` el panel entero, indicadores
    /// incluidos, se ve apagado: en la máquina real todo es un único
    /// cristal gobernado por la misma señal.
    ///
    /// Normalmente no hace falta llamarla: [`Pc1500::display`] lo hace
    /// con el `lcd_on` correcto.
    pub fn update_display_buffer(&mut self, lcd_on: bool) {
        let columns = decode_columns(|addr| self.read_byte(addr));
        let mut symbols = [false; SYMBOL_COUNT];
        for (symbol, addr, mask) in SYMBOL_BITS {
            symbols[symbol as usize] = self.read_byte(addr) & mask != 0;
        }
        self.display.refresh(columns, symbols, lcd_on);
    }
}

impl Default for DisplayController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_symbol_bytes(pc: &mut Pc1500, b0: u8, b1: u8) {
        pc.write_byte(0x764E, b0);
        pc.write_byte(0x764F, b1);
    }

    #[test]
    fn symbol_all_matches_enum_order() {
        for (i, symbol) in Symbol::ALL.iter().enumerate() {
            assert_eq!(*symbol as usize, i, "{symbol:?}");
        }
    }

    #[test]
    fn symbol_bits_are_unique_and_battery_unmapped() {
        for (i, a) in SYMBOL_BITS.iter().enumerate() {
            assert_ne!(a.0, Symbol::Battery, "Battery no tiene bit");
            for b in &SYMBOL_BITS[i + 1..] {
                assert_ne!(a.0, b.0, "indicador repetido");
                assert!(a.1 != b.1 || a.2 != b.2, "bit repetido: {a:?} {b:?}");
            }
        }
    }

    #[test]
    fn all_symbols_off_with_zero_bytes() {
        let mut pc = Pc1500::new();
        set_symbol_bytes(&mut pc, 0x00, 0x00);
        pc.update_display_buffer(true);
        for symbol in Symbol::ALL {
            assert!(!pc.display.is_symbol_on(symbol), "{symbol:?}");
        }
    }

    #[test]
    fn every_mapped_symbol_on_with_all_bits_set() {
        let mut pc = Pc1500::new();
        set_symbol_bytes(&mut pc, 0xFF, 0x77);
        pc.update_display_buffer(true);
        for symbol in Symbol::ALL {
            assert_eq!(pc.display.is_symbol_on(symbol), symbol != Symbol::Battery, "{symbol:?}");
        }
    }

    #[test]
    fn boot_bytes_decode_to_i_deg_pro() {
        let mut pc = Pc1500::new();
        set_symbol_bytes(&mut pc, 0x40, 0x23);
        pc.update_display_buffer(true);
        let on: Vec<Symbol> = Symbol::ALL.into_iter().filter(|&s| pc.display.is_symbol_on(s)).collect();
        assert_eq!(on, [Symbol::De, Symbol::G, Symbol::Pro, Symbol::RomanI]);
    }

    #[test]
    fn lcd_off_hides_pixels_and_symbols() {
        let mut pc = Pc1500::new();
        set_symbol_bytes(&mut pc, 0xFF, 0x77);
        pc.write_byte(0x7600, 0xFF);
        pc.write_byte(0x7601, 0xFF);
        pc.update_display_buffer(false);
        let d = &pc.display;
        assert!(!d.is_on());
        assert_eq!(d.columns()[0], 0x7F, "la RAM sigue ah\u{ed}");
        for x in 0..DISPLAY_WIDTH {
            for y in 0..DISPLAY_HEIGHT {
                assert!(!d.is_pixel_on(x, y));
            }
        }
        for symbol in Symbol::ALL {
            assert!(!d.is_symbol_on(symbol), "{symbol:?}");
        }
        assert!(d.rgba_buffer().chunks_exact(4).all(|p| p == LCD_RGBA_OFF));
    }

    #[test]
    fn column_mapping_and_rgba_palette() {
        let mut pc = Pc1500::new();
        for addr in (0x7600..0x764E).chain(0x7700..0x774E) {
            pc.write_byte(addr, 0);
        }
        // Pareja $7602/$7603: nibbles bajos → columna 1, altos → 79.
        pc.write_byte(0x7602, 0x21); // fila 0 en la columna 1, fila 1 en la 79
        pc.write_byte(0x7603, 0x40); // fila 6 en la columna 79
        // Pareja $774C/$774D (la última): columnas 77 y 155.
        pc.write_byte(0x774C, 0x80); // fila 3 en la columna 155
        pc.write_byte(0x774D, 0x04); // fila 6 en la columna 77
        pc.update_display_buffer(true);
        let d = &pc.display;
        let mut expected = [0_u8; DISPLAY_WIDTH];
        expected[1] = 0x01;
        expected[79] = 0x42;
        expected[155] = 0x08;
        expected[77] = 0x40;
        assert_eq!(d.columns(), &expected);
        assert!(d.is_pixel_on(1, 0));
        assert!(!d.is_pixel_on(1, 1));
        assert!(d.is_pixel_on(79, 6));
        assert!(!d.is_pixel_on(DISPLAY_WIDTH, 0), "fuera de rango");
        assert!(!d.is_pixel_on(1, 7), "fuera de rango");
        let rgba = |x: usize, y: usize| {
            let i = (y * DISPLAY_WIDTH + x) * 4;
            [d.rgba_buffer()[i], d.rgba_buffer()[i + 1], d.rgba_buffer()[i + 2], d.rgba_buffer()[i + 3]]
        };
        assert_eq!(rgba(1, 0), LCD_RGBA_ON);
        assert_eq!(rgba(155, 3), LCD_RGBA_ON);
        assert_eq!(rgba(0, 0), LCD_RGBA_OFF);
        assert_eq!(rgba(155, 6), LCD_RGBA_OFF);
    }
}
