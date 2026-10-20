//! The PC-1500's NEC `µPD1990AC` clock/calendar (the one the BASIC statement `TIME` reads and
//! writes, and the one that gives the time base of `WAIT`, `PAUSE` and `BEEP`).
//!
//! # What the chip is
//! The `µPD1990AC` is a clock powered by the backup battery, with a 40-bit internal counter in BCD
//! (except the month, which is hexadecimal) and a 40-bit shift register through which those
//! counters are read and written serially. Order of the 40 bits, from the least significant (the
//! first one out of `DATA OUT`) to the most:
//!
//! ```text
//! bits  0-7 : seconds (BCD, 00-59)
//! bits  8-15: minutes (BCD, 00-59)
//! bits 16-23: hours   (BCD, 00-23)
//! bits 24-31: day     (BCD, 01-31)
//! bits 32-35: day of the week (0-6, Sunday = 0)
//! bits 36-39: month   (hexadecimal, 1-12)
//! ```
//!
//! It is exactly the order in which the ROM keeps the 5 bytes in `ARX`, from $7A06 (seconds) down
//! to $7A02 (`month<<4 | weekday`).
//!
//! # Wiring in the PC-1500
//! Everything goes through the LH5810's port C (`OPC`, $F008), as in `PockEmul` (`pc1500.cpp`,
//! lines 722-756):
//!
//! ```text
//! PC0 = DATA IN    PC1 = STB    PC2 = CLK
//! PC3 = C0 and also OUT ENBL    PC4 = C1    PC5 = C2
//! ```
//!
//! and the two outputs come back through port B: `TP` on PB5 and `DATA OUT` on PB6 (PB7 is the
//! ON/BRK key, see `Keyboard::on_pulse`).
//!
//! PC3 being both `C0` and `OUT ENBL` is not an oversight: the ROM counts on it. `ARX_2_TIME_1`
//! ($E5A9-$E5AE, `ANI #(Y),$C7` / `ORI #(Y),$08`) raises PC3 after command 0 only to enable the
//! output, and the commands that read data (1 = shift, $08; 3 = read the time, $18) have `C0 = 1`,
//! that is, the output enabled.
//!
//! # Commands
//! On a rising edge of `STB` the chip latches the command `C2C1C0`. There are two independent
//! groups (NEC's data sheet): those with `C2 = 0` control the register and the counters, those with
//! `C2 = 1` only choose the frequency of `TP`, and each group keeps its last command without the
//! other touching it.
//!
//! - 0 `Register Hold`: does nothing with the register; `DATA OUT` = 1 Hz.
//! - 1 `Register Shift`: on each rising edge of `CLK` the register shifts one bit right, `DATA IN`
//!   comes in at bit 39 and `DATA OUT` is bit 0.
//! - 2 `Time Set & Counter Hold`: copies the register to the counters and stops them until the next
//!   command of group 0.
//! - 3 `Time Read`: copies the counters to the register; `DATA OUT` = 0.5 Hz.
//! - 4/5/6: `TP` = 64/256/2048 Hz. 7: test mode (only noted; the ROM never uses it).
//!
//! # How the A04 ROM uses it
//! - `TIME` (reading, `TIME2ARX` $E5B4): command 3, and then `WRITE_2_CLOCK` ($E52B): command 1 and
//!   40 pulses of `CLK`, reading PB6 BEFORE each pulse. Then `BCMD_TIME` ($DE82) turns it into the
//!   number `MMDDHH.MMSS`.
//! - `TIME=` (`ARX_2_VAR_5`..`13`, $DE1D-$DE56, and `ARX_2_TIME` $E59A): checks the number (month >
//!   12 → `ERROR 23`), puts in the 40 bits with command 1 (the day of the week stays at 0), and
//!   then command 2 (Time Set) and command 0.
//! - `WAIT`, `PAUSE`, `BEEP` (`TIME_DELAY`, $E88C): command 4 (`TP` at 64 Hz), and they count
//!   rising edges of PB5, 15.625 ms each.
//! - `RANDOM` ($F648) reads the time to get the seed.
//!
//! # Time bases
//! See [`RtcTimeBase`]: the GUI uses the host's local time (the clock follows the real time, at any
//! emulation speed, as the real chip with its battery), the tests a fixed time that moves on with
//! the emulated cycles, so that they are deterministic. `TP` and `DATA OUT` always go in EMULATED
//! time, so `WAIT` and `BEEP` speed up and slow down with the emulator's speed as the CPU does.

