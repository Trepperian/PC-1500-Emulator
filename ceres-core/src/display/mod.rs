/// LCD Display controller for PC-1500
///
/// The PC-1500 features a 156x7 pixel monochrome LCD display (CORRECTED).
/// Each pixel is represented by 1 bit, packed into bytes.

pub const DISPLAY_WIDTH: usize = 156;
pub const DISPLAY_HEIGHT: usize = 7; // CORREGIDO: PC-1500 tiene 7 filas, no 8
pub const DISPLAY_BYTES: usize = 160; // PC-1500 display memory: 160 bytes (80+80 sections)
pub const RGBA_SIZE: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT * 4; // RGBA format

#[derive(Debug, Clone)]
pub struct DisplayController {
    /// Raw display memory (1 bit per pixel, packed)
    vram: [u8; DISPLAY_BYTES],

    /// RGBA buffer for GPU rendering
    rgba_buffer: [u8; RGBA_SIZE],

    /// Display control settings
    contrast: u8,
    enabled: bool,

    /// Dirty flag to track when display needs update
    dirty: bool,
}

impl DisplayController {
    pub fn new() -> Self {
        Self {
            vram: [0; DISPLAY_BYTES],
            rgba_buffer: [0; RGBA_SIZE],
            contrast: 0x80, // Default contrast
            enabled: true,
            dirty: true,
        }
    }

    /// Read from video RAM
    pub fn read_vram(&self, offset: u16) -> u8 {
        let offset = offset as usize;
        if offset < DISPLAY_BYTES {
            self.vram[offset]
        } else {
            0xFF
        }
    }

    /// Write to video RAM
    pub fn write_vram(&mut self, offset: u16, value: u8) {
        let offset = offset as usize;
        if offset < DISPLAY_BYTES {
            self.vram[offset] = value;
            self.dirty = true;
        }
    }

    /// Set a pixel at given coordinates using PC-1500's two-symbol system
    pub fn set_pixel(&mut self, x: usize, y: usize, value: bool) {
        if x < DISPLAY_WIDTH && y < DISPLAY_HEIGHT && x < DISPLAY_BYTES {
            if y < 4 {
                // First symbol (4 DOTS - top 4 rows): bits 0,1,2,3
                if value {
                    self.vram[x] |= 1 << y;
                } else {
                    self.vram[x] &= !(1 << y);
                }
            } else if y < 7 {
                // Second symbol (3 DOTS - bottom 3 rows): bits 4,5,6
                let dot_bit = 4 + (y - 4); // Map y=4,5,6 to bits 4,5,6
                if value {
                    self.vram[x] |= 1 << dot_bit;
                } else {
                    self.vram[x] &= !(1 << dot_bit);
                }
            }
            // Note: Row 7 (bit 7) is ignored in PC-1500
            self.dirty = true;
        }
    }

    /// Get a pixel at given coordinates using PC-1500's two-symbol system  
    pub fn get_pixel(&self, x: usize, y: usize) -> bool {
        if x < DISPLAY_WIDTH && y < DISPLAY_HEIGHT && x < DISPLAY_BYTES {
            if y < 4 {
                // First symbol (4 DOTS - top 4 rows): bits 0,1,2,3
                (self.vram[x] & (1 << y)) != 0
            } else if y < 7 {
                // Second symbol (3 DOTS - bottom 3 rows): bits 4,5,6
                let dot_bit = 4 + (y - 4); // Map y=4,5,6 to bits 4,5,6
                (self.vram[x] & (1 << dot_bit)) != 0
            } else {
                // Row 7 (bit 7) is ignored in PC-1500
                false
            }
        } else {
            false
        }
    }

