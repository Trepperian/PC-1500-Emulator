use crate::interrupts::InterruptController;

/// PC-1500 Keyboard Controller
///
/// This module handles the keyboard matrix of the PC-1500, which is connected
/// to the LH5801 CPU through pins IN0-IN7 (pins 66-73 on the microprocessor).
///
/// The ITA (In To Accumulator) instruction reads these pins and transfers
/// the keyboard state to the CPU's accumulator register.
///
/// Key Code Matrix:
/// - The PC-1500 uses a 6x16 matrix (columns 0-5, rows 0-F)
/// - Each key has a unique code based on its row/column position
/// - The hardware uses active-low logic (pressed = 0, not pressed = 1)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    // Row 0 (0x00-0x05)
    Space = 0x02, // SPACE
    Zero = 0x03,  // 0
    P = 0x05,     // P

    // Row 1 (0x10-0x15)
    Shift = 0x10, // SHIFT
    F1 = 0x11,    // F1
    One = 0x13,   // 1
    A = 0x14,     // A
    Q = 0x15,     // Q

    // Row 2 (0x20-0x25)
    Sml = 0x20, // SML
    F2 = 0x21,  // F2
    Two = 0x23, // 2
    B = 0x24,   // B
    R = 0x25,   // R

    // Row 3 (0x30-0x35)
    F3 = 0x31,    // F3
    Three = 0x33, // 3
    C = 0x34,     // C
    S = 0x35,     // S

    // Row 4 (0x40-0x45)
    F4 = 0x41,   // F4
    Four = 0x43, // 4
    D = 0x44,    // D
    T = 0x45,    // T

    // Row 5 (0x50-0x55)
    F5 = 0x51,   // F5
    Five = 0x53, // 5
    E = 0x54,    // E
    U = 0x55,    // U

    // Row 6 (0x60-0x65)
    F6 = 0x61,  // F6
    Six = 0x63, // 6
    F = 0x64,   // F
    V = 0x65,   // V

    // Row 7 (0x70-0x75)
    Seven = 0x73, // 7
    G = 0x74,     // G
    W = 0x75,     // W

    // Row 8 (0x80-0x85)
    Left = 0x80,      // ◄ (left arrow)
    Cl = 0x81,        // CL
    LeftParen = 0x82, // (
    Eight = 0x83,     // 8
    H = 0x84,         // H
    X = 0x85,         // X

    // Row 9 (0x90-0x95)
    Up = 0x90,         // ▲ (up arrow)
    Down = 0x91,       // ▼ (down arrow)
    Rcl = 0x92,        // RCL
    RightParen = 0x93, // )
    Nine = 0x94,       // 9
    I = 0x95,          // I
    Y = 0x96,          // Y (note: appears to be in position 96, extending the pattern)

    // Row A (0xA0-0xA5)
    UpArrow = 0xA0,  // ↑
    Asterisk = 0xA2, // *
    J = 0xA4,        // J
    Z = 0xA5,        // Z

    // Row B (0xB0-0xB5)
    DownArrow = 0xB0, // ↓
    Def = 0xB1,       // DEF
    Plus = 0xB2,      // +
    K = 0xB4,         // K

    // Row C (0xC0-0xC5)
    Right = 0xC0, // ► (right arrow)
    L = 0xC4,     // L

    // Row D (0xD0-0xD5)
    Enter = 0xD0,  // ENTER
    Minus = 0xD2,  // -
    Equals = 0xD3, // =
    M = 0xD4,      // M

    // Row E (0xE0-0xE5)
    Dot = 0xE2, // .
    N = 0xE4,   // N

    // Row F (0xF0-0xF5)
    Off = 0xF0,         // OFF
    Mode = 0xF1,        // MODE
    Slash = 0xF2,       // /
    O = 0xF4,           // O
    Exclamation = 0xF5, // !

    // Additional keys for complete PC-1500 layout
    On = 0xF6,     // ON
    Ac = 0xF7,     // AC
    Quote = 0xF8,  // "
    Hash = 0xF9,   // #
    Dollar = 0xFA, // $
}

