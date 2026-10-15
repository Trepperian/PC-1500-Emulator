use chrono::{Datelike, Timelike};

pub const FREQUENCY: usize = 2600000 / 2;

/// The uPD1990AC clock: counters for seconds, minutes, hours and day (BCD),
/// weekday and month, and a 40-bit shift register between them and the
/// CPU. The ROM drives it as the data sheet says (`TIMER_MODE`, $E573, and
/// `WRITE_2_CLOCK`, $E52B): a command (C0-C2) is taken on a rising edge of
/// STB; in mode 1 (register shift) each rising edge of CLK shifts the
/// register one place, DATA OUT showing its lowest bit and DATA IN entering
/// at the top; mode 2 (time set) loads the counters from the register and
/// mode 3 (time read) the register from the counters. `TIME=x` is 40 bits
/// shifted in, then mode 2; reading `TIME`, mode 3, then 40 bits shifted
/// out. The counters advance once a second of emulated time.
///
/// Before, mode 1 read the counters directly and mode 2 wrote them bit by
/// bit as CLK went, so `TIME=x` changed nothing; and the counters never
/// advanced.
pub struct Pd1990ac {
    seconds: u8,
    minutes: u8,
    hours: u8,
    days: u8,
    weekday: u8,
    month: u8,
    /// The 40-bit shift register: seconds, minutes, hours, day (8 bits
    /// each), weekday and month (4 bits each), from the lowest bit.
    register: u64,

    // Signals
    c0: bool,
    c1: bool,
    c2: bool,
    stb: bool,
    cs: bool,
    data_in: bool,
    gnd: bool,
    clk: bool,
    tp: bool,
    out_enable: bool,
    n_xtal: bool,
    xtal: bool,
    vdd: bool,

    // Internal
    mode: u8,
    prev_stb: bool,
    prev_clk: bool,
    tp_frequency: usize,

    previous_state: usize,
    previous_state_tp: usize,
}

fn to_bcd(value: u32) -> u8 {
    ((value / 10) << 4 | value % 10) as u8
}

/// `value` (BCD) plus one; `limit` (BCD) wraps it to `first`. Whether it
/// wrapped.
fn bcd_increment(value: &mut u8, limit: u8, first: u8) -> bool {
    let mut next = *value + 1;
    if next & 0x0F == 0x0A {
        next += 6;
    }
    if next >= limit {
        *value = first;
        true
    } else {
        *value = next;
        false
    }
}

/// Days in `month` (1-12): the clock knows no years, so February has 28.
fn days_in_month(month: u8) -> u8 {
    const DAYS: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    DAYS[(month.clamp(1, 12) - 1) as usize]
}

impl Pd1990ac {
    pub fn new() -> Self {
        let now = std::time::SystemTime::now();
        let datetime: chrono::DateTime<chrono::Utc> = now.into();
        Self {
            seconds: to_bcd(datetime.second()),
            minutes: to_bcd(datetime.minute()),
            hours: to_bcd(datetime.hour()),
            days: to_bcd(datetime.day()),
            weekday: datetime.weekday().num_days_from_sunday() as u8,
            month: datetime.month() as u8,
            register: 0,

            // Signals
            c0: false,
            c1: false,
            c2: false,
            stb: false,
            cs: false,
            data_in: false,
            gnd: false,
            clk: false,
            tp: false,
            out_enable: false,
            n_xtal: false,
            xtal: false,
            vdd: false,

            // Internal
            mode: 0,
            prev_stb: false,
            prev_clk: false,
            tp_frequency: 1,

            previous_state: 0,
            previous_state_tp: 0,
        }
    }

    /// The counters as the shift register holds them.
    fn counters(&self) -> u64 {
        u64::from(self.seconds)
            | u64::from(self.minutes) << 8
            | u64::from(self.hours) << 16
            | u64::from(self.days) << 24
            | u64::from(self.weekday & 0x0F) << 32
            | u64::from(self.month & 0x0F) << 36
    }

    fn set_counters(&mut self, register: u64) {
        self.seconds = register as u8;
        self.minutes = (register >> 8) as u8;
        self.hours = (register >> 16) as u8;
        self.days = (register >> 24) as u8;
        self.weekday = (register >> 32) as u8 & 0x0F;
        self.month = (register >> 36) as u8 & 0x0F;
    }