use core::fmt;
use std::time::{Duration, Instant};

use chrono::{Datelike, Timelike};

/// Machine cycles per second of the LH5801 (the 2.6 MHz oscillator divided by 2). It is the unit of
/// `timer_state`/`emu_cycles`: the emulated clock and `TP` count in these cycles. `lh5810.rs` uses
/// it too.
pub const FREQUENCY: usize = 2_600_000 / 2;

/// [`FREQUENCY`] as a `u64`, for the internal arithmetic.
const FREQ: u64 = FREQUENCY as u64;

const NS_PER_SEC: u64 = 1_000_000_000;

const SECONDS_PER_DAY: u64 = 86_400;

/// Mask of the 40 bits of the shift register.
const SHIFT_MASK: u64 = (1 << 40) - 1;

/// Frequency of `TP` after switching on (command 4, the one the ROM uses).
const TP_DEFAULT_HZ: u32 = 64;

/// Date and time of the `µPD1990AC`, in binary (not BCD).
///
/// The chip has no year. The values are not checked: [`Pd1990ac`] keeps whatever is written to it
/// (`TIME=` accepts e.g. month 0), as the real chip does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RtcDateTime {
    /// Month, 1-12 (`TIME=` also accepts 0).
    pub month: u8,
    /// Day of the month, 1-31.
    pub day: u8,
    /// Hora, 0-23.
    pub hour: u8,
    /// Minuto, 0-59.
    pub minute: u8,
    /// Segundo, 0-59.
    pub second: u8,
    /// Day of the week, 0-6 with Sunday = 0 (the data sheet's example: "July 16 (Sun.)" with a 0).
    /// The ROM sets it to 0 at each `TIME=`.
    pub weekday: u8,
}

impl fmt::Display for RtcDateTime {
    /// Day/month and time format: `28/02 23:59:50`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02}/{:02} {:02}:{:02}:{:02}",
            self.day, self.month, self.hour, self.minute, self.second
        )
    }
}

/// Where the `µPD1990AC` gets its initial time from and at what pace it counts the seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RtcTimeBase {
    /// The default (the GUI): it starts at the host's LOCAL time (`chrono::Local`, not UTC) and the
    /// seconds move on with real time (`std::time::Instant`), whatever the emulation speed or the
    /// pauses, as the real chip with its battery. The host's time zone offset is taken only once,
    /// when it is created (it does not follow later daylight saving changes).
    HostLocal,
    /// For the tests: it starts at the given time and the seconds move on with the LH5801's
    /// emulated cycles ([`FREQUENCY`] per second), so everything is deterministic.
    Emulated(RtcDateTime),
}

/// Timeline with which it counts the seconds (see [`RtcTimeBase`]).
#[derive(Clone, Copy, Debug)]
enum Base {
    /// Real time. The `Instant` is the start of the host's current second when the chip is created
    /// (set back by the nanoseconds of that second already gone), so that the chip's second changes
    /// match the host clock's.
    Host(Instant),
    /// Emulated cycles.
    Emulated,
}

/// Binario (0-99) a BCD.
const fn to_bcd(v: u8) -> u8 {
    ((v / 10) << 4) | (v % 10)
}

/// BCD to binary. It does not check: an invalid BCD (e.g. $5A) gives an out-of-range value, as the
/// one the ROM would read.
const fn from_bcd(v: u8) -> u8 {
    (v >> 4) * 10 + (v & 0x0F)
}

/// BCD increment of a counter that goes back to 0 when it reaches `limit` (in BCD: $60 for seconds
/// and minutes, $24 for hours). It returns `true` if it went back to 0 (carry to the next counter).
const fn bcd_inc(v: u8, limit: u8) -> (u8, bool) {
    let mut n = v.wrapping_add(1);
    if n & 0x0F >= 10 {
        n = (n & 0xF0).wrapping_add(0x10);
    }
    if n >= limit { (0, true) } else { (n, false) }
}

