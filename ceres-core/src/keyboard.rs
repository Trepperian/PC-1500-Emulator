//! Teclado de la PC-1500: las 65 teclas reales, la matriz de 8×8 que barre
//! la ROM y la conversión de caracteres ASCII a pulsaciones.
//!
//! # La matriz
//! 64 de las 65 teclas están en una matriz de 8 filas × 8 columnas. La ROM
//! la barre escribiendo una "fila" (un bit a 1 por fila) en el registro
//! `DDA` del LH5810 (`ISKEY_1`, $E41A: `STA #(PC1500_PRT_A_DIR)`) y
//! leyendo las 8 columnas con `ITA` (un bit a 0 por tecla pulsada; la ROM
//! lo invierte con `EAI $FF`). Ver [`MATRIX`] y [`Keyboard::input`].
//!
//! La 65ª tecla, `ON/BRK`, NO está en la matriz: va a la patilla PB7 del
//! LH5810 (ver `Keyboard::on_pulse`).
//!
//! # `SHIFT` es un prefijo, no un modificador
//! En la máquina real `SHIFT` (igual que `DEF` y `SML`) NO se mantiene a
//! la vez que la otra tecla: se pulsa y se suelta, se enciende su
//! indicador en el LCD (bit 1 de $764E) y afecta a la SIGUIENTE tecla
//! (`AUTO_OFF_15`, $E3E8, lo invierte; `AUTO_OFF_6_1`, $E39E, lo borra
//! tras la siguiente tecla). Mantenerla pulsada junto con otra no sirve
//! de nada: `KEY_2_ASCII` ($E42C) devuelve solo la PRIMERA tecla que
//! encuentra en el barrido y `SHIFT` está en la fila 0, por delante de
//! casi todas, así que la otra tecla nunca se ve. Por eso [`KeyStroke`]
//! es "tecla + si va precedida de `SHIFT`", no un conjunto de teclas a
//! la vez.

/// Una tecla física de la PC-1500.
///
/// El orden de las variantes es el índice de [`Key::ALL`] y de
/// `Keyboard::pressed_keys` (`key as usize`): no reordenar sin cambiar
/// también [`Key::ALL`] (lo comprueban los tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    A,
    Asterisk,
    B,
    C,
    Cl,
    /// La tecla `DEF` (el nombre `Control` se conserva por compatibilidad
    /// con el código que ya la usaba). Es un prefijo como `SHIFT`: activa
    /// el indicador `DEF` (bit 7 de $764E) para la siguiente tecla.
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
    /// `ON/BRK`: fuera de la matriz (ver `Keyboard::on_pulse`).
    On,
    One,
    P,
    Plus,
    Q,
    R,
    Rcl,
    Right,
    RightParen,
    /// `RSV`: cambia de "capa" de teclas reservadas (código $09 →
    /// `RSV_CHNG`, $CB69). Es una tecla real de la fila inferior, entre
    /// `SML` y `RCL`.
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

/// La matriz de teclado, `MATRIX[fila][bit]`: `fila` es el bit de `KS`
/// (el valor que escribe la ROM en `DDA`: fila 0 = $01 ... fila 7 = $80)
/// y `bit` el bit que se lee con `ITA`.
///
/// Comprobada contra las dos tablas de la ROM que convierten posición a
/// código, `KB_LAYOUT_NORM` ($FE80) y `KB_LAYOUT_SH` ($FEC0, con
/// `SHIFT`), que `KEY_2_ASCII` ($E42C-$E44E) indexa como
/// `$80 + 8 × fila + (7 − bit)` (ver `tests/keyboard_mapping.rs`), y
/// contra `PockEmul` (`pc1500.cpp`, `Cpc15XX::in()`), que coincide bit a
/// bit. La única tecla que no está es `ON/BRK` ([`Key::On`]).
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
    /// Número de teclas: las 64 de la matriz más `ON/BRK`.
    pub const COUNT: usize = 65;

    /// Todas las teclas, en el orden de declaración del enum (así
    /// `Key::ALL[k as usize] == k`).
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

    /// Posición `(fila, bit)` de la tecla en [`MATRIX`], o `None` para
    /// `ON/BRK`, que no está en la matriz.
    #[must_use]
    pub const fn matrix_pos(self) -> Option<(u8, u8)> {
        let mut row = 0;
        while row < 8 {
            let mut bit = 0;
            while bit < 8 {
                if MATRIX[row][bit] as usize == self as usize {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "fila y bit son < 8"
                    )]
                    return Some((row as u8, bit as u8));
                }
                bit += 1;
            }
            row += 1;
        }
        None
    }

    /// `true` para las 26 letras, las únicas teclas cuyo resultado
    /// depende del modo `SML` (minúsculas): la ROM invierte
    /// mayúsculas/minúsculas solo en las letras (`AUTO_OFF_5..9`,
    /// $E38D-$E3AF).
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

    /// El rótulo principal de la tecla, tal como va impreso en ella (las
    /// flechas como flechas Unicode).
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

    /// El rótulo de `SHIFT` impreso encima de la tecla (lo que teclea
    /// precedida de `SHIFT`), o `None` si no tiene. Las letras no llevan
    /// rótulo: con `SHIFT` dan la minúscula (con `SML` apagado).
    ///
    /// Sale de `KB_LAYOUT_SH` ($FEC0) y de `EDCTRL_TBL` ($C38B) para los
    /// códigos de control: `DEL` ($1D), `INS` ($1C), `CA` ($1A) y
    /// `RESERVE` ($1E, "SHIFT-MODE").
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

    /// El código que genera la tecla según las tablas de la ROM
    /// (`KB_LAYOUT_NORM` $FE80 sin `SHIFT`, `KB_LAYOUT_SH` $FEC0 con
    /// `SHIFT`), o `None` para `ON/BRK`, que no está en ellas.
    ///
    /// Los códigos de control son los de `EDCTRL_TBL` ($C38B): $08 ←, $09
    /// `RSV`, $0A ↓, $0B ↑, $0C →, $18 `CL`, $19 `RCL`, $1A `CA`, $1B
    /// `DEF`, $1C `INS`, $1D `DEL`, $1E `RESERVE`, $1F `MODE`; además $01
    /// `SHIFT`, $02 `SML`, $0D `ENTER` y $0F `OFF`. Con `SHIFT` las letras
    /// dan la minúscula. Sirve para comprobar [`MATRIX`] contra la ROM.
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
                // Letra: el rótulo es la mayúscula; con SHIFT, + $20.
                let upper = letter.label().as_bytes()[0];
                (upper, upper + 0x20)
            }
        };
        Some(if shift { shifted } else { normal })
    }
}

