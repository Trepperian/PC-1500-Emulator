use chrono::{Datelike, Timelike};

pub const FREQUENCY: usize = 2600000 / 2;

pub struct Pd1990ac {
    seconds: u16,
    minutes: u16,
    hours: u16,
    days: u16,
    weekday: u16,
    month: u16,

    // Signals
    c0: bool,
    c1: bool,
    c2: bool,
    stb: bool,
    cs: bool,
    data_in: bool,
    gnd: bool,
    clk: bool,
    data_out: bool,
    tp: bool,
    out_enable: bool,
    n_xtal: bool,
    xtal: bool,
    vdd: bool,

    // Internal
    mode: u8,
    bitno: u8,
    current_bit: u8,
    new_mode: bool,
    new_clk: bool,
    prev_mode: u8,
    prev_clk: bool,
    flip_clk: bool,
    tp_frequency: usize,

    previous_state: usize,
    previous_state_tp: usize,
}

fn hex2bcd(d: u32) -> u16 {
    let a = d / 100;
    let b = d - (a * 100);
    let c = b / 10;
    ((a as u16) << 8) + ((c as u16) << 4) + (b as u16) - ((c as u16) * 10)
}

fn read_bit(value: u16, position: u8) -> bool {
    ((value >> position) & 0x01) != 0
}

fn set_bit(value: &mut u16, position: u8) {
    *value |= 0x01 << position;
}

fn unset_bit(value: &mut u16, position: u8) {
    *value &= !(0x01 << position);
}

fn put_bit(value: &mut u16, position: u8, bit: bool) {
    if bit {
        set_bit(value, position);
    } else {
        unset_bit(value, position);
    }
}

impl Pd1990ac {
    pub fn new() -> Self {
        let now = std::time::SystemTime::now();
        let datetime: chrono::DateTime<chrono::Utc> = now.into();
        Self {
            seconds: hex2bcd(datetime.second()),
            minutes: hex2bcd(datetime.minute()),
            hours: hex2bcd(datetime.hour()),
            days: hex2bcd(datetime.day()),
            weekday: hex2bcd(datetime.weekday().num_days_from_sunday()),
            month: datetime.month() as u16,

            // Signals
            c0: false,
            c1: false,
            c2: false,
            stb: false,
            cs: false,
            data_in: false,
            gnd: false,
            clk: false,
            data_out: false,
            tp: false,
            out_enable: false,
            n_xtal: false,
            xtal: false,
            vdd: false,

            // Internal
            mode: 0,
            bitno: 0,
            current_bit: 0,
            new_mode: false,
            new_clk: false,
            prev_mode: 0x10,
            prev_clk: false,
            flip_clk: false,
            tp_frequency: 1,

            previous_state: 0,
            previous_state_tp: 0,
        }
    }