/// `true` if `v` is a valid BCD below `limit`.
const fn bcd_below(v: u8, limit: u8) -> bool {
    v & 0x0F < 10 && v < limit
}

/// Converts a `chrono` field (always small: month, day, hour...) to `u8`.
fn small(v: u32) -> u8 {
    u8::try_from(v).unwrap_or(u8::MAX)
}

pub struct Pd1990ac {
    base: Base,
    /// Counters, in the same order as the 40-bit register:
    /// `[seconds BCD, minutes BCD, hours BCD, day BCD, month<<4 | day of the week]`.
    counter: [u8; 5],
    /// 40-bit shift register (the top 24 bits at 0).
    shift_reg: u64,
    /// `false` while command 2 (Time Set & Counter Hold) keeps the counters stopped.
    counting: bool,
    /// Start of the current second, in nanoseconds of the timeline of `base` (see
    /// [`Pd1990ac::now_ns`]).
    sec_origin_ns: u64,
    /// Last command of group 0 (0-3).
    mode: u8,
    /// Frequency of `TP` in Hz (last command of group 1).
    tp_freq: u32,
    /// Command 7 (test mode). Only noted.
    test_mode: bool,
    /// `emu_cycles` when the current frequency of `TP` was chosen: the wave's phase origin.
    tp_origin_cycles: u64,
    /// Emulated cycles since the chip was created. Monotonic: it survives the CPU's resets (see
    /// [`Pd1990ac::sync`]).
    emu_cycles: u64,
    /// Last `timer_state` of the LH5801 seen by [`Pd1990ac::sync`].
    last_timer_state: usize,
    /// Last levels of the input pins.
    pins: Pins,
}

/// Last levels of the `µPD1990AC`'s input pins that are needed to detect edges (`STB`, `CLK`) or
/// that decide an output (`OUT ENBL`). `DATA IN` is not kept: it only counts on the edge of `CLK`,
/// and it is read from the same byte of `OPC` (see [`Pd1990ac::write_port`]).
#[derive(Clone, Copy, Debug, Default)]
struct Pins {
    stb: bool,
    clk: bool,
    out_enable: bool,
}

impl Pd1990ac {
    /// Chip with the host's local time ([`RtcTimeBase::HostLocal`]).
    pub fn new() -> Self {
        Self::with_time_base(RtcTimeBase::HostLocal)
    }

    pub fn with_time_base(time_base: RtcTimeBase) -> Self {
        let (base, start) = match time_base {
            RtcTimeBase::HostLocal => {
                let now = chrono::Local::now();
                // `nanosecond()` goes past 1e9 during a leap second.
                let into_second = Duration::from_nanos(u64::from(now.nanosecond()) % NS_PER_SEC);
                let epoch = Instant::now().checked_sub(into_second).unwrap_or_else(Instant::now);
                let start = RtcDateTime {
                    month: small(now.month()),
                    day: small(now.day()),
                    hour: small(now.hour()),
                    minute: small(now.minute()),
                    second: small(now.second().min(59)),
                    weekday: small(now.weekday().num_days_from_sunday()),
                };
                (Base::Host(epoch), start)
            }
            RtcTimeBase::Emulated(start) => (Base::Emulated, start),
        };
        Self {
            base,
            counter: [
                to_bcd(start.second),
                to_bcd(start.minute),
                to_bcd(start.hour),
                to_bcd(start.day),
                (start.month << 4) | (start.weekday & 0x0F),
            ],
            shift_reg: 0,
            counting: true,
            // The current second started at instant 0 of the timeline (see `Base::Host`).
            sec_origin_ns: 0,
            mode: 0,
            tp_freq: TP_DEFAULT_HZ,
            test_mode: false,
            tp_origin_cycles: 0,
            emu_cycles: 0,
            last_timer_state: 0,
            pins: Pins::default(),
        }
    }

