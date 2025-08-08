/// Keyboard matrix controller for PC-1500
///
/// The PC-1500 uses an 8x8 matrix keyboard system with 64 keys total.
/// Keys are scanned by setting a row and reading the column states.

#[derive(Debug, Clone)]
pub struct KeyboardController {
    /// 8x8 key matrix - matrix[row][col]
    matrix: [[bool; 8]; 8],

    /// Currently selected row for scanning
    current_row: u8,
}

impl KeyboardController {
    pub fn new() -> Self {
        Self {
            matrix: [[false; 8]; 8],
            current_row: 0,
        }
    }

    /// Set the current row for scanning
    pub fn set_scan_row(&mut self, row: u8) {
        if row < 8 {
            self.current_row = row;
        }
    }

    /// Read the column states for the current row
    pub fn read_columns(&self) -> u8 {
        let row = self.current_row as usize;
        let mut result = 0u8;

        for col in 0..8 {
            if self.matrix[row][col] {
                result |= 1 << col;
            }
        }

        result
    }

    /// Set the state of a specific key
    pub fn set_key(&mut self, row: u8, col: u8, pressed: bool) {
        if row < 8 && col < 8 {
            self.matrix[row as usize][col as usize] = pressed;
        }
    }

    /// Get the state of a specific key
    pub fn get_key(&self, row: u8, col: u8) -> bool {
        if row < 8 && col < 8 {
            self.matrix[row as usize][col as usize]
        } else {
            false
        }
    }

    /// Clear all key states
    pub fn clear_all(&mut self) {
        self.matrix = [[false; 8]; 8];
    }

    /// Map PC keyboard input to PC-1500 matrix
    /// This is a simplified mapping - actual mapping depends on keyboard layout
    pub fn handle_pc_key(&mut self, scancode: u32, pressed: bool) {
        // TODO: Implement proper key mapping
        // This is a placeholder mapping for common keys

        match scancode {
            // Numbers 0-9
            11..=19 => {
                let num = scancode - 10;
                let row = if num == 0 { 3 } else { (num - 1) / 3 };
                let col = if num == 0 { 2 } else { (num - 1) % 3 };
                self.set_key(row as u8, col as u8, pressed);
            }

            // Letters A-Z (simplified mapping)
            30..=38 => {
                // QWERTYUIO
                let row = 4;
                let col = scancode - 30;
                self.set_key(row as u8, col as u8, pressed);
            }

            // Space bar
            57 => self.set_key(7, 0, pressed),

            // Enter key (EXE)
            28 => self.set_key(7, 7, pressed),

            _ => {
                // Unmapped key, ignore
            }
        }
    }
}

impl Default for KeyboardController {
    fn default() -> Self {
        Self::new()
    }
}

/// PC-1500 key definitions for reference
#[allow(dead_code)]
pub mod keys {
    // Row 0: Function keys
    pub const F1: (u8, u8) = (0, 0);
    pub const F2: (u8, u8) = (0, 1);
    pub const F3: (u8, u8) = (0, 2);
    pub const F4: (u8, u8) = (0, 3);
    pub const F5: (u8, u8) = (0, 4);
    pub const F6: (u8, u8) = (0, 5);

    // Row 1: Numbers
    pub const NUM_1: (u8, u8) = (1, 0);
    pub const NUM_2: (u8, u8) = (1, 1);
    pub const NUM_3: (u8, u8) = (1, 2);
    pub const NUM_4: (u8, u8) = (1, 3);
    pub const NUM_5: (u8, u8) = (1, 4);
    pub const NUM_6: (u8, u8) = (1, 5);
    pub const NUM_7: (u8, u8) = (1, 6);
    pub const NUM_8: (u8, u8) = (1, 7);

    // Row 2: More numbers and operators
    pub const NUM_9: (u8, u8) = (2, 0);
    pub const NUM_0: (u8, u8) = (2, 1);
    pub const PLUS: (u8, u8) = (2, 2);
    pub const MINUS: (u8, u8) = (2, 3);
    pub const MULTIPLY: (u8, u8) = (2, 4);
    pub const DIVIDE: (u8, u8) = (2, 5);
    pub const EQUALS: (u8, u8) = (2, 6);
    pub const DOT: (u8, u8) = (2, 7);

    // Row 3: Letters Q-P
    pub const Q: (u8, u8) = (3, 0);
    pub const W: (u8, u8) = (3, 1);
    pub const E: (u8, u8) = (3, 2);
    pub const R: (u8, u8) = (3, 3);
    pub const T: (u8, u8) = (3, 4);
    pub const Y: (u8, u8) = (3, 5);
    pub const U: (u8, u8) = (3, 6);
    pub const I: (u8, u8) = (3, 7);

    // Continue for other rows...
    // This is a simplified mapping for demonstration
}
