use crate::Pc1500;

pub const DISPLAY_WIDTH: usize = 156;
pub const DISPLAY_HEIGHT: usize = 7;
pub const RGBA_SIZE: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT * 4; // RGBA format

fn low(b: u8) -> u8 {
    b & 0x0F
}

fn high(b: u8) -> u8 {
    (b >> 4) & 0x0F
}

#[derive(Debug, Clone, Copy)]
pub enum Symbol {
    Busy,
    Shift,
    Kana,
    Small,
    /// The angle mode's three segments: DE, G and RAD (see `update`).
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

#[derive(Debug, Clone)]
pub struct DisplayController {
    /// RGBA buffer for GPU rendering
    rgba_buffer: [u8; RGBA_SIZE],
    symbol_buffer: [bool; 15],
}

impl DisplayController {
    pub fn new() -> Self {
        Self {
            rgba_buffer: [0; RGBA_SIZE],
            symbol_buffer: [false; 15],
        }
    }

    pub fn rgba_buffer(&self) -> &[u8; RGBA_SIZE] {
        &self.rgba_buffer
    }

    pub fn is_symbol_on(&self, symbol: Symbol) -> bool {
        self.symbol_buffer[symbol as usize]
    }

    fn draw_black_pixel(&mut self, x: usize, y: usize) {
        let index = (y * DISPLAY_WIDTH + x) * 4;
        self.rgba_buffer[index..index + 4].copy_from_slice(&[0, 0, 0, 255]);
    }

    fn draw_white_pixel(&mut self, x: usize, y: usize) {
        let index = (y * DISPLAY_WIDTH + x) * 4;
        self.rgba_buffer[index..index + 4].copy_from_slice(&[255, 255, 255, 255]);
    }

    fn clear(&mut self) {
        self.rgba_buffer.fill(0xff);
    }
}

impl Pc1500 {
    pub fn update_display_buffer(&mut self) {
        if !self.lh5801.display_enabled() {
            self.display.clear();
            return;
        }

        for ind in (0..0x4D).step_by(2) {
            let adr = 0x7600 | ind;
            let data = low(self.read_byte(adr)) | (low(self.read_byte(adr + 1)) << 4);
            let x = ind >> 1;

            for b in 0..7 {
                if (data >> b) & 0x01 != 0 {
                    self.display.draw_black_pixel(x as usize, b);
                } else {
                    self.display.draw_white_pixel(x as usize, b);
                }
            }

            let data = high(self.read_byte(adr)) | (high(self.read_byte(adr + 1)) << 4);
            let x = x + 78;

            for b in 0..7 {
                if (data >> b) & 0x01 != 0 {
                    self.display.draw_black_pixel(x as usize, b);
                } else {
                    self.display.draw_white_pixel(x as usize, b);
                }
            }
        }

        for ind in (0..0x4D).step_by(2) {
            let adr = 0x7700 | ind;
            let data = low(self.read_byte(adr)) | (low(self.read_byte(adr + 1)) << 4);
            let x = (ind >> 1) + 39;

            for b in 0..7 {
                if (data >> b) & 0x01 != 0 {
                    self.display.draw_black_pixel(x as usize, b);
                } else {
                    self.display.draw_white_pixel(x as usize, b);
                }
            }

            let data = high(self.read_byte(adr)) | (high(self.read_byte(adr + 1)) << 4);
            let x = x + 78;

            for b in 0..7 {
                if (data >> b) & 0x01 != 0 {
                    self.display.draw_black_pixel(x as usize, b);
                } else {
                    self.display.draw_white_pixel(x as usize, b);
                }
            }
        }

        // Symbols
        let symb1 = self.read_byte(0x764E);
        let symb2 = self.read_byte(0x764F);

        // A set bit lights its indicator: the ROM sets BUSY (bit 0 of
        // &764E) while a program runs (CONT, $C8D5) and clears it to wait
        // for a key ($CA88), and the bit of the mode it is in (&764F).
        // They used to be drawn the other way round.
        self.display.symbol_buffer[Symbol::Busy as usize] = (symb1 & 0x01) != 0;
        self.display.symbol_buffer[Symbol::Shift as usize] = (symb1 & 0x02) != 0;
        self.display.symbol_buffer[Symbol::Kana as usize] = (symb1 & 0x04) != 0;
        self.display.symbol_buffer[Symbol::Small as usize] = (symb1 & 0x08) != 0;
        self.display.symbol_buffer[Symbol::RomanIII as usize] = (symb1 & 0x10) != 0;
        self.display.symbol_buffer[Symbol::RomanII as usize] = (symb1 & 0x20) != 0;
        self.display.symbol_buffer[Symbol::RomanI as usize] = (symb1 & 0x40) != 0;
        self.display.symbol_buffer[Symbol::Def as usize] = (symb1 & 0x80) != 0;

        // The angle mode, in the low three bits of &764F (DEGREE puts 3,
        // RADIAN 4 and GRAD 6: BCMD_DEGREE, $C697), lights the segments DE
        // (bit 0), G (bit 1) and RAD (bit 2): DEG, RAD or GRAD.
        self.display.symbol_buffer[Symbol::De as usize] = (symb2 & 0x01) != 0;
        self.display.symbol_buffer[Symbol::G as usize] = (symb2 & 0x02) != 0;
        self.display.symbol_buffer[Symbol::Rad as usize] = (symb2 & 0x04) != 0;
        self.display.symbol_buffer[Symbol::Reserve as usize] = (symb2 & 0x10) != 0;
        self.display.symbol_buffer[Symbol::Pro as usize] = (symb2 & 0x20) != 0;
        self.display.symbol_buffer[Symbol::Run as usize] = (symb2 & 0x40) != 0;

        self.display.symbol_buffer[Symbol::Battery as usize] = true;
    }
}

impl Default for DisplayController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Symbol;
    use crate::keyboard::Key;
    use crate::Pc1500;