    /// Moves the emulated clock on to `timer_state` (the LH5801's cycle counter). It is called
    /// after each instruction.
    ///
    /// If `timer_state` is LOWER than the last one seen, the CPU is new (a reset of
    /// [`crate::Pc1500::reset`] that keeps the chip) and started from 0: the whole `timer_state` is
    /// counted. It does not normally happen, because the ROM's reset no longer sets `timer_state`
    /// to 0 (see `cpu_internal_reset` in `lh5801.rs`) and `Pc1500::reset` calls
    /// [`Pd1990ac::rebase_timer`]; it is only a safety net so that the clock never jumps back nor
    /// the subtraction overflows.
    pub fn sync(&mut self, timer_state: usize) {
        let delta = timer_state
            .checked_sub(self.last_timer_state)
            .unwrap_or(timer_state);
        self.emu_cycles += delta as u64;
        self.last_timer_state = timer_state;
    }

    /// Makes the next [`Pd1990ac::sync`] count from `timer_state` without adding anything: for when
    /// the chip moves to another CPU (reset) or is fitted in a machine that had already run cycles.
    pub const fn rebase_timer(&mut self, timer_state: usize) {
        self.last_timer_state = timer_state;
    }

    /// Emulated time in nanoseconds.
    fn emu_ns(&self) -> u64 {
        let ns = u128::from(self.emu_cycles) * u128::from(NS_PER_SEC) / u128::from(FREQ);
        u64::try_from(ns).unwrap_or(u64::MAX)
    }

    /// "Now" on the timeline of the active base, in nanoseconds. With [`Base::Host`] it calls
    /// `Instant::now()`: it is only used by the Time Read / Time Set commands and by
    /// [`Pd1990ac::now`], never per instruction.
    fn now_ns(&self) -> u64 {
        match self.base {
            Base::Host(epoch) => u64::try_from(epoch.elapsed().as_nanos()).unwrap_or(u64::MAX),
            Base::Emulated => self.emu_ns(),
        }
    }

    /// Adds to the counters the whole seconds gone since `sec_origin_ns`. The counters are not
    /// updated every second, but only when someone looks at them (Time Read or [`Pd1990ac::now`]):
    /// the result is the same and it costs nothing per instruction.
    fn catch_up(&mut self) {
        if !self.counting {
            return;
        }
        let elapsed = self.now_ns().saturating_sub(self.sec_origin_ns);
        let seconds = elapsed / NS_PER_SEC;
        self.sec_origin_ns += seconds * NS_PER_SEC;
        self.advance_seconds(seconds);
    }

    /// Moves on `seconds` seconds. Whole days are skipped at once with [`Pd1990ac::increment_day`]
    /// (if the time is valid BCD, adding 86400 seconds leaves the same time and one more day), and
    /// the rest second by second.
    fn advance_seconds(&mut self, mut seconds: u64) {
        let [sec, min, hour, ..] = self.counter;
        if seconds >= SECONDS_PER_DAY
            && bcd_below(sec, 0x60)
            && bcd_below(min, 0x60)
            && bcd_below(hour, 0x24)
        {
            for _ in 0..seconds / SECONDS_PER_DAY {
                self.increment_day();
            }
            seconds %= SECONDS_PER_DAY;
        }
        for _ in 0..seconds {
            self.tick_second();
        }
    }

    /// One more second, with carry seconds → minutes → hours → day.
    fn tick_second(&mut self) {
        let limits = [0x60, 0x60, 0x24];
        for (counter, limit) in self.counter.iter_mut().zip(limits) {
            let (next, carry) = bcd_inc(*counter, limit);
            *counter = next;
            if !carry {
                return;
            }
        }
        self.increment_day();
    }