    /// Update the RGBA buffer from VRAM using PC-1500's two-symbol system
    pub fn update_rgba_buffer(&mut self) {
        if !self.dirty {
            return;
        }

        // PC-1500 display: Each byte encodes TWO hex symbols:
        // - First symbol (4 DOTS): bits 0,1,2,3 (top 4 rows)
        // - Second symbol (3 DOTS): bits 4,5,6 (bottom 3 rows)  
        // - Bit 7 is ignored
        for x in 0..DISPLAY_WIDTH {
            if x < DISPLAY_BYTES {
                let byte_value = self.vram[x];
                
                // Extract the two symbols from the byte
                let first_symbol = byte_value & 0x0F;   // bits 0,1,2,3 (4 DOTS)
                let second_symbol = (byte_value >> 4) & 0x07; // bits 4,5,6 (3 DOTS)
                
                // Render first symbol (4 DOTS - top 4 rows)
                for dot in 0..4 {
                    let pixel_on = (first_symbol & (1 << dot)) != 0;
                    let y = dot;
                    let rgba_idx = (y * DISPLAY_WIDTH + x) * 4;
                    
                    if pixel_on && self.enabled {
                        let brightness = self.contrast;
                        self.rgba_buffer[rgba_idx] = brightness; // R
                        self.rgba_buffer[rgba_idx + 1] = brightness; // G
                        self.rgba_buffer[rgba_idx + 2] = brightness; // B
                        self.rgba_buffer[rgba_idx + 3] = 255; // A
                    } else {
                        // Pixel is off - background color
                        self.rgba_buffer[rgba_idx] = 0; // R
                        self.rgba_buffer[rgba_idx + 1] = 0; // G
                        self.rgba_buffer[rgba_idx + 2] = 0; // B
                        self.rgba_buffer[rgba_idx + 3] = 255; // A
                    }
                }
                
                // Render second symbol (3 DOTS - bottom 3 rows)
                for dot in 0..3 {
                    let pixel_on = (second_symbol & (1 << dot)) != 0;
                    let y = 4 + dot; // Start from row 4 (after first symbol)
                    let rgba_idx = (y * DISPLAY_WIDTH + x) * 4;
                    
                    if pixel_on && self.enabled {
                        let brightness = self.contrast;
                        self.rgba_buffer[rgba_idx] = brightness; // R
                        self.rgba_buffer[rgba_idx + 1] = brightness; // G
                        self.rgba_buffer[rgba_idx + 2] = brightness; // B
                        self.rgba_buffer[rgba_idx + 3] = 255; // A
                    } else {
                        // Pixel is off - background color
                        self.rgba_buffer[rgba_idx] = 0; // R
                        self.rgba_buffer[rgba_idx + 1] = 0; // G
                        self.rgba_buffer[rgba_idx + 2] = 0; // B
                        self.rgba_buffer[rgba_idx + 3] = 255; // A
                    }
                }
            } else {
                // For columns beyond display memory, clear all pixels
                for y in 0..DISPLAY_HEIGHT {
                    let rgba_idx = (y * DISPLAY_WIDTH + x) * 4;
                    self.rgba_buffer[rgba_idx] = 0; // R
                    self.rgba_buffer[rgba_idx + 1] = 0; // G
                    self.rgba_buffer[rgba_idx + 2] = 0; // B
                    self.rgba_buffer[rgba_idx + 3] = 255; // A
                }
            }
        }

        self.dirty = false;
    }

    /// Get the RGBA buffer for rendering
    pub fn rgba_buffer(&mut self) -> &[u8] {
        self.update_rgba_buffer();
        &self.rgba_buffer
    }

    /// Get the RGBA buffer without updating (const version for GameBoy compatibility)
    #[must_use]
    pub const fn rgba_buffer_const(&self) -> &[u8] {
        &self.rgba_buffer
    }

    /// Set display control register
    pub fn set_control(&mut self, value: u8) {
        let new_enabled = (value & 0x01) != 0;
        let new_contrast = (value & 0xFE) >> 1;

        if new_enabled != self.enabled || new_contrast != self.contrast {
            self.enabled = new_enabled;
            self.contrast = new_contrast;
            self.dirty = true;
        }
    }

    /// Clear the entire display
    pub fn clear(&mut self) {
        self.vram.fill(0);
        self.dirty = true;
    }

    /// Fill the entire display (for testing)
    pub fn fill(&mut self) {
        self.vram.fill(0xFF);
        self.dirty = true;
    }