/// Una pulsación lógica: una tecla física de la PC-1500, opcionalmente
/// precedida de `SHIFT`.
///
/// En la PC-1500 `SHIFT` NO se mantiene a la vez que la otra tecla: es
/// una tecla de prefijo (se pulsa y se suelta, se enciende el indicador
/// `SHIFT` del LCD y afecta a la siguiente tecla). Además, aunque se
/// mantuviera, `KEY_2_ASCII` ($E42C) devuelve solo la PRIMERA tecla que
/// encuentra en el barrido de la matriz, así que dos teclas a la vez no
/// componen nada.
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

/// Convierte un carácter ASCII a la pulsación de la PC-1500 que lo
/// produce, o `None` si no hay ninguna tecla que lo genere.
///
/// Sacado de las dos tablas de teclado de la ROM, `KB_LAYOUT_NORM`
/// ($FE80) y `KB_LAYOUT_SH` ($FEC0, con `SHIFT`), cruzadas con
/// [`MATRIX`] (fila = bit de `KS`, columna = bit leído por `ITA`;
/// `KEY_2_ASCII` indexa la tabla como fila × 8 + (7 − bit)):
///
/// - Sin `SHIFT`: `A`-`Z`, `0`-`9`, espacio, `\n` (ENTER), `.` `-` `+`
///   `*` `/` `=` `(` `)`.
/// - Con `SHIFT`: `!` `"` `#` `$` `%` `&` (F1-F6), `,` (`-`), `<` (`(`),
///   `>` (`)`), `?` (`/`), `:` (`*`), `;` (`+`), `@` (`=`), `^`
///   (espacio), `[` (↑), `]` (↓), y `a`-`z` (letra con `SHIFT`).
///
/// Las minúsculas asumen el modo `SML` apagado (el de arranque): con
/// `SML` encendido la ROM invierte mayúsculas/minúsculas (el alimentador
/// de teclas de la GUI lo tiene en cuenta, ver `ShiftPolicy::Exact`).
///
/// No tienen tecla (devuelven `None`): `'` `_` `` ` `` `{` `|` `}` `~`
/// `\` y cualquier carácter no ASCII o de control distinto de `\n`. El
/// glifo de `_` existe en la ROM (es el propio cursor), pero ninguna
/// tecla lo genera.
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