    /// After booting, the ROM waits for a key in PRO mode and DEGREE
    /// (&764E = &40, &764F = &23): PRO and the DE and G segments (DEG) are
    /// lit; RUN, RAD and BUSY are not.
    #[test]
    fn indicators_after_boot_are_pro_and_deg() {
        let mut pc1500 = Pc1500::new();
        for _ in 0..200 {
            pc1500.step_frame();
        }
        let display = pc1500.display();
        let lit = |symbol| display.is_symbol_on(symbol);
        assert!(lit(Symbol::Pro) && lit(Symbol::De) && lit(Symbol::G));
        assert!(!lit(Symbol::Run) && !lit(Symbol::Rad) && !lit(Symbol::Busy));
    }

    /// The cold start shows NEW0? :CHECK, and the ROM takes no key but CL
    /// until it is cleared: MODE does nothing. After CL, MODE goes to RUN
    /// mode (bit 6 of &764F, the one the ROM's RUN and GOTO need) and RUN
    /// is lit, and a second MODE goes back to PRO.
    #[test]
    fn mode_works_after_cl_and_lights_run() {
        let mut pc1500 = Pc1500::new();
        for _ in 0..200 {
            pc1500.step_frame();
        }
        // About as long as the window holds a key (150 ms) and a pause.
        let type_key = |pc1500: &mut Pc1500, key| {
            pc1500.press(key);
            for _ in 0..15 {
                pc1500.step_frame();
            }
            pc1500.release(key);
            for _ in 0..40 {
                pc1500.step_frame();
            }
            let display = pc1500.display();
            (display.is_symbol_on(Symbol::Run), display.is_symbol_on(Symbol::Pro))
        };
        assert_eq!(type_key(&mut pc1500, Key::Mode), (false, true));
        assert_eq!(type_key(&mut pc1500, Key::Cl), (false, true));
        assert_eq!(type_key(&mut pc1500, Key::Mode), (true, false));
        assert_eq!(pc1500.read_byte(0x764F) & 0x60, 0x40);
        assert_eq!(type_key(&mut pc1500, Key::Mode), (false, true));
    }
}
