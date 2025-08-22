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
    Deg,
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
    symbol_buffer: [bool; 14],
}

impl DisplayController {
    pub fn new() -> Self {
        Self {
            rgba_buffer: [0; RGBA_SIZE],
            symbol_buffer: [false; 14],
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

        self.display.symbol_buffer[Symbol::Busy as usize] = (symb1 & 0x01) == 0;
        self.display.symbol_buffer[Symbol::Shift as usize] = (symb1 & 0x02) == 0;
        self.display.symbol_buffer[Symbol::Kana as usize] = (symb1 & 0x04) == 0;
        self.display.symbol_buffer[Symbol::Small as usize] = (symb1 & 0x08) == 0;
        self.display.symbol_buffer[Symbol::RomanIII as usize] = (symb1 & 0x10) == 0;
        self.display.symbol_buffer[Symbol::RomanII as usize] = (symb1 & 0x20) == 0;
        self.display.symbol_buffer[Symbol::RomanI as usize] = (symb1 & 0x40) == 0;
        self.display.symbol_buffer[Symbol::Def as usize] = (symb1 & 0x80) == 0;

        self.display.symbol_buffer[Symbol::Deg as usize] = (symb2 & 0x01) == 0;
        self.display.symbol_buffer[Symbol::Rad as usize] = (symb2 & 0x02) == 0;
        self.display.symbol_buffer[Symbol::Rad as usize] = (symb2 & 0x04) == 0;
        self.display.symbol_buffer[Symbol::Reserve as usize] = (symb2 & 0x10) == 0;
        self.display.symbol_buffer[Symbol::Pro as usize] = (symb2 & 0x20) == 0;
        self.display.symbol_buffer[Symbol::Run as usize] = (symb2 & 0x40) == 0;

        self.display.symbol_buffer[Symbol::Battery as usize] = true;
    }
}

impl Default for DisplayController {
    fn default() -> Self {
        Self::new()
    }
}
