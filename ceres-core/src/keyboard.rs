#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    A,
    Asterisk,
    B,
    C,
    Cl,
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
    On,
    One,
    P,
    Plus,
    Q,
    Quote,
    R,
    Rcl,
    Right,
    RightParen,
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

#[derive(Debug)]
pub struct Keyboard {
    ks: u8,
    pressed_keys: [bool; 68],
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            ks: 0,
            pressed_keys: [false; 68],
        }
    }

    pub fn set_ks(&mut self, ks: u8) {
        self.ks = ks;
    }

    fn get_ks(&self) -> u8 {
        self.ks
    }

    pub fn press(&mut self, key: Key) {
        self.pressed_keys[key as usize] = true;
    }

    pub fn release(&mut self, key: Key) {
        self.pressed_keys[key as usize] = false;
    }

    pub fn input(&self) -> u8 {
        let mut data = 0;

        if self.get_ks() & 0x01 != 0 {
            if self.pressed_keys[Key::Two as usize] {
                data |= 1;
            }
            if self.pressed_keys[Key::Five as usize] {
                data |= 2;
            }
            if self.pressed_keys[Key::Eight as usize] {
                data |= 4;
            }
            if self.pressed_keys[Key::H as usize] {
                data |= 8;
            }
            if self.pressed_keys[Key::Shift as usize] {
                data |= 0x10;
            }
            if self.pressed_keys[Key::Y as usize] {
                data |= 0x20;
            }
            if self.pressed_keys[Key::N as usize] {
                data |= 0x40;
            }
            if self.pressed_keys[Key::Up as usize] {
                data |= 0x80;
            }
        }

        if self.get_ks() & 0x02 != 0 {
            if self.pressed_keys[Key::Dot as usize] {
                data |= 1;
            }
            if self.pressed_keys[Key::Minus as usize] {
                data |= 2;
            }
            if self.pressed_keys[Key::Off as usize] {
                data |= 4;
            }
            if self.pressed_keys[Key::S as usize] {
                data |= 8;
            }
            if self.pressed_keys[Key::F1 as usize] {
                data |= 0x10;
            }
            if self.pressed_keys[Key::W as usize] {
                data |= 0x20;
            }
            if self.pressed_keys[Key::X as usize] {
                data |= 0x40;
            }
            if self.pressed_keys[Key::Rsv as usize] {
                data |= 0x80;
            }
        }

        if self.get_ks() & 0x4 != 0 {
            if self.pressed_keys[Key::One as usize] {
                data |= 1;
            }
            if self.pressed_keys[Key::Four as usize] {
                data |= 2;
            }
            if self.pressed_keys[Key::Seven as usize] {
                data |= 4;
            }
            if self.pressed_keys[Key::J as usize] {
                data |= 8;
            }
            if self.pressed_keys[Key::F5 as usize] {
                data |= 0x10;
            }
            if self.pressed_keys[Key::U as usize] {
                data |= 0x20;
            }
            if self.pressed_keys[Key::M as usize] {
                data |= 0x40;
            }
            if self.pressed_keys[Key::Zero as usize] {
                data |= 0x80;
            }
        }

        if self.get_ks() & 0x8 != 0 {
            if self.pressed_keys[Key::RightParen as usize] {
                data |= 1;
            }
            if self.pressed_keys[Key::L as usize] {
                data |= 2;
            }
            if self.pressed_keys[Key::O as usize] {
                data |= 4;
            }
            if self.pressed_keys[Key::K as usize] {
                data |= 8;
            }
            if self.pressed_keys[Key::F6 as usize] {
                data |= 0x10;
            }
            if self.pressed_keys[Key::I as usize] {
                data |= 0x20;
            }
            if self.pressed_keys[Key::LeftParen as usize] {
                data |= 0x40;
            }
            if self.pressed_keys[Key::Enter as usize] {
                data |= 0x80;
            }
        }

        if self.get_ks() & 0x10 != 0 {
            if self.pressed_keys[Key::Plus as usize] {
                data |= 1;
            }
            if self.pressed_keys[Key::Asterisk as usize] {
                data |= 2;
            }
            if self.pressed_keys[Key::Slash as usize] {
                data |= 4;
            }
            if self.pressed_keys[Key::D as usize] {
                data |= 8;
            }
            if self.pressed_keys[Key::F2 as usize] {
                data |= 0x10;
            }
            if self.pressed_keys[Key::E as usize] {
                data |= 0x20;
            }
            if self.pressed_keys[Key::C as usize] {
                data |= 0x40;
            }
            if self.pressed_keys[Key::Rcl as usize] {
                data |= 0x80;
            }
        }

        if self.get_ks() & 0x20 != 0 {
            if self.pressed_keys[Key::Equals as usize] {
                data |= 1;
            }

            if self.pressed_keys[Key::Left as usize] {
                data |= 2;
            }

            if self.pressed_keys[Key::P as usize] {
                data |= 4;
            }

            if self.pressed_keys[Key::F as usize] {
                data |= 8;
            }

            if self.pressed_keys[Key::F3 as usize] {
                data |= 0x10;
            }

            if self.pressed_keys[Key::R as usize] {
                data |= 0x20;
            }

            if self.pressed_keys[Key::V as usize] {
                data |= 0x40;
            }

            if self.pressed_keys[Key::Space as usize] {
                data |= 0x80;
            }
        }

        if self.get_ks() & 0x40 != 0 {
            if self.pressed_keys[Key::Right as usize] {
                data |= 1;
            }
            if self.pressed_keys[Key::Mode as usize] {
                data |= 2;
            }
            if self.pressed_keys[Key::Cl as usize] {
                data |= 4;
            }
            if self.pressed_keys[Key::A as usize] {
                data |= 8;
            }
            if self.pressed_keys[Key::Control as usize] {
                data |= 0x10;
            }
            if self.pressed_keys[Key::Q as usize] {
                data |= 0x20;
            }
            if self.pressed_keys[Key::Z as usize] {
                data |= 0x40;
            }
            if self.pressed_keys[Key::Sml as usize] {
                data |= 0x80;
            }
        }

        if self.get_ks() & 0x80 != 0 {
            if self.pressed_keys[Key::Three as usize] {
                data |= 1;
            }
            if self.pressed_keys[Key::Six as usize] {
                data |= 2;
            }
            if self.pressed_keys[Key::Nine as usize] {
                data |= 4;
            }
            if self.pressed_keys[Key::G as usize] {
                data |= 8;
            }
            if self.pressed_keys[Key::F4 as usize] {
                data |= 0x10;
            }
            if self.pressed_keys[Key::T as usize] {
                data |= 0x20;
            }
            if self.pressed_keys[Key::B as usize] {
                data |= 0x40;
            }
            if self.pressed_keys[Key::Down as usize] {
                data |= 0x80;
            }
        }

        data ^ 0xff
    }
}