    /// One more day: day of the week (6 → 0) and day of the month, moving to the next month (12 →
    /// 1).
    ///
    /// Length of the months according to the data sheet: 30 days April, June, September and
    /// November; 28 February; 31 the rest (also the invalid months 0 and 13-15 that can be
    /// written). No leap years: "leap year is not automatically performed... possible to set
    /// FEB.29, and the day after, it will be Mar.1".
    const fn increment_day(&mut self) {
        let month = self.counter[4] >> 4;
        let weekday = self.counter[4] & 0x0F;
        let weekday = if weekday >= 6 { 0 } else { weekday + 1 };
        let last_day = match month {
            4 | 6 | 9 | 11 => 0x30,
            2 => 0x28,
            _ => 0x31,
        };
        // The limit is set by the month, not by the counter: $FF only keeps `bcd_inc` from setting
        // it to 0.
        let (mut day, _) = bcd_inc(self.counter[3], 0xFF);
        let mut month = month;
        if day > last_day {
            day = 0x01;
            month = if month >= 12 { 1 } else { month + 1 };
        }
        self.counter[3] = day;
        self.counter[4] = (month << 4) | weekday;
    }

    /// New value of the LH5810's `OPC` register (see the wiring in the module's documentation).
    ///
    /// Calling it once per instruction is enough: all the ROM's sequences change a single pin per
    /// instruction, and besides they write each level twice (`STA #(Y)` twice, `ORI`/`ANI` pairs).
    pub fn write_port(&mut self, opc: u8) {
        let data_in = opc & 0x01 != 0;
        let stb = opc & 0x02 != 0;
        let clk = opc & 0x04 != 0;
        self.pins.out_enable = opc & 0x08 != 0;

        // Commands are latched on the RISING edge of STB (the level used to be looked at).
        if stb && !self.pins.stb {
            self.strobe((opc >> 3) & 0x07);
        }
        self.pins.stb = stb;

        // Shift on the rising edge of CLK, sampling DATA IN on that same edge (the data sheet's
        // timing diagram, page 7). DATA OUT becomes the new bit 0.
        if clk && !self.pins.clk && self.mode == 1 {
            self.shift_reg = ((self.shift_reg >> 1) | (u64::from(data_in) << 39)) & SHIFT_MASK;
        }
        self.pins.clk = clk;
    }

    /// Runs the command `C2C1C0` latched by an edge of `STB`.
    fn strobe(&mut self, command: u8) {
        if command < 4 {
            // Group 0. Leaving Time Set releases the counters with a second just started: command 2
            // clears stages 11-15 of the divider.
            if !self.counting && command != 2 {
                self.counting = true;
                self.sec_origin_ns = self.now_ns();
            }
            match command {
                // Register Hold. It also leaves the test mode.
                0 if self.test_mode => {
                    self.test_mode = false;
                    self.set_tp(TP_DEFAULT_HZ);
                }
                2 => {
                    // Time Set & Counter Hold.
                    let bytes = self.shift_reg.to_le_bytes();
                    self.counter.copy_from_slice(&bytes[..5]);
                    self.counting = false;
                }
                3 => {
                    // Time Read.
                    self.catch_up();
                    let mut bytes = [0; 8];
                    bytes[..5].copy_from_slice(&self.counter);
                    self.shift_reg = u64::from_le_bytes(bytes);
                }
                // 0 without test mode, and 1 (Register Shift): nothing to do when latched.
                _ => {}
            }
            self.mode = command;
        } else {
            // Group 1: only TP; the command of group 0 stays in force.
            match command {
                4 => self.set_tp(64),
                5 => self.set_tp(256),
                6 => self.set_tp(2048),
                _ => self.test_mode = true,
            }
        }
    }

    /// Changes the frequency of `TP`. The wave's phase is restarted only if the frequency changes.
    const fn set_tp(&mut self, hz: u32) {
        if hz != self.tp_freq {
            self.tp_freq = hz;
            self.tp_origin_cycles = self.emu_cycles;
        }
    }

    /// Level of the `TP` output (PB5): a 50% square wave of the chosen frequency, in EMULATED time,
    /// with no drift.
    ///
    /// In [`FREQUENCY`] cycles (1 s) there are exactly `tp_freq` periods, so taking the time modulo
    /// 1 s does not change the phase and avoids any overflow.
    pub fn tp(&self) -> bool {
        let elapsed = (self.emu_cycles - self.tp_origin_cycles) % FREQ;
        (elapsed * 2 * u64::from(self.tp_freq) / FREQ) & 1 == 1
    }

