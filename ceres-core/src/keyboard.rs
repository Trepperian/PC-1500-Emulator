#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    // Row 0 (0x00-0x05)
    Space, // SPACE
    Zero,  // 0
    P,     // P

    // Row 1 (0x10-0x15)
    Shift, // SHIFT
    F1,    // F1
    One,   // 1
    A,     // A
    Q,     // Q

    // Row 2 (0x20-0x25)
    Sml, // SML
    F2,  // F2
    Two, // 2
    B,   // B
    R,   // R

    // Row 3 (0x30-0x35)
    F3,    // F3
    Three, // 3
    C,     // C
    S,     // S

    // Row 4 (0x40-0x45)
    F4,   // F4
    Four, // 4
    D,    // D
    T,    // T

    // Row 5 (0x50-0x55)
    F5,   // F5
    Five, // 5
    E,    // E
    U,    // U

    // Row 6 (0x60-0x65)
    F6,  // F6
    Six, // 6
    F,   // F
    V,   // V

    // Row 7 (0x70-0x75)
    Seven, // 7
    G,     // G
    W,     // W

    // Row 8 (0x80-0x85)
    Left,      // ◄ (left arrow)
    Cl,        // CL
    LeftParen, // (
    Eight,     // 8
    H,         // H
    X,         // X

    // Row 9 (0x90-0x95)
    Up,         // ▲ (up arrow)
    Down,       // ▼ (down arrow)
    Rcl,        // RCL
    RightParen, // )
    Nine,       // 9
    I,          // I
    Y,          // Y (note: appears to be in position 96, extending the pattern)

    // Row A (0xA0-0xA5)
    UpArrow,  // ↑
    Asterisk, // *
    J,        // J
    Z,        // Z

    // Row B (0xB0-0xB5)
    DownArrow, // ↓
    Def,       // DEF
    Plus,      // +
    K,         // K

    // Row C (0xC0-0xC5)
    Right, // ► (right arrow)
    L,     // L

    // Row D (0xD0-0xD5)
    Enter,  // ENTER
    Minus,  // -
    Equals, // =
    M,      // M

    // Row E (0xE0-0xE5)
    Dot, // .
    N,   // N

    // Row F (0xF0-0xF5)
    Off,         // OFF
    Mode,        // MODE
    Slash,       // /
    O,           // O
    Exclamation, // !

    // Additional keys for complete PC-1500 layout
    On,     // ON
    Ac,     // AC
    Quote,  // "
    Hash,   // #
    Dollar, // $
}

#[derive(Default, Debug)]
pub struct Keyboard {
    ks: u8,
    input: u8,
}

impl Keyboard {
    pub fn new() -> Self {
        Self { ks: 0, input: 0xff }
    }

    pub fn set_ks(&mut self, ks: u8) {
        self.ks = ks;
    }

    pub fn get_ks(&self) -> u8 {
        self.ks
    }

    pub fn press(&mut self, key: Key) {
        let mut data = 0;

        data |= 8;

        self.input = data ^ 0xff;
    }

    pub fn release(&mut self, key: Key) {
        self.input = 0xff;
    }

    pub fn input(&self) -> u8 {
        self.input
    }
}