    /// Check if display needs update
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Write a value using PC-1500's two-symbol encoding
    /// value: byte where bits 0-3 = first symbol (4 DOTS), bits 4-6 = second symbol (3 DOTS)
    pub fn write_pc1500_symbols(&mut self, column: usize, value: u8) {
        if column < DISPLAY_BYTES {
            self.vram[column] = value;
            self.dirty = true;
        }
    }

    /// Read a value using PC-1500's two-symbol encoding
    pub fn read_pc1500_symbols(&self, column: usize) -> u8 {
        if column < DISPLAY_BYTES {
            self.vram[column]
        } else {
            0x00
        }
    }

    /// Set first symbol (4 DOTS - upper part) for a column
    pub fn set_first_symbol(&mut self, column: usize, symbol: u8) {
        if column < DISPLAY_BYTES {
            let current = self.vram[column];
            // Clear bits 0-3 and set new first symbol
            self.vram[column] = (current & 0xF0) | (symbol & 0x0F);
            self.dirty = true;
        }
    }

    /// Set second symbol (3 DOTS - lower part) for a column  
    pub fn set_second_symbol(&mut self, column: usize, symbol: u8) {
        if column < DISPLAY_BYTES {
            let current = self.vram[column];
            // Clear bits 4-6 and set new second symbol
            self.vram[column] = (current & 0x8F) | ((symbol & 0x07) << 4);
            self.dirty = true;
        }
    }

    /// Get first symbol (4 DOTS - upper part) for a column
    pub fn get_first_symbol(&self, column: usize) -> u8 {
        if column < DISPLAY_BYTES {
            self.vram[column] & 0x0F
        } else {
            0x00
        }
    }

    /// Get second symbol (3 DOTS - lower part) for a column
    pub fn get_second_symbol(&self, column: usize) -> u8 {
        if column < DISPLAY_BYTES {
            (self.vram[column] >> 4) & 0x07
        } else {
            0x00
        }
    }

    /// Draw a character at given position using 5x7 font
    pub fn draw_char(&mut self, x: usize, y: usize, ch: char) {
        let font_data = get_char_bitmap(ch);
        
        for (row, &byte) in font_data.iter().enumerate() {
            for col in 0..5 {
                let pixel_on = (byte & (1 << (4 - col))) != 0;
                self.set_pixel(x + col, y + row, pixel_on);
            }
        }
    }

    /// Draw a string starting at given position
    pub fn draw_string(&mut self, x: usize, y: usize, text: &str) {
        let mut char_x = x;
        
        for ch in text.chars() {
            if char_x + 5 >= DISPLAY_WIDTH {
                break; // No more space
            }
            
            self.draw_char(char_x, y, ch);
            char_x += 6; // 5 pixels wide + 1 pixel spacing
        }
    }

    /// Draw a string centered on the display
    pub fn draw_string_centered(&mut self, y: usize, text: &str) {
        let text_width = text.len() * 6 - 1; // 6 pixels per char, minus last spacing
        if text_width < DISPLAY_WIDTH {
            let start_x = (DISPLAY_WIDTH - text_width) / 2;
            self.draw_string(start_x, y, text);
        }
    }

    /// Create a test pattern with text
    pub fn test_pattern_with_text(&mut self) {
        self.clear();
        self.draw_string_centered(0, "PC-1500");
    }

    /// Display status message
    pub fn show_status(&mut self, message: &str) {
        self.clear();
        self.draw_string_centered(1, message);
    }
}

