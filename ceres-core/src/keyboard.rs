//! The PC-1500's keyboard: the 65 real keys, the 8×8 matrix the ROM scans and the conversion of
//! ASCII characters to key presses.
//!
//! # The matrix
//! 64 of the 65 keys are in a matrix of 8 rows × 8 columns. The ROM scans it by writing a "row"
//! (one bit at 1 per row) to the LH5810's `DDA` register (`ISKEY_1`, $E41A:
//! `STA #(PC1500_PRT_A_DIR)`) and reading the 8 columns with `ITA` (one bit at 0 per key pressed;
//! the ROM inverts it with `EAI $FF`). See [`MATRIX`] and [`Keyboard::input`].
//!
//! The 65th key, `ON/BRK`, is NOT in the matrix: it goes to the LH5810's PB7 pin (see
//! `Keyboard::on_pulse`).
//!
//! # `SHIFT` is a prefix, not a modifier
//! On the real machine `SHIFT` (like `DEF` and `SML`) is NOT held together with the other key: it
//! is pressed and released, its indicator lights on the LCD (bit 1 of $764E) and it affects the
//! NEXT key (`AUTO_OFF_15`, $E3E8, toggles it; `AUTO_OFF_6_1`, $E39E, clears it after the next
//! key). Holding it down together with another key is no use: `KEY_2_ASCII` ($E42C) returns only
//! the FIRST key it finds in the scan, and `SHIFT` is in row 0, ahead of almost all of them, so the
//! other key is never seen. That is why [`KeyStroke`] is "key + whether `SHIFT` goes before it",
//! not a set of keys at once.

/// A physical key of the PC-1500.
///
/// The order of the variants is the index in [`Key::ALL`] and in `Keyboard::pressed_keys`
/// (`key as usize`): do not reorder them without also changing [`Key::ALL`] (the tests check it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    A,
    Asterisk,
    B,
    C,
    Cl,
    /// The `DEF` key (the name `Control` is kept for the code that already used it). It is a prefix
    /// like `SHIFT`: it lights the `DEF` indicator (bit 7 of $764E) for the next key.
    Control,
    D,
    Dot,
    Down,
    E,
    Eight,
    Enter,
    Equals,
    F,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    Five,
    Four,
    G,
    H,
    I,
    J,
    K,
    L,
    Left,
    LeftParen,
    M,
    Minus,
    Mode,
    N,
    Nine,
    O,
    Off,
    /// `ON/BRK`: outside the matrix (see `Keyboard::on_pulse`).
    On,
    One,
    P,
    Plus,
    Q,
    R,
    Rcl,
    Right,
    RightParen,
    /// `RSV`: changes the "layer" of reserved keys (code $09 → `RSV_CHNG`, $CB69). It is a real key
    /// of the bottom row, between `SML` and `RCL`.
    Rsv,
    S,
    Seven,
    Shift,
    Six,
    Slash,
    Sml,
    Space,
    T,
    Three,
    Two,
    U,
    Up,
    V,
    W,
    X,
    Y,
    Z,
    Zero,
}

/// The keyboard matrix, `MATRIX[row][bit]`: `row` is the bit of `KS` (the value the ROM writes to
/// `DDA`: row 0 = $01 ... row 7 = $80) and `bit` the bit read with `ITA`.
///
/// Checked against the ROM's two tables that turn a position into a code, `KB_LAYOUT_NORM` ($FE80)
/// and `KB_LAYOUT_SH` ($FEC0, with `SHIFT`), which `KEY_2_ASCII` ($E42C-$E44E) indexes as
/// `$80 + 8 × row + (7 − bit)` (see `tests/keyboard_mapping.rs`), and against `PockEmul`
/// (`pc1500.cpp`, `Cpc15XX::in()`), which matches bit for bit. The only key not in it is `ON/BRK`
/// ([`Key::On`]).
pub const MATRIX: [[Key; 8]; 8] = {
    use Key as K;
    [
        [K::Two, K::Five, K::Eight, K::H, K::Shift, K::Y, K::N, K::Up],
        [K::Dot, K::Minus, K::Off, K::S, K::F1, K::W, K::X, K::Rsv],
        [K::One, K::Four, K::Seven, K::J, K::F5, K::U, K::M, K::Zero],
        [K::RightParen, K::L, K::O, K::K, K::F6, K::I, K::LeftParen, K::Enter],
        [K::Plus, K::Asterisk, K::Slash, K::D, K::F2, K::E, K::C, K::Rcl],
        [K::Equals, K::Left, K::P, K::F, K::F3, K::R, K::V, K::Space],
        [K::Right, K::Mode, K::Cl, K::A, K::Control, K::Q, K::Z, K::Sml],
        [K::Three, K::Six, K::Nine, K::G, K::F4, K::T, K::B, K::Down],
    ]
};

