//! The PC-1500's LCD: the 156x7 dot matrix and the 15 indicators ("annunciators") printed above it,
//! as someone looking at the real machine would see them.
//!
//! Everything comes from the display RAM ($7600-$764F and $7700-$774D) plus two CPU signals: `BF`
//! (machine on) and `DISP` (LCD enabled, instructions `SDP`/`RDP`). See
//! [`Pc1500::update_display_buffer`] and [`Pc1500::display`] for how they combine.

use crate::Pc1500;

pub const DISPLAY_WIDTH: usize = 156;
pub const DISPLAY_HEIGHT: usize = 7;
pub const RGBA_SIZE: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT * 4; // RGBA format

/// RGBA color of a lit dot in [`DisplayController::rgba_buffer`]: the almost black grey of a real
/// STN LCD, not pure black.
pub const LCD_RGBA_ON: [u8; 4] = [0x1F, 0x26, 0x28, 0xFF];

/// RGBA color of an unlit dot (the glass's greenish grey background) in
/// [`DisplayController::rgba_buffer`]. It used to be pure white.
pub const LCD_RGBA_OFF: [u8; 4] = [0xA9, 0xB3, 0xA0, 0xFF];

/// Number of LCD indicators, `Battery` included (see [`Symbol`]).
pub const SYMBOL_COUNT: usize = 15;

/// The indicators printed on the LCD's glass, above the dot matrix, in the order they appear from
/// left to right.
///
/// `De`, `G` and `Rad` are THREE different segments, not two: on the real machine they sit one
/// after the other and read "DEG" (`De` + `G`), "RAD" (`Rad` alone) or "GRAD" (`G` + `Rad`). The
/// enum used to have only `Deg`/`Rad`, and the bit of `G` ended up overwritten by `RAD`'s, so
/// `GRAD` and `DEG` could not be told apart.
///
/// `Battery` (low battery) is not driven by any bit of the display RAM or any other signal we
/// emulate: it is always off. The variant is kept so that the GUI can draw its segment (or support
/// it in the future). It used to be forced on.
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
    /// All the indicators, in the same order as the enum (`ALL[i] as usize == i`).
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

/// Which bit of which byte of the display RAM lights each indicator: `(indicator, address, mask)`.
/// A bit at 1 = indicator ON.
///
/// `display.rs` used to read it the other way round (on with the bit at 0), and at boot showed
/// BUSY, SHIFT, KANA, SML, II, III, DEF, RAD, RESERVE... just the opposite of the real machine.
/// Every bit is confirmed in the A04 ROM itself:
///
/// - `RESET_5` ($E04D-$E054) writes $41 to $764E (BUSY + I) and $43 to $764F (RUN + DE + G);
///   `COLD_START_2` ($CA24) writes $23 to $764F (PRO + DE + G). The boot prompt is therefore
///   `[$40, $23]`: "I", "DEG" and "PRO".
/// - BUSY ($764E bit 0): `ORI (DISP_BUFF + $4E),$01` at $C8D5, $CA1D and $CCC3 when it starts
///   running; `ANI ...,$FE` at $CA88 when it ends.
/// - SHIFT ($02) and DEF ($80): `AUTO_OFF_15` ($E3E8, `EAI $02` / `ANI $7F`) toggles SHIFT and
///   clears DEF; `AUTO_OFF_16` ($E3EF, `EAI $80` / `ANI $FD`) the other way round.
/// - KANA ($04) and SMALL ($08): `AUTO_OFF_17` ($E3F6) looks at the LH5810's PB3 (export model,
///   which is ours, see `Pc1500::step`) and then toggles SMALL ($E40C, `EAI $08`) and never KANA.
/// - III ($10), II ($20), I ($40): `RSV_CHNG` ($CB69-$CB78) rotates $40 → $20 → $10 → $40;
///   `RES_KEY_CODE` ($CE8E) and its table at $CEC4 confirm which level each bit is.
/// - DE ($764F bit 0), G ($02), RAD ($04): `BCMD_DEGREE` ($C697) writes $03, `BCMD_RADIAN` ($C6A4)
///   $04 and `BCMD_GRAD` ($C6A8) $06; the ROM itself reads them back to know the angle unit
///   (`ARX_MINUS_ARV_20`, $F85A).
/// - RESERVE ($10), PRO ($20), RUN ($40): `BTN_MODE` ($CBA0) and `BTN_SHMODE` ($CB9C). Bits $08 and
///   $80 of $764F are not used.
///
/// The same as `PockEmul`'s `Lcdc_pc1500.cpp` (`COLOR(bit && On)`).
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