    pub fn step(&mut self, timer_state: usize) -> bool {
        // Mode:
        // 0 - Register Hold DATA OUT = 1 Hz
        // 1 - Register Shift DATA OUT = [LSB] = 0 or 1
        // 2 - Time Set DATA OUT = [LSB] = 0 or 1
        // 3 - Time Read DATA OUT = 1 Hz

        if self.previous_state == 0 {
            self.previous_state = timer_state;
        }

        while (timer_state - self.previous_state) >= FREQUENCY {
            self.previous_state += FREQUENCY;
        }

        if self.stb {
            // Mode can change
            self.mode = (self.c0 as u8) + ((self.c1 as u8) << 1) + ((self.c2 as u8) << 2);
            if self.mode != self.prev_mode {
                self.new_mode = true;
                self.prev_mode = self.mode;
            } else {
                self.new_mode = false;
            }
        }

        if self.clk != self.prev_clk {
            self.flip_clk = true;
            self.prev_clk = self.clk;
        } else {
            self.flip_clk = false;
        }

        if self.mode == 4 {
            self.tp_frequency = 64;
        }
        if self.mode == 0 {
            self.clk = true;
            self.flip_clk = true;
            self.bitno = 0;
        }

        if self.clk && self.flip_clk {
            match self.mode {
                0x00 | 0x01 => {
                    // Start afresh with shifting or Load Register
                    match self.bitno {
                        0x00..=0x03 => {
                            // Seconds (1)
                            self.data_out = read_bit(self.seconds, self.bitno);
                        }
                        0x04..=0x07 => {
                            // Seconds (10)
                            self.data_out = read_bit(self.seconds, self.bitno);
                        }
                        0x08..=0x0B => {
                            // Minutes (1)
                            self.data_out = read_bit(self.minutes, self.bitno - 0x08);
                        }
                        0x0C..=0x0F => {
                            // Minutes (10)
                            self.data_out = read_bit(self.minutes, self.bitno - 0x08);
                        }
                        0x10..=0x13 => {
                            // Hours (1)
                            self.data_out = read_bit(self.hours, self.bitno - 0x10);
                        }
                        0x14..=0x17 => {
                            // Hours (10)
                            self.data_out = read_bit(self.hours, self.bitno - 0x10);
                        }
                        0x18..=0x1B => {
                            // Days (1)
                            self.data_out = read_bit(self.days, self.bitno - 0x18);
                        }
                        0x1C..=0x1F => {
                            // Days (10)
                            self.data_out = read_bit(self.days, self.bitno - 0x18);
                        }
                        0x20..=0x23 => {
                            // Weekday
                            self.data_out = read_bit(self.weekday, self.bitno - 0x20);
                        }
                        0x24..=0x27 => {
                            // Month
                            self.data_out = read_bit(self.month, self.bitno - 0x24);
                        }
                        _ => {}
                    }

                    self.bitno = self.bitno.wrapping_add(1);
                }
                0x02 => {
                    match self.bitno {
                        0x00..=0x07 => {
                            put_bit(&mut self.seconds, self.bitno, self.data_in);
                        }
                        0x08..=0x0F => {
                            put_bit(&mut self.minutes, self.bitno - 0x08, self.data_in);
                        }
                        0x10..=0x17 => {
                            put_bit(&mut self.hours, self.bitno - 0x10, self.data_in);
                        }
                        0x18..=0x1F => {
                            put_bit(&mut self.days, self.bitno - 0x18, self.data_in);
                        }
                        0x20..=0x23 => {
                            put_bit(&mut self.weekday, self.bitno - 0x20, self.data_in);
                        }
                        0x24..=0x27 => {
                            put_bit(&mut self.month, self.bitno - 0x24, self.data_in);
                        }
                        _ => {}
                    }
                    self.bitno = self.bitno.wrapping_add(1);
                }
                _ => {}
            }
        }

        true
    }

    pub fn get_data(&self) -> bool {
        self.data_out
    }

    pub fn get_tp(&mut self, timer_state: usize) -> bool {
        // generate tp signal. Used by tape functionality
        let tp_state = FREQUENCY / self.tp_frequency;

        if self.previous_state_tp == 0 {
            self.previous_state_tp = timer_state;
        }

        while (timer_state - self.previous_state_tp) >= (tp_state / 2) {
            self.tp = !self.tp;
            self.previous_state_tp += tp_state / 2;
        }

        self.tp
    }

    pub fn set_c0(&mut self, value: bool) {
        self.c0 = value;
    }

    pub fn set_c1(&mut self, value: bool) {
        self.c1 = value;
    }

    pub fn set_c2(&mut self, value: bool) {
        self.c2 = value;
    }

    pub fn set_stb(&mut self, value: bool) {
        self.stb = value;
    }

    pub fn set_cs(&mut self, value: bool) {
        self.cs = value;
    }

    pub fn set_data(&mut self, value: bool) {
        self.data_in = value;
    }

    pub fn set_gnd(&mut self, value: bool) {
        self.gnd = value;
    }

    pub fn set_clk(&mut self, value: bool) {
        self.clk = value;
    }

    pub fn set_out_enable(&mut self, value: bool) {
        self.out_enable = value;
    }

    pub fn set_n_xtal(&mut self, value: bool) {
        self.n_xtal = value;
    }

    pub fn set_xtal(&mut self, value: bool) {
        self.xtal = value;
    }

    pub fn set_vdd(&mut self, value: bool) {
        self.vdd = value;
    }
}