impl Key {
    /// Number of keys: the 64 of the matrix plus `ON/BRK`.
    pub const COUNT: usize = 65;

    /// All the keys, in the enum's declaration order (so `Key::ALL[k as usize] == k`).
    pub const ALL: [Self; Self::COUNT] = {
        [
            Self::A,
            Self::Asterisk,
            Self::B,
            Self::C,
            Self::Cl,
            Self::Control,
            Self::D,
            Self::Dot,
            Self::Down,
            Self::E,
            Self::Eight,
            Self::Enter,
            Self::Equals,
            Self::F,
            Self::F1,
            Self::F2,
            Self::F3,
            Self::F4,
            Self::F5,
            Self::F6,
            Self::Five,
            Self::Four,
            Self::G,
            Self::H,
            Self::I,
            Self::J,
            Self::K,
            Self::L,
            Self::Left,
            Self::LeftParen,
            Self::M,
            Self::Minus,
            Self::Mode,
            Self::N,
            Self::Nine,
            Self::O,
            Self::Off,
            Self::On,
            Self::One,
            Self::P,
            Self::Plus,
            Self::Q,
            Self::R,
            Self::Rcl,
            Self::Right,
            Self::RightParen,
            Self::Rsv,
            Self::S,
            Self::Seven,
            Self::Shift,
            Self::Six,
            Self::Slash,
            Self::Sml,
            Self::Space,
            Self::T,
            Self::Three,
            Self::Two,
            Self::U,
            Self::Up,
            Self::V,
            Self::W,
            Self::X,
            Self::Y,
            Self::Z,
            Self::Zero,
        ]
    };