    /// Level of the `DATA OUT` output (PB6).
    ///
    /// - With `OUT ENBL` (PC3) at 0 the output is in high impedance and reads 1 (pull-up resistor;
    ///   MAME, `upd1990a.cpp`, does the same). Not checked on the PC-1500's schematic, but the ROM
    ///   never reads it in that state.
    /// - Commands 1 and 2: bit 0 of the register.
    /// - Command 0: a 1 Hz wave; command 3: 0.5 Hz. Taken from the emulated time, not from the
    ///   chip's 1 s divider (the phase may not be the real one). The A04 ROM never samples them:
    ///   PB6 is only read at $E544, with command 1.
    pub const fn data_out(&self) -> bool {
        if !self.pins.out_enable {
            return true;
        }
        match self.mode {
            1 | 2 => self.shift_reg & 1 != 0,
            0 => (self.emu_cycles / (FREQ / 2)) & 1 == 0,
            _ => (self.emu_cycles / FREQ) & 1 == 0,
        }
    }

    /// Current date and time of the counters (for the GUI and the tests).
    pub fn now(&mut self) -> RtcDateTime {
        self.catch_up();
        let [second, minute, hour, day, month_weekday] = self.counter;
        RtcDateTime {
            month: month_weekday >> 4,
            day: from_bcd(day),
            hour: from_bcd(hour),
            minute: from_bcd(minute),
            second: from_bcd(second),
            weekday: month_weekday & 0x0F,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bits 6-7 of `OPC` as the ROM's reset I/O table leaves them ($E169: $C0). The chip ignores
    /// them; they are included to write exactly the same bytes as the ROM.
    const OPC_HIGH: u8 = 0xC0;

    const NEW_YEAR_EVE: RtcDateTime = RtcDateTime {
        month: 12,
        day: 31,
        hour: 23,
        minute: 59,
        second: 58,
        weekday: 6,
    };

    fn emulated(start: RtcDateTime) -> Pd1990ac {
        Pd1990ac::with_time_base(RtcTimeBase::Emulated(start))
    }

    /// Copy of `TIMER_MODE` ($E573, `VMJ ($5A)`): `OPC = (OPC & $C0) + A` twice, `+2` (STB at 1)
    /// twice and `-2` (STB at 0) twice.
    fn rom_mode(rtc: &mut Pd1990ac, a: u8) {
        let opc = OPC_HIGH | a;
        for value in [opc, opc, opc + 2, opc + 2, opc, opc] {
            rtc.write_port(value);
        }
    }

    /// Copy of `WRITE_2_CLOCK` ($E52B): command 1 and, for `ARX` from $7A06 to $7A02, 8 bits per
    /// byte: it samples `DATA OUT` (PB6) BEFORE the pulse, `ROR` (the sample comes in at bit 7 and
    /// the outgoing bit goes to DATA IN, so the LSB goes out first), DATA IN twice, CLK at 1 twice
    /// and CLK at 0 once. It reads and writes at the same time, as the ROM.
    fn rom_write2clock(rtc: &mut Pd1990ac, arx: &mut [u8; 8]) {
        rom_mode(rtc, 0x08);
        let mut opc = OPC_HIGH | 0x08;
        for x in (2..=6).rev() {
            let mut a = arx[x];
            for _ in 0..8 {
                let sample = rtc.data_out();
                let out = a & 1 != 0;
                a = (a >> 1) | (u8::from(sample) << 7);
                opc = if out { opc | 0x01 } else { opc & !0x01 };
                rtc.write_port(opc);
                rtc.write_port(opc);
                opc |= 0x04;
                rtc.write_port(opc);
                rtc.write_port(opc);
                opc &= !0x04;
                rtc.write_port(opc);
            }
            arx[x] = a;
        }
    }

    /// `TIME`: `TIME2ARX` ($E5B4) = command 3 + `WRITE_2_CLOCK`, and then command 0. It returns
    /// `ARX[2..7]`.
    fn rom_read(rtc: &mut Pd1990ac) -> [u8; 5] {
        let mut arx = [0; 8];
        rom_mode(rtc, 0x18);
        rom_write2clock(rtc, &mut arx);
        rom_mode(rtc, 0x00);
        let mut out = [0; 5];
        out.copy_from_slice(&arx[2..7]);
        out
    }

    /// `TIME=`: `ARX_2_TIME` ($E59A) = `WRITE_2_CLOCK` with the 5 bytes, command 2, command 0 and
    /// PC3 at 1.
    fn rom_set(rtc: &mut Pd1990ac, bytes: [u8; 5]) {
        let mut arx = [0; 8];
        arx[2..7].copy_from_slice(&bytes);
        rom_write2clock(rtc, &mut arx);
        rom_mode(rtc, 0x10);
        rom_mode(rtc, 0x00);
        rtc.write_port(OPC_HIGH | 0x08);
    }

    /// Moves the emulated clock on `cycles` cycles.
    fn advance(rtc: &mut Pd1990ac, cycles: u64) {
        let target = rtc.last_timer_state + usize::try_from(cycles).unwrap_or(usize::MAX);
        rtc.sync(target);
    }

    #[test]
    fn read_shifts_counters_lsb_first() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        assert_eq!(rom_read(&mut rtc), [0xC6, 0x31, 0x23, 0x59, 0x58], "reading");
    }

    #[test]
    fn year_rollover() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        advance(&mut rtc, 3 * FREQ);
        // January 1, 00:00:01, Sunday (6 → 0).
        assert_eq!(rom_read(&mut rtc), [0x10, 0x01, 0x00, 0x00, 0x01], "rollover");
        assert_eq!(
            rtc.now(),
            RtcDateTime { month: 1, day: 1, hour: 0, minute: 0, second: 1, weekday: 0 },
            "now()"
        );
    }