    /// One second more.
    fn tick(&mut self) {
        if bcd_increment(&mut self.seconds, 0x60, 0)
            && bcd_increment(&mut self.minutes, 0x60, 0)
            && bcd_increment(&mut self.hours, 0x24, 0)
        {
            self.weekday = (self.weekday + 1) % 7;
            if bcd_increment(&mut self.days, to_bcd(u32::from(days_in_month(self.month))) + 1, 1) {
                self.month = self.month % 12 + 1;
            }
        }
    }

    pub fn step(&mut self, timer_state: usize) -> bool {
        if self.previous_state == 0 {
            self.previous_state = timer_state;
        }
        while (timer_state - self.previous_state) >= FREQUENCY {
            self.previous_state += FREQUENCY;
            self.tick();
        }

        // Mode:
        // 0 - Register Hold DATA OUT = 1 Hz
        // 1 - Register Shift DATA OUT = [LSB] = 0 or 1
        // 2 - Time Set DATA OUT = [LSB] = 0 or 1
        // 3 - Time Read DATA OUT = 1 Hz
        if self.stb && !self.prev_stb {
            self.mode = (self.c0 as u8) + ((self.c1 as u8) << 1) + ((self.c2 as u8) << 2);
            match self.mode {
                // The second starts again from the time set.
                2 => {
                    self.set_counters(self.register);
                    self.previous_state = timer_state;
                }
                3 => self.register = self.counters(),
                4 => self.tp_frequency = 64,
                _ => {}
            }
        }
        self.prev_stb = self.stb;

        if self.clk && !self.prev_clk && self.mode == 1 {
            self.register = self.register >> 1 | u64::from(self.data_in) << 39;
        }
        self.prev_clk = self.clk;

        true
    }

    pub fn get_data(&self) -> bool {
        self.register & 1 != 0
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

#[cfg(test)]
mod tests {
    use super::{FREQUENCY, Pd1990ac};

    /// Sends command `mode` (C0-C2) with a pulse of STB, as `TIMER_MODE`.
    fn command(clock: &mut Pd1990ac, mode: u8, at: usize) {
        clock.set_c0(mode & 1 != 0);
        clock.set_c1(mode & 2 != 0);
        clock.set_c2(mode & 4 != 0);
        for stb in [true, false] {
            clock.set_stb(stb);
            clock.step(at);
        }
    }

    /// Shifts the 40 bits of `bits` in, lowest first, and returns those
    /// shifted out, as `WRITE_2_CLOCK` does.
    fn exchange(clock: &mut Pd1990ac, bits: u64, at: usize) -> u64 {
        command(clock, 1, at);
        let mut out = 0;
        for bit in 0..40 {
            out |= u64::from(clock.get_data()) << bit;
            clock.set_data(bits >> bit & 1 != 0);
            for clk in [true, false] {
                clock.set_clk(clk);
                clock.step(at);
            }
        }
        out
    }

    /// `TIME=` sets the counters (40 bits shifted in, then "time set"),
    /// reading `TIME` gives them back ("time read", then 40 bits shifted
    /// out), and they advance with the seconds, across midnight and the
    /// end of the month: 31/12 23:59:58 is 1/1 00:00:18 20 seconds later.
    #[test]
    fn the_time_set_is_read_back_and_advances() {
        let mut clock = Pd1990ac::new();
        let start = 1000;
        let time = 0x58 | 0x59 << 8 | 0x23 << 16 | 0x31 << 24 | 3 << 32 | 12 << 36;
        exchange(&mut clock, time, start);
        command(&mut clock, 2, start);
        command(&mut clock, 3, start);
        assert_eq!(exchange(&mut clock, 0, start), time);

        let later = start + 20 * FREQUENCY;
        command(&mut clock, 3, later);
        let expected = 0x18 | 0x00 << 8 | 0x00 << 16 | 0x01 << 24 | 4 << 32 | 1 << 36;
        assert_eq!(exchange(&mut clock, 0, later), expected);
    }
}