    /// Position `(row, bit)` of the key in [`MATRIX`], or `None` for `ON/BRK`, which is not in the
    /// matrix.
    #[must_use]
    pub const fn matrix_pos(self) -> Option<(u8, u8)> {
        let mut row = 0;
        while row < 8 {
            let mut bit = 0;
            while bit < 8 {
                if MATRIX[row][bit] as usize == self as usize {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "row and bit are < 8"
                    )]
                    return Some((row as u8, bit as u8));
                }
                bit += 1;
            }
            row += 1;
        }
        None
    }

    /// `true` for the 26 letters, the only keys whose result depends on the `SML` mode (lowercase):
    /// the ROM swaps upper and lower case only for letters (`AUTO_OFF_5..9`, $E38D-$E3AF).
    #[must_use]
    pub const fn is_letter(self) -> bool {
        matches!(
            self,
            Self::A | Self::B
                | Self::C
                | Self::D
                | Self::E
                | Self::F
                | Self::G
                | Self::H
                | Self::I
                | Self::J
                | Self::K
                | Self::L
                | Self::M
                | Self::N
                | Self::O
                | Self::P
                | Self::Q
                | Self::R
                | Self::S
                | Self::T
                | Self::U
                | Self::V
                | Self::W
                | Self::X
                | Self::Y
                | Self::Z
        )
    }

    /// The key's main label, as printed on it (the arrows as Unicode arrows).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Zero => "0",
            Self::One => "1",
            Self::Two => "2",
            Self::Three => "3",
            Self::Four => "4",
            Self::Five => "5",
            Self::Six => "6",
            Self::Seven => "7",
            Self::Eight => "8",
            Self::Nine => "9",
            Self::Asterisk => "*",
            Self::Dot => ".",
            Self::Equals => "=",
            Self::LeftParen => "(",
            Self::RightParen => ")",
            Self::Minus => "-",
            Self::Plus => "+",
            Self::Slash => "/",
            Self::Space => "SPACE",
            Self::Enter => "ENTER",
            Self::Cl => "CL",
            Self::Control => "DEF",
            Self::Mode => "MODE",
            Self::Off => "OFF",
            Self::On => "ON/BRK",
            Self::Rcl => "RCL",
            Self::Rsv => "RSV",
            Self::Shift => "SHIFT",
            Self::Sml => "SML",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::Left => "\u{2190}",
            Self::Right => "\u{2192}",
            Self::Up => "\u{2191}",
            Self::Down => "\u{2193}",
        }
    }

    /// The `SHIFT` label printed above the key (what it types after `SHIFT`), or `None` if it has
    /// none. Letters have no label: with `SHIFT` they give the lowercase letter (with `SML` off).
    ///
    /// It comes from `KB_LAYOUT_SH` ($FEC0), and from `EDCTRL_TBL` ($C38B) for the control codes:
    /// `DEL` ($1D), `INS` ($1C), `CA` ($1A) and `RESERVE` ($1E, "SHIFT-MODE").
    #[must_use]
    pub const fn shift_label(self) -> Option<&'static str> {
        Some(match self {
            Self::F1 => "!",
            Self::F2 => "\"",
            Self::F3 => "#",
            Self::F4 => "$",
            Self::F5 => "%",
            Self::F6 => "&",
            Self::Slash => "?",
            Self::Asterisk => ":",
            Self::Minus => ",",
            Self::Plus => ";",
            Self::LeftParen => "<",
            Self::RightParen => ">",
            Self::Equals => "@",
            Self::Space => "^",
            Self::Up => "[",
            Self::Down => "]",
            Self::Left => "DEL",
            Self::Right => "INS",
            Self::Cl => "CA",
            Self::Mode => "RESERVE",
            _ => return None,
        })
    }

    /// The code the key generates according to the ROM's tables (`KB_LAYOUT_NORM` $FE80 without
    /// `SHIFT`, `KB_LAYOUT_SH` $FEC0 with `SHIFT`), or `None` for `ON/BRK`, which is not in them.
    ///
    /// The control codes are those of `EDCTRL_TBL` ($C38B): $08 ←, $09 `RSV`, $0A ↓, $0B ↑, $0C →,
    /// $18 `CL`, $19 `RCL`, $1A `CA`, $1B `DEF`, $1C `INS`, $1D `DEL`, $1E `RESERVE`, $1F `MODE`;
    /// besides, $01 `SHIFT`, $02 `SML`, $0D `ENTER` and $0F `OFF`. With `SHIFT` letters give the
    /// lowercase letter. It is used to check [`MATRIX`] against the ROM.
    #[must_use]
    pub const fn rom_code(self, shift: bool) -> Option<u8> {
        let (normal, shifted) = match self {
            Self::On => return None,
            Self::Zero => (b'0', b'0'),
            Self::One => (b'1', b'1'),
            Self::Two => (b'2', b'2'),
            Self::Three => (b'3', b'3'),
            Self::Four => (b'4', b'4'),
            Self::Five => (b'5', b'5'),
            Self::Six => (b'6', b'6'),
            Self::Seven => (b'7', b'7'),
            Self::Eight => (b'8', b'8'),
            Self::Nine => (b'9', b'9'),
            Self::Dot => (b'.', b'.'),
            Self::Minus => (b'-', b','),
            Self::Plus => (b'+', b';'),
            Self::Asterisk => (b'*', b':'),
            Self::Slash => (b'/', b'?'),
            Self::Equals => (b'=', b'@'),
            Self::LeftParen => (b'(', b'<'),
            Self::RightParen => (b')', b'>'),
            Self::Space => (b' ', b'^'),
            Self::F1 => (0x11, b'!'),
            Self::F2 => (0x12, b'"'),
            Self::F3 => (0x13, b'#'),
            Self::F4 => (0x14, b'$'),
            Self::F5 => (0x15, b'%'),
            Self::F6 => (0x16, b'&'),
            Self::Up => (0x0B, b'['),
            Self::Down => (0x0A, b']'),
            Self::Left => (0x08, 0x1D),
            Self::Right => (0x0C, 0x1C),
            Self::Cl => (0x18, 0x1A),
            Self::Mode => (0x1F, 0x1E),
            Self::Shift => (0x01, 0x01),
            Self::Sml => (0x02, 0x02),
            Self::Rsv => (0x09, 0x09),
            Self::Enter => (0x0D, 0x0D),
            Self::Off => (0x0F, 0x0F),
            Self::Rcl => (0x19, 0x19),
            Self::Control => (0x1B, 0x1B),
            letter => {
                // Letter: the label is the uppercase letter; with SHIFT, + $20.
                let upper = letter.label().as_bytes()[0];
                (upper, upper + 0x20)
            }
        };
        Some(if shift { shifted } else { normal })
    }
}