impl Default for DisplayController {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple 5x7 bitmap font for basic characters
fn get_char_bitmap(ch: char) -> &'static [u8; 7] {
    match ch {
        'A' | 'a' => &[0x1C, 0x22, 0x22, 0x3E, 0x22, 0x22, 0x22],
        'B' | 'b' => &[0x3E, 0x22, 0x22, 0x3C, 0x22, 0x22, 0x3E],
        'C' | 'c' => &[0x1C, 0x22, 0x20, 0x20, 0x20, 0x22, 0x1C],
        'D' | 'd' => &[0x3C, 0x22, 0x22, 0x22, 0x22, 0x22, 0x3C],
        'E' | 'e' => &[0x3E, 0x20, 0x20, 0x3C, 0x20, 0x20, 0x3E],
        'F' | 'f' => &[0x3E, 0x20, 0x20, 0x3C, 0x20, 0x20, 0x20],
        'G' | 'g' => &[0x1C, 0x22, 0x20, 0x26, 0x22, 0x22, 0x1C],
        'H' | 'h' => &[0x22, 0x22, 0x22, 0x3E, 0x22, 0x22, 0x22],
        'I' | 'i' => &[0x1C, 0x08, 0x08, 0x08, 0x08, 0x08, 0x1C],
        'J' | 'j' => &[0x02, 0x02, 0x02, 0x02, 0x22, 0x22, 0x1C],
        'K' | 'k' => &[0x22, 0x24, 0x28, 0x30, 0x28, 0x24, 0x22],
        'L' | 'l' => &[0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x3E],
        'M' | 'm' => &[0x22, 0x36, 0x2A, 0x2A, 0x22, 0x22, 0x22],
        'N' | 'n' => &[0x22, 0x32, 0x2A, 0x26, 0x22, 0x22, 0x22],
        'O' | 'o' => &[0x1C, 0x22, 0x22, 0x22, 0x22, 0x22, 0x1C],
        'P' | 'p' => &[0x3C, 0x22, 0x22, 0x3C, 0x20, 0x20, 0x20],
        'Q' | 'q' => &[0x1C, 0x22, 0x22, 0x22, 0x2A, 0x24, 0x1A],
        'R' | 'r' => &[0x3C, 0x22, 0x22, 0x3C, 0x28, 0x24, 0x22],
        'S' | 's' => &[0x1C, 0x22, 0x20, 0x1C, 0x02, 0x22, 0x1C],
        'T' | 't' => &[0x3E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08],
        'U' | 'u' => &[0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x1C],
        'V' | 'v' => &[0x22, 0x22, 0x22, 0x22, 0x14, 0x14, 0x08],
        'W' | 'w' => &[0x22, 0x22, 0x22, 0x2A, 0x2A, 0x36, 0x22],
        'X' | 'x' => &[0x22, 0x22, 0x14, 0x08, 0x14, 0x22, 0x22],
        'Y' | 'y' => &[0x22, 0x22, 0x14, 0x08, 0x08, 0x08, 0x08],
        'Z' | 'z' => &[0x3E, 0x02, 0x04, 0x08, 0x10, 0x20, 0x3E],
        '0' => &[0x1C, 0x22, 0x26, 0x2A, 0x32, 0x22, 0x1C],
        '1' => &[0x08, 0x18, 0x08, 0x08, 0x08, 0x08, 0x1C],
        '2' => &[0x1C, 0x22, 0x02, 0x0C, 0x10, 0x20, 0x3E],
        '3' => &[0x1C, 0x22, 0x02, 0x0C, 0x02, 0x22, 0x1C],
        '4' => &[0x04, 0x0C, 0x14, 0x24, 0x3E, 0x04, 0x04],
        '5' => &[0x3E, 0x20, 0x3C, 0x02, 0x02, 0x22, 0x1C],
        '6' => &[0x0C, 0x10, 0x20, 0x3C, 0x22, 0x22, 0x1C],
        '7' => &[0x3E, 0x02, 0x04, 0x08, 0x10, 0x10, 0x10],
        '8' => &[0x1C, 0x22, 0x22, 0x1C, 0x22, 0x22, 0x1C],
        '9' => &[0x1C, 0x22, 0x22, 0x1E, 0x02, 0x04, 0x18],
        ' ' => &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '-' => &[0x00, 0x00, 0x00, 0x1C, 0x00, 0x00, 0x00],
        '.' => &[0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18],
        '!' => &[0x08, 0x08, 0x08, 0x08, 0x00, 0x08, 0x08],
        '?' => &[0x1C, 0x22, 0x02, 0x0C, 0x08, 0x00, 0x08],
        _ => &[0x3E, 0x22, 0x20, 0x20, 0x20, 0x22, 0x3E], // Default to 'E' for unknown chars
    }
}