/// Traduce un texto entero a la secuencia de pulsaciones que lo teclea.
///
/// `\r` se ignora (para aceptar ficheros con saltos de línea CRLF tal
/// cual) y `\n` es ENTER. Devuelve `Err(c)` con el primer carácter que no
/// se puede teclear (ver [`key_for_char`]), sin teclear nada.
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
    /// Pulso pendiente de la tecla `ON/BRK` (ver [`Keyboard::take_on_pulse`]).
    ///
    /// `ON/BRK` no forma parte de la matriz de teclado que barre la ROM
    /// con `KS`/`ITA` (por eso no aparece en [`Keyboard::input`]): en la
    /// PC-1500 real va cableada a la patilla PB7 del LH5810, que la
    /// "engancha" en el bit 1 de su registro `IF`. La ROM solo consulta
    /// ese bit (`CHK_BRK`, $E451) y lo borra ella misma cuando atiende
    /// el BREAK ($C4C6/$C4ED), así que lo que tiene que llegarle es un
    /// FLANCO por pulsación, no un nivel: si PB7 siguiera alto mientras
    /// la tecla está mantenida, el `IF` se volvería a poner a 1 justo
    /// después de que la ROM lo borrara y un segundo BREAK caería en el
    /// editor (`BCMD_STOP_6` → `WARM_START`), borrando el mensaje
    /// `BREAK IN nn` recién escrito. Es lo mismo que hace `PockEmul` con
    /// `Ckeyb::CheckKon()` (`Kon` de un solo disparo).
    ///
    /// Se activa en el flanco de subida de `press(Key::On)` y sobrevive
    /// aunque la tecla se suelte antes de que la CPU llegue a ejecutar
    /// nada (p. ej. una pulsación y suelta dentro del mismo repintado de
    /// la GUI): hasta que no lo recoge `Pc1500::step` no se pierde.
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
        // Solo el flanco de subida: volver a llamar a `press(Key::On)`
        // con la tecla ya pulsada (autorrepetición del sistema operativo,
        // la GUI reafirmando una tecla mantenida...) no genera otro
        // BREAK. Ver `on_pulse`.
        if key == Key::On && !self.pressed_keys[key as usize] {
            self.on_pulse = true;
        }
        self.pressed_keys[key as usize] = true;
    }

    pub fn release(&mut self, key: Key) {
        self.pressed_keys[key as usize] = false;
    }

    /// `true` si `key` está pulsada ahora mismo.
    #[must_use]
    pub const fn is_pressed(&self, key: Key) -> bool {
        self.pressed_keys[key as usize]
    }

    /// `true` si hay un pulso de `ON/BRK` pendiente de recoger (ver
    /// [`Keyboard::take_on_pulse`]), sin recogerlo.
    #[must_use]
    pub const fn on_pulse_pending(&self) -> bool {
        self.on_pulse
    }

    /// Recoge (y borra) el pulso pendiente de `ON/BRK`: `true` una sola
    /// vez por cada pulsación de la tecla, por mucho que se mantenga.
    /// Ver `on_pulse` para el porqué.
    pub const fn take_on_pulse(&mut self) -> bool {
        let pulse = self.on_pulse;
        self.on_pulse = false;
        pulse
    }

    /// Lo que lee la ROM con `ITA` para la fila (o filas) que tiene
    /// seleccionadas en `KS`: un bit a 0 por cada columna con alguna tecla
    /// pulsada en alguna de esas filas (ver [`MATRIX`]). Antes era una
    /// cadena de 64 `if` escritos a mano, con el mismo resultado.
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
        // Reafirmar la pulsación (autorrepetición) no genera otro pulso.
        kb.press(Key::On);
        assert!(kb.is_pressed(Key::On));
        assert!(kb.take_on_pulse());
        assert!(!kb.take_on_pulse(), "solo una vez");
        kb.press(Key::On);
        assert!(!kb.take_on_pulse(), "mantenida no vuelve a disparar");

        kb.release(Key::On);
        assert!(!kb.take_on_pulse());
        kb.press(Key::On);
        assert!(kb.take_on_pulse(), "nueva pulsaci\u{f3}n, nuevo pulso");
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
        assert_eq!(kb.input(), idle, "ON no cambia ninguna columna");
        assert_eq!(Key::On.matrix_pos(), None);
    }

    #[test]
    fn all_is_in_declaration_order() {
        for (index, key) in Key::ALL.iter().enumerate() {
            assert_eq!(*key as usize, index, "{key:?}");
        }
    }

    /// Cada tecla pulsada sola pone a 0 exactamente su bit, y solo cuando
    /// su fila está seleccionada en `KS`: lo mismo que hacían los 64 `if`
    /// escritos a mano de antes de `MATRIX`.
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
                assert_eq!(kb.input(), expected, "{key:?} con KS={:#04X}", 1 << strobe);
            }
            kb.set_ks(0xFF);
            assert_eq!(kb.input(), !(1_u8 << bit), "{key:?} con todas las filas");
            kb.set_ks(0);
            assert_eq!(kb.input(), 0xFF, "{key:?} sin filas");
        }
    }

    #[test]
    fn several_keys_combine_by_column() {
        let mut kb = Keyboard::new();
        kb.press(Key::Two); // fila 0 bit 0
        kb.press(Key::Dot); // fila 1 bit 0
        kb.press(Key::Down); // fila 7 bit 7
        kb.set_ks(0x03);
        assert_eq!(kb.input(), 0xFE, "dos filas, misma columna");
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
        // Cada rótulo de SHIFT de un solo carácter es justo lo que teclea
        // `key_for_char` con SHIFT delante.
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