/// A logical key press: a physical key of the PC-1500, optionally after `SHIFT`.
///
/// On the PC-1500 `SHIFT` is NOT held together with the other key: it is a prefix key (pressed and
/// released, it lights the LCD's `SHIFT` indicator and affects the next key). Besides, even if it
/// were held, `KEY_2_ASCII` ($E42C) returns only the FIRST key it finds in the matrix scan, so two
/// keys at once make up nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyStroke {
    pub key: Key,
    pub shift: bool,
}

impl KeyStroke {
    #[must_use]
    pub const fn plain(key: Key) -> Self {
        Self { key, shift: false }
    }

    #[must_use]
    pub const fn shifted(key: Key) -> Self {
        Self { key, shift: true }
    }
}

/// Converts an ASCII character to the PC-1500 key press that produces it, or `None` if no key
/// generates it.
///
/// Taken from the ROM's two keyboard tables, `KB_LAYOUT_NORM` ($FE80) and `KB_LAYOUT_SH` ($FEC0,
/// with `SHIFT`), crossed with [`MATRIX`] (row = bit of `KS`, column = bit read by `ITA`;
/// `KEY_2_ASCII` indexes the table as row × 8 + (7 − bit)):
///
/// - Without `SHIFT`: `A`-`Z`, `0`-`9`, space, `\n` (ENTER), `.` `-` `+` `*` `/` `=` `(` `)`.
/// - With `SHIFT`: `!` `"` `#` `$` `%` `&` (F1-F6), `,` (`-`), `<` (`(`), `>` (`)`), `?` (`/`), `:`
///   (`*`), `;` (`+`), `@` (`=`), `^` (space), `[` (↑), `]` (↓), and `a`-`z` (letter with `SHIFT`).
///
/// Lowercase letters assume the `SML` mode off (the one at boot): with `SML` on, the ROM swaps
/// upper and lower case (the GUI's key feeder takes it into account, see `ShiftPolicy::Exact`).
///
/// With no key (they return `None`): `'` `_` `` `
/// `` `{` `|` `}` `~` `\` and any non-ASCII character or control character other than `\n`. The glyph of `_`
/// exists in the ROM (it is the cursor itself), but no key generates it.
#[must_use]
pub fn key_for_char(c: char) -> Option<KeyStroke> {
    use Key as K;
    let plain = |k| Some(KeyStroke::plain(k));
    let shifted = |k| Some(KeyStroke::shifted(k));
    match c {
        'A'..='Z' => plain(letter_key(c as u8 - b'A')),
        'a'..='z' => shifted(letter_key(c as u8 - b'a')),
        '0'..='9' => plain(digit_key(c as u8 - b'0')),
        ' ' => plain(K::Space),
        '\n' => plain(K::Enter),
        '.' => plain(K::Dot),
        '-' => plain(K::Minus),
        '+' => plain(K::Plus),
        '*' => plain(K::Asterisk),
        '/' => plain(K::Slash),
        '=' => plain(K::Equals),
        '(' => plain(K::LeftParen),
        ')' => plain(K::RightParen),
        '!' => shifted(K::F1),
        '"' => shifted(K::F2),
        '#' => shifted(K::F3),
        '$' => shifted(K::F4),
        '%' => shifted(K::F5),
        '&' => shifted(K::F6),
        ',' => shifted(K::Minus),
        '<' => shifted(K::LeftParen),
        '>' => shifted(K::RightParen),
        '?' => shifted(K::Slash),
        ':' => shifted(K::Asterisk),
        ';' => shifted(K::Plus),
        '@' => shifted(K::Equals),
        '^' => shifted(K::Space),
        '[' => shifted(K::Up),
        ']' => shifted(K::Down),
        _ => None,
    }
}