#[derive(Default, Debug)]
pub struct Keyboard {
    pressed_keys: u64, // Bit field to track which keys are pressed (64 bits for all possible key codes)
    col_select: u8,    // Column selection for keyboard matrix scanning
    row_select: u8,    // Row selection for keyboard matrix scanning
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            pressed_keys: 0,
            col_select: 0,
            row_select: 0,
        }
    }

    /// Press a key on the PC-1500 keyboard
    pub fn press(&mut self, key: Key, ints: &mut InterruptController) {
        let key_code = key as u8;
        let bit_pos = key_code & 0x3F; // Use lower 6 bits for bit position (0-63)

        self.pressed_keys |= 1u64 << bit_pos;

        // Request interrupt for keyboard input (maps to maskable interrupt)
        ints.request_p1(); // Uses maskable interrupt for PC-1500 keyboard input
    }

    /// Release a key on the PC-1500 keyboard
    pub fn release(&mut self, key: Key) {
        let key_code = key as u8;
        let bit_pos = key_code & 0x3F; // Use lower 6 bits for bit position (0-63)

        self.pressed_keys &= !(1u64 << bit_pos);
    }

    /// Check if a specific key is currently pressed
    pub fn is_pressed(&self, key: Key) -> bool {
        let key_code = key as u8;
        let bit_pos = key_code & 0x3F;

        (self.pressed_keys & (1u64 << bit_pos)) != 0
    }

    /// Read keyboard matrix data based on current column/row selection
    /// This method is called by the ITA (In To Accumulator) instruction
    /// to read the IN0-IN7 pins connected to the keyboard matrix
    #[must_use]
    pub fn read_keyboard_matrix(&self) -> u8 {
        self.read_input_pins_in0_in7()
    }

    /// Read IN0-IN7 pins for ITA instruction
    /// This simulates reading the key input port pins (66-73) on the LH5801
    /// Returns the 8-bit value representing the current keyboard state
    #[must_use]
    pub fn read_input_pins_in0_in7(&self) -> u8 {
        let mut result = 0xFF; // Default: all pins high (no keys pressed, active low)

        // Scan through all possible key codes in our matrix
        for row in 0..=0xF {
            for col in 0..=0x5 {
                // PC-1500 has 6 columns (0-5)
                let key_code = (row << 4) | col;
                let bit_pos = key_code & 0x3F; // Use as bit position

                // Check if this key is currently pressed
                if (self.pressed_keys & (1u64 << bit_pos)) != 0 {
                    // Key is pressed - determine which IN pin should be affected
                    // Map the key position to the appropriate IN0-IN7 bit

                    // For PC-1500 keyboard matrix:
                    // - Columns (0-5) map to lower 3 bits of IN0-IN7 (bits 0,1,2)
                    // - Rows (0-F) map to upper 4 bits of IN0-IN7 (bits 3,4,5,6)
                    // - Bit 7 can be used for additional state

                    let input_pin_value = (col & 0x07) | ((row & 0x0F) << 3);

                    // In active-low logic, pressed key pulls the corresponding bit low
                    result &= !(1 << (input_pin_value & 0x07));

                    // For simplicity, we'll return the first pressed key's code
                    // In real hardware, multiple keys might create different patterns
                    if result != 0xFF {
                        // Return the key code directly for ITA instruction
                        return key_code;
                    }
                }
            }
        }

        result
    }

    /// Get the raw key code of the first pressed key (for ITA instruction)
    /// This is what the CPU's ITA instruction should read from IN0-IN7
    #[must_use]
    pub fn get_pressed_key_code(&self) -> u8 {
        // Find the first pressed key and return its code
        for row in 0..=0xF {
            for col in 0..=0x5 {
                let key_code = (row << 4) | col;
                let bit_pos = key_code & 0x3F;

                if (self.pressed_keys & (1u64 << bit_pos)) != 0 {
                    return key_code; // Return the exact key code from our matrix
                }
            }
        }

        0xFF // No key pressed
    }

    /// Read the current keyboard matrix state for a specific column
    /// This can be used for more sophisticated keyboard scanning
    #[must_use]
    pub fn read_column_state(&self, column: u8) -> u8 {
        let mut result = 0xFF; // All bits high (no keys pressed)

        if column <= 5 {
            // PC-1500 has columns 0-5
            for row in 0..=0xF {
                let key_code = (row << 4) | column;
                let bit_pos = key_code & 0x3F;

                if (self.pressed_keys & (1u64 << bit_pos)) != 0 {
                    // Key in this row/column is pressed - clear corresponding bit
                    result &= !(1 << row);
                }
            }
        }

        result
    }

    /// Write to keyboard control register (column selection)
    pub fn write_column_select(&mut self, val: u8) {
        self.col_select = val;
    }

    /// Write to keyboard control register (row selection)
    pub fn write_row_select(&mut self, val: u8) {
        self.row_select = val;
    }

    /// Read keyboard column selection
    #[must_use]
    pub fn read_column_select(&self) -> u8 {
        self.col_select
    }

    /// Read keyboard row selection
    #[must_use]
    pub fn read_row_select(&self) -> u8 {
        self.row_select
    }
}