/// Decodes the 156 columns of the LCD's matrix from the display RAM, read with `read` (address →
/// byte).
///
/// Each column is a byte with bit 0 = top row and bit 6 = bottom row (bit 7 always 0), the same
/// convention as the harness's `Pc1500::lcd_columns` and as the ROM's character table.
///
/// The display RAM is two blocks of 78 bytes, $7600-$764D and $7700-$774D, in even/odd pairs: the
/// even byte gives rows 0-3 and the odd one rows 4-6, each in a nibble. The LOW nibble of the pair
/// `ind`/`ind+1` is column `ind/2` (block $7600) or `ind/2 + 39` (block $7700); the HIGH nibble,
/// that same column + 78. That is: columns 0-38 = low nibbles of $7600, 39-77 = low ones of $7700,
/// 78-116 = high ones of $7600 and 117-155 = high ones of $7700. The same as `PockEmul`.
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

/// What the LCD shows after the last call to [`Pc1500::display`] /
/// [`Pc1500::update_display_buffer`].
#[derive(Debug, Clone)]
pub struct DisplayController {
    /// RGBA buffer (156x7, row by row) with the palette [`LCD_RGBA_ON`] / [`LCD_RGBA_OFF`]. The GUI
    /// no longer uses it (it paints the LCD straight from [`DisplayController::columns`]), but it
    /// is kept for anyone who wants a texture.
    rgba_buffer: [u8; RGBA_SIZE],
    /// Columns decoded from the display RAM (see [`decode_columns`]), whether visible or not.
    columns: [u8; DISPLAY_WIDTH],
    /// State of each indicator according to the display RAM, indexed by `Symbol as usize`, whether
    /// the LCD is visible or not.
    symbol_buffer: [bool; SYMBOL_COUNT],
    /// Whether the whole LCD (matrix and indicators) is on.
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

    /// `true` if the LCD shows: machine on and `DISP` enabled (see [`Pc1500::display`]). Off,
    /// neither the dots nor the indicators show, although the display RAM keeps its contents.
    #[must_use]
    pub const fn is_on(&self) -> bool {
        self.lcd_on
    }

    /// The 156 columns of the matrix as they are in the display RAM (see [`decode_columns`]). NOTE:
    /// it does not take [`DisplayController::is_on`] into account; for what really shows, use
    /// [`DisplayController::is_pixel_on`] or also look at `is_on()`.
    #[must_use]
    pub const fn columns(&self) -> &[u8; DISPLAY_WIDTH] {
        &self.columns
    }

    /// `true` if the dot at column `x` (0-155, left to right) and row `y` (0-6, top to bottom)
    /// shows lit. Out of range, `false`.
    #[must_use]
    pub fn is_pixel_on(&self, x: usize, y: usize) -> bool {
        self.lcd_on && y < DISPLAY_HEIGHT && self.columns.get(x).is_some_and(|&c| (c >> y) & 1 != 0)
    }

    /// `true` if the indicator `symbol` shows lit (always `false` with the LCD off, and for
    /// [`Symbol::Battery`]).
    #[must_use]
    pub const fn is_symbol_on(&self, symbol: Symbol) -> bool {
        self.lcd_on && self.symbol_buffer[symbol as usize]
    }

    /// Works out what shows from the columns and indicators already decoded and from whether the
    /// LCD is on.
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
    /// Reads the display RAM again (matrix and indicators) and leaves in [`DisplayController`] what
    /// shows, with the LCD on if `lcd_on`. With `lcd_on == false` the whole panel, indicators
    /// included, shows off: on the real machine it is all a single glass driven by the same signal.
    ///
    /// There is usually no need to call it: [`Pc1500::display`] does it with the right `lcd_on`.
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
            assert_ne!(a.0, Symbol::Battery, "Battery has no bit");
            for b in &SYMBOL_BITS[i + 1..] {
                assert_ne!(a.0, b.0, "repeated indicator");
                assert!(a.1 != b.1 || a.2 != b.2, "repeated bit: {a:?} {b:?}");
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
        assert_eq!(d.columns()[0], 0x7F, "the RAM is still there");
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
        // Pair $7602/$7603: low nibbles → column 1, high ones → 79.
        pc.write_byte(0x7602, 0x21); // fila 0 en la columna 1, fila 1 en la 79
        pc.write_byte(0x7603, 0x40); // fila 6 en la columna 79
        // Pair $774C/$774D (the last one): columns 77 and 155.
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
        assert!(!d.is_pixel_on(DISPLAY_WIDTH, 0), "out of range");
        assert!(!d.is_pixel_on(1, 7), "out of range");
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