    #[test]
    fn set_then_read() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        advance(&mut rtc, FREQ / 3);
        // TIME=101912.3045: ARX_2_VAR_12 leaves $7A02 = $A0 (month 10 in hexadecimal, day of the
        // week 0).
        rom_set(&mut rtc, [0xA0, 0x19, 0x12, 0x30, 0x45]);
        advance(&mut rtc, 2 * FREQ + 100);
        assert_eq!(rom_read(&mut rtc), [0xA0, 0x19, 0x12, 0x30, 0x47], "after TIME=");
    }

    #[test]
    fn time_set_holds_counters() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        let mut arx = [0, 0, 0xA0, 0x19, 0x12, 0x30, 0x45, 0];
        rom_write2clock(&mut rtc, &mut arx);
        rom_mode(&mut rtc, 0x10);
        advance(&mut rtc, 5 * FREQ);
        assert_eq!(rtc.now().second, 45, "stopped with command 2");
        // Leaving Time Set starts a new second at that instant.
        rom_mode(&mut rtc, 0x00);
        advance(&mut rtc, FREQ - 10);
        assert_eq!(rtc.now().second, 45, "1 s has not gone by yet");
        advance(&mut rtc, 10);
        assert_eq!(rtc.now().second, 46, "1 s after releasing it");
    }

    /// Sets the date `month`/`day` at 23:59:59 and moves on 1 s.
    fn next_day(month: u8, day: u8) -> (u8, u8) {
        let mut rtc = emulated(RtcDateTime { month, day, hour: 23, minute: 59, second: 59, weekday: 0 });
        advance(&mut rtc, FREQ);
        let now = rtc.now();
        (now.month, now.day)
    }

    #[test]
    fn month_lengths_without_leap_years() {
        assert_eq!(next_day(2, 28), (3, 1), "Feb 28");
        assert_eq!(next_day(2, 29), (3, 1), "Feb 29 set by hand");
        assert_eq!(next_day(4, 30), (5, 1), "Apr 30");
        assert_eq!(next_day(7, 30), (7, 31), "Jul 30");
        assert_eq!(next_day(7, 31), (8, 1), "Jul 31");
        assert_eq!(next_day(9, 9), (9, 10), "BCD carry of the day");
        assert_eq!(next_day(0, 31), (1, 1), "month 0 (TIME= accepts it)");
    }

    #[test]
    fn long_gaps_skip_whole_days() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        // 3 s + 400 days: Dec 31 23:59:58 → Jan 1 00:00:01 (Sunday) → 365 days later Jan 1 again
        // (the chip has no years, so no leap years either) → 35 more days, Feb 5.
        advance(&mut rtc, (400 * SECONDS_PER_DAY + 3) * FREQ);
        let now = rtc.now();
        assert_eq!((now.month, now.day, now.hour, now.minute, now.second), (2, 5, 0, 0, 1), "date");
        // 400 days from a Sunday: 400 % 7 = 1 → Monday.
        assert_eq!(now.weekday, 1, "day of the week");
    }

    /// Rising edges of `TP` in 1 emulated s.
    fn tp_edges_in_one_second(rtc: &mut Pd1990ac) -> u32 {
        let mut edges = 0;
        let mut prev = rtc.tp();
        for _ in 0..FREQ / 13 {
            advance(rtc, 13);
            let level = rtc.tp();
            if level && !prev {
                edges += 1;
            }
            prev = level;
        }
        edges
    }

    #[test]
    fn tp_frequencies() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        rom_mode(&mut rtc, 0x20);
        assert_eq!(tp_edges_in_one_second(&mut rtc), 64, "command 4");
        rom_mode(&mut rtc, 0x28);
        assert_eq!(tp_edges_in_one_second(&mut rtc), 256, "command 5");
        rom_mode(&mut rtc, 0x30);
        assert_eq!(tp_edges_in_one_second(&mut rtc), 2048, "command 6");
        rom_mode(&mut rtc, 0x00);
        assert_eq!(tp_edges_in_one_second(&mut rtc), 2048, "group 0 does not touch TP");
    }

    #[test]
    fn tp_command_keeps_group_0_mode() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        // Register = counters (seconds $58 = %0101_1000), command 1.
        rom_mode(&mut rtc, 0x18);
        rom_mode(&mut rtc, 0x08);
        // Command 5 (TP 256 Hz; with C0 = 1, output enabled): the mode is still Register Shift.
        rom_mode(&mut rtc, 0x28);
        assert!(!rtc.data_out(), "bit 0");
        for _ in 0..3 {
            rtc.write_port(OPC_HIGH | 0x28 | 0x04);
            rtc.write_port(OPC_HIGH | 0x28);
        }
        assert!(rtc.data_out(), "bit 3 after 3 pulses of CLK");
    }

    #[test]
    fn data_out_is_high_when_output_disabled() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        // Register at 0 with 40 zeros through DATA IN.
        rom_write2clock(&mut rtc, &mut [0; 8]);
        assert!(!rtc.data_out(), "OUT ENBL = 1: bit 0 = 0");
        // Command 2 (C0 = 0): DATA OUT in high impedance.
        rom_mode(&mut rtc, 0x10);
        assert!(rtc.data_out(), "OUT ENBL = 0");
    }

    #[test]
    fn cpu_reset_does_not_rewind_the_clock() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        rtc.sync(FREQUENCY);
        assert_eq!(rtc.now().second, 59, "1 s");
        // New CPU with timer_state from 0, without `rebase_timer`.
        rtc.sync(FREQUENCY / 2);
        rtc.sync(FREQUENCY);
        assert_eq!(rtc.now().second, 0, "1.5 s + 0.5 s more");
    }

    #[test]
    fn host_local_follows_local_time() {
        // Retry in case the time changes just between the two reads.
        let matches = (0..2).any(|_| {
            let mut rtc = Pd1990ac::new();
            let now = rtc.now();
            let local = chrono::Local::now();
            (now.hour, now.day, now.month) == (small(local.hour()), small(local.day()), small(local.month()))
        });
        assert!(matches, "the chip's time is the host's LOCAL time");
    }

    #[test]
    fn display_format() {
        let dt = RtcDateTime { month: 2, day: 8, hour: 7, minute: 5, second: 3, weekday: 0 };
        assert_eq!(dt.to_string(), "08/02 07:05:03", "format");
    }
}