const fn letter_key(index: u8) -> Key {
    const LETTERS: [Key; 26] = [
        Key::A,
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::I,
        Key::J,
        Key::K,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Y,
        Key::Z,
    ];
    LETTERS[index as usize]
}

const fn digit_key(index: u8) -> Key {
    const DIGITS: [Key; 10] = [
        Key::Zero,
        Key::One,
        Key::Two,
        Key::Three,
        Key::Four,
        Key::Five,
        Key::Six,
        Key::Seven,
        Key::Eight,
        Key::Nine,
    ];
    DIGITS[index as usize]
}

/// Translates a whole text to the sequence of key presses that types it.
///
/// `\r` is ignored (to accept files with CRLF line ends as they are) and `\n` is ENTER. It returns
/// `Err(c)` with the first character that cannot be typed (see [`key_for_char`]), without typing
/// anything.
pub fn text_to_keystrokes(text: &str) -> Result<Vec<KeyStroke>, char> {
    text.chars()
        .filter(|&c| c != '\r')
        .map(|c| key_for_char(c).ok_or(c))
        .collect()
}

#[derive(Debug)]
pub struct Keyboard {
    ks: u8,
    pressed_keys: [bool; Key::COUNT],
    /// Pending pulse of the `ON/BRK` key (see [`Keyboard::take_on_pulse`]).
    ///
    /// `ON/BRK` is not part of the keyboard matrix the ROM scans with `KS`/`ITA` (that is why it
    /// does not appear in [`Keyboard::input`]): on the real PC-1500 it is wired to the LH5810's PB7
    /// pin, which "latches" it in bit 1 of its `IF` register. The ROM only checks that bit
    /// (`CHK_BRK`, $E451) and clears it itself when it serves the BREAK ($C4C6/$C4ED), so what must
    /// reach it is an EDGE per key press, not a level: if PB7 stayed high while the key is held,
    /// `IF` would be set again right after the ROM cleared it, and a second BREAK would land in the
    /// editor (`BCMD_STOP_6` → `WARM_START`), clearing the `BREAK IN nn` message just written. It
    /// is what `PockEmul` does with `Ckeyb::CheckKon()` (a one-shot `Kon`).
    ///
    /// It is set on the rising edge of `press(Key::On)` and survives even if the key is released
    /// before the CPU gets to run anything (e.g. a press and release within the same repaint of the
    /// GUI): it is not lost until `Pc1500::step` picks it up.
    on_pulse: bool,
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            ks: 0,
            pressed_keys: [false; Key::COUNT],
            on_pulse: false,
        }
    }

    pub fn set_ks(&mut self, ks: u8) {
        self.ks = ks;
    }

    pub fn press(&mut self, key: Key) {
        // Only the rising edge: calling `press(Key::On)` again with the key already down (the
        // operating system's key repeat, the GUI reasserting a held key...) does not generate
        // another BREAK. See `on_pulse`.
        if key == Key::On && !self.pressed_keys[key as usize] {
            self.on_pulse = true;
        }
        self.pressed_keys[key as usize] = true;
    }

    pub fn release(&mut self, key: Key) {
        self.pressed_keys[key as usize] = false;
    }

    /// `true` if `key` is down right now.
    #[must_use]
    pub const fn is_pressed(&self, key: Key) -> bool {
        self.pressed_keys[key as usize]
    }

    /// `true` if there is an `ON/BRK` pulse waiting to be picked up (see
    /// [`Keyboard::take_on_pulse`]), without picking it up.
    #[must_use]
    pub const fn on_pulse_pending(&self) -> bool {
        self.on_pulse
    }

    /// Picks up (and clears) the pending `ON/BRK` pulse: `true` only once for each press of the
    /// key, however long it is held. See `on_pulse` for why.
    pub const fn take_on_pulse(&mut self) -> bool {
        let pulse = self.on_pulse;
        self.on_pulse = false;
        pulse
    }

    /// What the ROM reads with `ITA` for the row (or rows) selected in `KS`: one bit at 0 for each
    /// column with some key down in any of those rows (see [`MATRIX`]). It used to be a chain of 64
    /// `if`s written by hand, with the same result.
    #[must_use]
    pub fn input(&self) -> u8 {
        let mut data = 0;
        for (row, keys) in MATRIX.iter().enumerate() {
            if self.ks & (1 << row) == 0 {
                continue;
            }
            for (bit, &key) in keys.iter().enumerate() {
                if self.pressed_keys[key as usize] {
                    data |= 1 << bit;
                }
            }
        }
        data ^ 0xFF
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, KeyStroke, Keyboard, MATRIX, key_for_char};

    #[test]
    fn on_pulse_is_one_shot_per_press() {
        let mut kb = Keyboard::new();
        assert!(!kb.take_on_pulse());

        kb.press(Key::On);
        // Reasserting the key press (key repeat) does not generate another pulse.
        kb.press(Key::On);
        assert!(kb.is_pressed(Key::On));
        assert!(kb.take_on_pulse());
        assert!(!kb.take_on_pulse(), "only once");
        kb.press(Key::On);
        assert!(!kb.take_on_pulse(), "held, it does not fire again");

        kb.release(Key::On);
        assert!(!kb.take_on_pulse());
        kb.press(Key::On);
        assert!(kb.take_on_pulse(), "new press, new pulse");
    }

    #[test]
    fn on_pulse_survives_release_before_being_taken() {
        let mut kb = Keyboard::new();
        kb.press(Key::On);
        kb.release(Key::On);
        assert!(!kb.is_pressed(Key::On));
        assert!(kb.take_on_pulse());
    }

    #[test]
    fn on_is_not_part_of_the_matrix() {
        let mut kb = Keyboard::new();
        kb.set_ks(0xFF);
        let idle = kb.input();
        kb.press(Key::On);
        assert_eq!(kb.input(), idle, "ON changes no column");
        assert_eq!(Key::On.matrix_pos(), None);
    }

    #[test]
    fn all_is_in_declaration_order() {
        for (index, key) in Key::ALL.iter().enumerate() {
            assert_eq!(*key as usize, index, "{key:?}");
        }
    }

    /// Each key pressed alone clears exactly its bit, and only when its row is selected in `KS`:
    /// the same as the 64 `if`s written by hand before `MATRIX`.
    #[test]
    fn input_reads_one_bit_per_key_and_row() {
        for key in Key::ALL {
            let Some((row, bit)) = key.matrix_pos() else {
                continue;
            };
            let mut kb = Keyboard::new();
            kb.press(key);
            for strobe in 0..8 {
                kb.set_ks(1 << strobe);
                let expected = if strobe == row { !(1_u8 << bit) } else { 0xFF };
                assert_eq!(kb.input(), expected, "{key:?} with KS={:#04X}", 1 << strobe);
            }
            kb.set_ks(0xFF);
            assert_eq!(kb.input(), !(1_u8 << bit), "{key:?} with all the rows");
            kb.set_ks(0);
            assert_eq!(kb.input(), 0xFF, "{key:?} with no rows");
        }
    }

    #[test]
    fn several_keys_combine_by_column() {
        let mut kb = Keyboard::new();
        kb.press(Key::Two); // fila 0 bit 0
        kb.press(Key::Dot); // fila 1 bit 0
        kb.press(Key::Down); // fila 7 bit 7
        kb.set_ks(0x03);
        assert_eq!(kb.input(), 0xFE, "two rows, same column");
        kb.set_ks(0x83);
        assert_eq!(kb.input(), 0x7E);
        kb.release(Key::Dot);
        kb.set_ks(0x02);
        assert_eq!(kb.input(), 0xFF);
    }

    #[test]
    fn matrix_pos_matches_matrix() {
        for (row, keys) in MATRIX.iter().enumerate() {
            for (bit, key) in keys.iter().enumerate() {
                let pos = key.matrix_pos().map(|(r, b)| (usize::from(r), usize::from(b)));
                assert_eq!(pos, Some((row, bit)), "{key:?}");
            }
        }
    }

    #[test]
    fn shift_labels_match_char_table() {
        // Each one-character SHIFT label is exactly what `key_for_char` types with SHIFT before it.
        for key in Key::ALL {
            if let Some(label) = key.shift_label()
                && let [byte] = label.as_bytes()
            {
                assert_eq!(
                    key_for_char(char::from(*byte)),
                    Some(KeyStroke::shifted(key)),
                    "{key:?}"
                );
            }
        }
    }
}
