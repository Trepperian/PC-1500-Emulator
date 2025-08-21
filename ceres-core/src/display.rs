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

#[derive(Debug, Clone)]
pub struct DisplayController {
    /// RGBA buffer for GPU rendering
    rgba_buffer: [u8; RGBA_SIZE],
}

impl DisplayController {
    pub fn new() -> Self {
        Self {
            rgba_buffer: [0; RGBA_SIZE],
        }
    }

    pub fn rgba_buffer(&self) -> &[u8; RGBA_SIZE] {
        &self.rgba_buffer
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
    }
}

impl Default for DisplayController {
    fn default() -> Self {
        Self::new()
    }
}
