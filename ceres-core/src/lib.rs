pub mod basic;
pub mod display;
pub mod harness;
pub mod host_io;
mod key_feeder;
pub mod keyboard;
pub mod lh5_loader;
mod lh5801;
mod lh5810;
mod memory;
mod pd1990ac;

use core::mem;
use std::time::Duration;
use std::path::Path;

use display::DisplayController;
use host_io::HostIo;
pub use host_io::{HostCommand, HostEvent};
pub use key_feeder::{HostKeyId, ON_MIN_HOLD_FRAMES, ShiftPolicy, TYPED_IDLE_TIMEOUT_FRAMES};
pub use keyboard::{Key, KeyStroke};
use key_feeder::KeyFeeder;
use keyboard::Keyboard;
pub use lh5801::Lh5801;
use memory::MemoryBus;

use crate::{lh5810::Lh5810, pd1990ac::Pd1990ac};
pub use pd1990ac::{RtcDateTime, RtcTimeBase};

/// Real nanoseconds per LH5801 machine cycle, from the PC-1500's real
/// oscillator (2.6MHz), which the CPU divides by 2 inside: ~1.3MHz. This
/// constant used to exist with the wrong units
/// (`Duration::from_nanos(2_600_000 / 2)` = 1.3ms, not ~769ns, a factor of
/// ~1690) and, worse, nothing in the repository used it: `step_frame()`
/// (below) is only a budget of CPU cycles, with no idea of real time. See
/// `FRAME_DURATION`, which does use it.
const NANOS_PER_TICK: u64 = 1_000_000_000 / (2_600_000 / 2);

/// Clock frequency of the PC-1500's LH5801 CPU, in cycles per second.
///
/// These are machine cycles ("ticks", [`Pc1500::ticks`]) per second of real hardware time: the 2.6
/// MHz oscillator divided by 2 inside the CPU itself (the same as `setfrequency(2600000/2)` in
/// `PockEmul`). It is the same constant the emulated `µPD1990AC` clock counts time with.
///
/// It is what any real-time scheduler (the GUI) needs: `seconds × CPU_HZ × multiplier` ticks to run
/// with [`Pc1500::run_for_ticks`]. Better this than [`FRAME_DURATION`]: computing `NANOS_PER_TICK`
/// with integers, `1e9 / 1.3e6` is truncated to 769 ns and `FRAME_DURATION` comes out 0.03% short.
pub const CPU_HZ: usize = pd1990ac::FREQUENCY;

/// CPU cycles that a call to [`Pc1500::step_frame`] runs.
///
/// They are ~11.5 ms of real hardware time at [`CPU_HZ`]. It is the time unit of the test harness
/// ("frames") and the period of the per-frame hooks of [`Pc1500::run_for_ticks`].
pub const TICKS_PER_FRAME: usize = 15000;

/// The real time (the PC-1500's own clock, not the host's) that one call
/// to [`Pc1500::step_frame`] stands for: `TICKS_PER_FRAME` real LH5801
/// machine cycles at ~1.3MHz.
///
/// `step_frame()` itself knows nothing of real time: it is a loop that
/// runs until it has spent exactly `TICKS_PER_FRAME` cycles, without
/// looking at the clock. Before this constant, the GUI
/// (`ceres-egui::Pc1500App`) called `step_frame()` once per repaint, and
/// the repaints were not paced to real time either (only to `vsync`), so
/// the emulation's speed was tied to the user's monitor refresh rate, not
/// to the real PC-1500. This constant is what any caller needs to pace its
/// calls to `step_frame()` to real time.
///
/// It is kept for compatibility: the GUI no longer paces whole frames but
/// single ticks, with [`Pc1500::run_for_ticks`] and [`CPU_HZ`] (without the
/// truncation of `NANOS_PER_TICK`), see `ceres-egui/src/pacing.rs`.
pub const FRAME_DURATION: Duration = Duration::from_nanos(TICKS_PER_FRAME as u64 * NANOS_PER_TICK);

/// Persistence of the LCD, in CPU cycles.
///
/// The panel still shows lit for this emulated time after the CPU turns the `DISP` signal off (see
/// [`Pc1500::display`]). One frame, ~11.5 ms: more than enough to cover the `RDP`...`SDP` pulse of
/// ~1.6 ms (2090 cycles) of the idle keyboard loop, and well under what a real LCD takes to react,
/// so a program that really turns the LCD off still shows it off.
pub const LCD_PERSISTENCE_TICKS: usize = TICKS_PER_FRAME;

/// Start of the ROM routine `WAIT_4_KB` ($E243): waiting for a key with a blinking cursor and
/// automatic power-off.
pub(crate) const WAIT_4_KB_START: u16 = 0xE243;

/// End (exclusive) of the block `WAIT_4_KB`..`AUTO_OFF` ($E33F). While the CPU is halted in `HLT`
/// with the PC inside this range, the ROM is idle waiting for the keyboard.
pub(crate) const WAIT_4_KB_END: u16 = 0xE33F;

/// `APOW_CTR_H` ($7B0A), first byte of the 24-bit inactivity counter (big-endian, $7B0A-$7B0C) that
/// the ROM keeps for the automatic power-off.
///
/// `WAIT_4_KB` sets it to $FE1D1D on entry (`WAIT_4_KB_3`, $E25B) and increments it at each wake-up
/// with no key down (`WAIT_4_KB_13`, $E2E4); when it overflows past $FFFFFF it jumps to `AUTO_OFF`
/// ($E33F), which switches the machine off. That is $1E2E3 (123619) wake-ups; measured in the
/// emulator, with the cursor blinking at the prompt, ~4.9 per frame (~420 per emulated second),
/// that is some 5 minutes of inactivity at real speed.
const APOW_CTR: u32 = 0x7B0A;

/// Initial value of the power-off counter, the same that the ROM writes at $E25B-$E266 (`LDI A,$FE`
/// / `LDI A,$1D` ×2).
const APOW_CTR_START: [u8; 3] = [0xFE, 0x1D, 0x1D];

pub struct Pc1500 {
    lh5801: Lh5801,
    lh5810: Lh5810,
    pd1990ac: Pd1990ac,
    memory: MemoryBus,
    keyboard: Keyboard,
    display: DisplayController,
    /// If `false`, the ROM's automatic power-off is prevented (see [`Pc1500::set_auto_power_off`]).
    auto_power_off: bool,
    /// "Latch" of the CPU's `DISP` signal: the last CPU cycle (`Lh5801::get_ticks`) at the end of
    /// an instruction with `DISP` on, or `None` if it has never been on since [`Pc1500::new`].
    /// [`Pc1500::run_for_ticks`] updates it after each instruction. See [`Pc1500::display`] for
    /// why.
    disp_on_at: Option<usize>,
    /// CPU cycle from which the per-frame hooks are due again (see `Pc1500::run_frame_hooks`). It
    /// is measured in emulated cycles, not in calls: so it makes no difference in which pieces the
    /// emulation is split (whole frames of [`Pc1500::step_frame`] or the GUI's variable budgets
    /// with [`Pc1500::run_for_ticks`]).
    next_frame_hook_at: usize,
    /// The host's key feeder (the PC's keyboard, the mouse, pasted text), which presses and
    /// releases the keys in emulated time: see `key_feeder.rs` and [`Pc1500::host_key_down`].
    feeder: KeyFeeder,
    /// `CLOAD`/`CSAVE`/`MERGE`/`CHAIN` with host files: folder, tape, the command in progress and
    /// the events for the GUI (see `host_io.rs` and [`Pc1500::set_host_dir`]).
    host_io: HostIo,
}

impl Pc1500 {
    /// A PC-1500 with the CE-161 expansion (user RAM `&0100-&47FF`).
    #[must_use]
    pub fn new() -> Self {
        Self::with_memory(MemoryBus::new())
    }

    /// A PC-1500A with the CE-161 expansion: 4 KB more RAM, user RAM
    /// `&0100-&57FF`. Same ROM: A04 is the PC-1500A's ROM too (the version
    /// table at the top of Jeff Birt's listing: "A04 = ROM2 - PC1500 =
    /// PC1500A = PC2").
    #[must_use]
    pub fn new_pc1500a() -> Self {
        Self::with_memory(MemoryBus::new_pc1500a())
    }

    /// Last address of user RAM of the emulated model.
    #[must_use]
    pub fn user_memory_end(&self) -> u16 {
        self.memory.user_memory_end as u16
    }

    /// `true` for a PC-1500A ([`Pc1500::new_pc1500a`]); [`Pc1500::reset`]
    /// keeps the model.
    #[must_use]
    pub fn is_pc1500a(&self) -> bool {
        self.memory.user_memory_end == memory::PC1500A_USER_MEMORY_END
    }

    fn with_memory(memory: MemoryBus) -> Self {
        let mut pc = Self {
            lh5801: Lh5801::new(),
            memory,
            keyboard: Keyboard::new(),
            display: DisplayController::new(),
            lh5810: Lh5810::new(),
            pd1990ac: Pd1990ac::new(),
            auto_power_off: true,
            disp_on_at: None,
            next_frame_hook_at: 0,
            feeder: KeyFeeder::new(),
            host_io: HostIo::default(),
        };
        pc.fix_boot_fixture();
        pc
    }

    fn run(&mut self) {
        self.step_cpu();

        self.step();

        self.keyboard.set_ks(self.lh5810.get_reg(lh5810::Reg::DDA));

        if self.lh5810.int() {
            self.lh5801.set_ir2(true);
        }
    }

    /// Runs a "frame": [`TICKS_PER_FRAME`] CPU cycles (~11.5 ms of real hardware time). It is
    /// exactly `run_for_ticks(TICKS_PER_FRAME)`; see [`Pc1500::run_for_ticks`].
    pub fn step_frame(&mut self) {
        self.run_for_ticks(TICKS_PER_FRAME);
    }

    /// Runs instructions (with their peripherals and interrupts) until at least `budget` CPU cycles
    /// are spent and returns the ones really spent: `budget` plus what the last instruction goes
    /// over, as it is not split (a few cycles). `budget = 0` runs nothing.
    ///
    /// It is the way to move the emulation on in pieces of any size: the GUI calls it with a budget
    /// that comes from the real time elapsed and the speed multiplier (some 433 cycles per repaint
    /// at 0.02x on a 60 Hz monitor, millions in Max mode), and [`Pc1500::step_frame`] is the case
    /// of a whole frame. For both ways to behave the same, EVERYTHING that used to be done "once
    /// per frame" lives in here and is measured in emulated cycles, not in calls:
    ///
    /// - The `DISP` latch is updated after each instruction (see [`Pc1500::display`]).
    /// - The per-frame hooks (`Pc1500::run_frame_hooks`: the key feeder of
    ///   [`Pc1500::host_key_down`] and the brake on the automatic power-off of
    ///   [`Pc1500::set_auto_power_off`]) run before the first instruction that starts once
    ///   [`TICKS_PER_FRAME`] cycles have passed since the previous time. With `step_frame()` that
    ///   is right at the start of each frame, as before; with smaller pieces, every
    ///   `TICKS_PER_FRAME` emulated cycles all the same, however many calls there are in between.
    ///
    /// Splitting the same cycles in different pieces gives exactly the same emulation, except for
    /// what the caller does between pieces (pressing keys, looking at the LCD...).
    pub fn run_for_ticks(&mut self, budget: usize) -> usize {
        let start = self.lh5801.get_ticks();
        let end = start.saturating_add(budget);
        // See `Pc1500::halted_states` (fast forward in `HLT`): never skip either the end of the
        // budget or the next per-frame hook.
        self.lh5801
            .set_halt_deadline(end.min(self.next_frame_hook_at));

        while self.lh5801.get_ticks() < end {
            if self.lh5801.get_ticks() >= self.next_frame_hook_at {
                self.run_frame_hooks();
                self.lh5801
                    .set_halt_deadline(end.min(self.next_frame_hook_at));
            }

            self.run();

            // See `Pc1500::display`: latch `DISP` after each instruction so that the brief
            // `RDP`...`SDP` of the keyboard loop does not turn the LCD off.
            if self.lh5801.display_enabled() {
                self.disp_on_at = Some(self.lh5801.get_ticks());
            }
        }

        self.lh5801.get_ticks() - start
    }

    /// What has to be done once per emulated frame (every [`TICKS_PER_FRAME`] cycles), called from
    /// [`Pc1500::run_for_ticks`]. Any new per-frame hook goes here, not in [`Pc1500::step_frame`],
    /// or the GUI, which calls `run_for_ticks` directly, would skip it.
    ///
    /// The key feeder (`Pc1500::feed_keys`) goes here for that very reason: its key presses are
    /// counted in emulated frames, whatever the speed and the size of the pieces the GUI splits the
    /// emulation in.
    fn run_frame_hooks(&mut self) {
        self.next_frame_hook_at = self.lh5801.get_ticks() + TICKS_PER_FRAME;
        self.feed_keys();
        self.hold_auto_power_off_counter();
    }

    /// CPU cycles (LH5801 states, [`CPU_HZ`] per second of real hardware time) run since
    /// [`Pc1500::new`] or the last [`Pc1500::reset`]. It never goes back except with `reset`.
    #[must_use]
    pub fn ticks(&self) -> usize {
        self.lh5801.get_ticks()
    }

    /// Enables (`true`, the default) or disables the fast forward of the CPU halted in `HLT` (see
    /// `Pc1500::halted_states`).
    ///
    /// It is an optimization with no visible effect on the emulation: with it, the ROM idle at the
    /// prompt runs some 7 times faster on the host. The switch exists to be able to check it (the
    /// tests run the same script with and without it and compare cycles, LCD and RAM) and to rule
    /// it out if it is ever suspected.
    pub const fn set_fast_halt(&mut self, enabled: bool) {
        self.lh5801.set_fast_halt(enabled);
    }

    /// `true` if the fast forward in `HLT` is enabled (see [`Pc1500::set_fast_halt`]).
    #[must_use]
    pub const fn fast_halt(&self) -> bool {
        self.lh5801.fast_halt()
    }

    /// `true` if the machine is on (the LH5801's `BF` flag). It becomes `false` when the ROM runs
    /// `OFF`: with the `OFF` key or by the automatic power-off after ~5 minutes without pressing
    /// anything (see [`Pc1500::set_auto_power_off`]). While it is off the LCD is blank, the CPU
    /// runs nothing and only the `ON` key switches it on again.
    #[must_use]
    pub const fn is_powered_on(&self) -> bool {
        self.lh5801.bf()
    }

    /// Enables (`true`, the default, as on the real machine) or disables the ROM's automatic
    /// power-off.
    ///
    /// The PC-1500 switches itself off after some 5 minutes waiting for the keyboard without
    /// anything pressed (sooner in host time if the emulator runs above 1x). The ROM has no option
    /// to avoid it, so, disabled, the emulator prevents it from outside: once per emulated frame
    /// (every [`TICKS_PER_FRAME`] cycles, see [`Pc1500::run_for_ticks`]), if the ROM is in
    /// `WAIT_4_KB` and its inactivity counter ($7B0A-$7B0C) has gone past $FF0000, it sets it back
    /// to its initial value $FE1D1D, the same that the ROM itself writes on entering `WAIT_4_KB`.
    /// From $FF0000 there are still 65536 wake-ups (~13000 frames, some 2.5 emulated minutes) to
    /// the overflow, so there is no way for it to slip through between two frames, and outside
    /// `WAIT_4_KB` it is not touched (the RAM belongs to the ROM and to the programs). Also useful
    /// for long tests.
    pub const fn set_auto_power_off(&mut self, enabled: bool) {
        self.auto_power_off = enabled;
    }

    /// `true` if the automatic power-off is enabled (see [`Pc1500::set_auto_power_off`]).
    #[must_use]
    pub const fn auto_power_off(&self) -> bool {
        self.auto_power_off
    }

    /// Restarts the machine as if its main batteries were taken out and put back but NOT the
    /// clock's: everything goes back to the state of [`Pc1500::new`] (CPU, RAM with the boot
    /// program, LCD, keyboard, LH5810...) and the ROM does a cold start (`NEW0? :CHECK`). The model
    /// (PC-1500 or PC-1500A) is kept.
    ///
    /// # What survives the reset
    /// This is the complete list; any new state that must be kept has to be added here explicitly:
    ///
    /// - The whole `µPD1990AC` clock (the time, a time set with `TIME=`, its time base from
    ///   [`Pc1500::set_rtc_time_base`]), which on the real machine has its own backup battery. The
    ///   ROM's `RESET` routine only sends it command 0 ($E0FF-$E101), which does not touch the
    ///   time.
    /// - The automatic power-off option ([`Pc1500::set_auto_power_off`]), which is an option of the
    ///   emulator, not state of the machine.
    /// - The fast forward in `HLT` option ([`Pc1500::set_fast_halt`]), likewise.
    /// - The configuration of `CLOAD`/`CSAVE` with host files: the folder
    ///   ([`Pc1500::set_host_dir`]) and the tape file ([`Pc1500::set_tape`]), and the events the
    ///   GUI has not collected yet ([`Pc1500::take_host_events`]). Not a command in progress.
    ///
    /// What does NOT survive, on purpose: the key feeder (host keys down and pasted text pending,
    /// see [`Pc1500::host_key_down`]) starts empty, as does the keyboard matrix. A host key still
    /// down during the reset is not pressed again by itself: its later `host_key_up` does nothing.
    ///
    /// [`Pc1500::ticks`] goes back to 0.
    pub fn reset(&mut self) {
        let fresh = if self.is_pc1500a() { Self::new_pc1500a() } else { Self::new() };
        let old = mem::replace(self, fresh);
        self.set_fast_halt(old.fast_halt());
        self.pd1990ac = old.pd1990ac;
        // The new CPU starts with `timer_state` at 0.
        self.pd1990ac.rebase_timer(self.lh5801.timer_state());
        self.auto_power_off = old.auto_power_off;
        self.host_io.keep_config_from(old.host_io);
    }

    /// Replaces the `µPD1990AC` clock with a new chip of the given time base (see [`RtcTimeBase`]),
    /// starting at that time. [`Pc1500::new`] uses [`RtcTimeBase::HostLocal`] (the host's local
    /// time, what the GUI wants); the tests use [`RtcTimeBase::Emulated`] to be deterministic (see
    /// [`Pc1500::new_deterministic`]).
    ///
    /// Meant to be called right after [`Pc1500::new`], before emulating anything: the new chip
    /// loses the state of its pins and of the command it latched (the ROM sends them again on every
    /// use, so in practice it makes no difference, but it is not what would happen on the real
    /// machine).
    pub fn set_rtc_time_base(&mut self, time_base: RtcTimeBase) {
        self.pd1990ac = Pd1990ac::with_time_base(time_base);
        self.pd1990ac.rebase_timer(self.lh5801.timer_state());
    }

    /// Current date and time of the `µPD1990AC` clock (what `TIME` would read).
    pub fn rtc_now(&mut self) -> RtcDateTime {
        self.pd1990ac.sync(self.lh5801.timer_state());
        self.pd1990ac.now()
    }

    /// `true` if the PC is inside the ROM routine `WAIT_4_KB` ($E243-$E33E), asleep in its `HLT` or
    /// not.
    pub(crate) const fn cpu_in_wait_4_kb(&self) -> bool {
        let p = self.lh5801.p();
        p >= WAIT_4_KB_START && p < WAIT_4_KB_END
    }

    /// See [`Pc1500::set_auto_power_off`].
    fn hold_auto_power_off_counter(&mut self) {
        if self.auto_power_off || !self.lh5801.bf() || !self.cpu_in_wait_4_kb() {
            return;
        }
        if self.read_byte(APOW_CTR) == 0xFF {
            for (addr, value) in (APOW_CTR..).zip(APOW_CTR_START) {
                self.write_byte(addr, value);
            }
        }
    }

    /// Executes one CPU instruction together with the peripherals (timers,
    /// keyboard strobe, interrupts): the unit of work `step_frame` repeats.
    /// Lets a caller observe the machine between two instructions, e.g. a
    /// test harness that hooks ROM routines by address, without losing the
    /// hardware timing the ROM relies on (`step_cpu` alone skips it).
    pub fn step_instruction(&mut self) {
        self.run_for_ticks(1);
    }

    /// Read-only access to the LH5801 CPU, to look at its registers (e.g.
    /// from a test harness that loads a `.lh5` and checks the state it leaves
    /// after running it with [`Pc1500::step_cpu`] / [`Pc1500::read_byte`]).
    #[must_use]
    pub fn cpu(&self) -> &Lh5801 {
        &self.lh5801
    }

    /// The LCD as it shows right now (see [`DisplayController`]).
    ///
    /// The LCD shows lit if the machine is on (`BF`) and the CPU's `DISP` signal (`SDP`/`RDP`) is
    /// on now or was at some point in the last [`LCD_PERSISTENCE_TICKS`] emulated cycles:
    /// `lcd_on = bf && (latch || disp)`.
    ///
    /// # Why a latch and not the instant value of `DISP`
    /// The ROM turns the LCD off for an instant on purpose: in the idle keyboard loop
    /// (`WAIT_4_KB_17`, $E30D `RDP` / `LDI UL,$BC` / `LOP UL` / $E313 `SDP`, some 1.6 ms, 2090
    /// cycles) and in `RESET` ($E006 `RDP`, $E00C `SDP`, $E012 `RDP`). On the real machine that
    /// does not show: the liquid crystal takes much longer to react. It used to look only at the
    /// value of `DISP` at the instant `display()` was called (between two frames), and if that
    /// instant fell inside the pulse, the GUI painted a whole frame with the LCD blank: a flicker
    /// the real machine does not have. Now [`Pc1500::run_for_ticks`] notes after each instruction
    /// the last cycle with `DISP` on, so it is enough for it to have been on once in the window.
    ///
    /// # Why a window in emulated cycles
    /// The first version of the latch covered the interval between two calls to `display()`, which
    /// the GUI makes once per repaint. That held while each repaint emulated at least a whole
    /// frame, but with the real-time scheduler (see [`Pc1500::run_for_ticks`]) at 0.02x each
    /// repaint emulates only some 433 cycles: windows much shorter than the `RDP` pulse itself
    /// (measured at the prompt: 2090 cycles, every ~6.3 million, some 4.8 emulated s), so in slow
    /// motion the LCD would have gone off for a few repaints in a row at each pulse: the flicker of
    /// before, in slow motion. Measured in emulated cycles, the persistence depends neither on how
    /// often anyone looks at the LCD nor on the emulation speed: it is the same at 0.02x, at 1x and
    /// in Max mode, and in the tests.
    ///
    /// Calling `display()` twice in a row without emulating anything in between gives the same
    /// result.
    ///
    /// With the machine off (`OFF` or automatic power-off, see [`Pc1500::is_powered_on`]) the whole
    /// LCD, indicators included, shows blank although the display RAM keeps what was there (the ROM
    /// paints it again on resuming).
    pub fn display(&mut self) -> &DisplayController {
        let now = self.lh5801.get_ticks();
        let recently_on = self
            .disp_on_at
            .is_some_and(|at| now.saturating_sub(at) < LCD_PERSISTENCE_TICKS);
        let lcd_on = self.lh5801.bf() && (recently_on || self.lh5801.display_enabled());
        self.update_display_buffer(lcd_on);
        &self.display
    }

    pub fn press(&mut self, key: Key) {
        self.keyboard.press(key);
    }

    pub fn release(&mut self, key: Key) {
        self.keyboard.release(key);
    }

    fn step(&mut self) {
        // µPD1990AC (see `pd1990ac.rs`): first its emulated clock catches up with the cycles of the
        // instruction the CPU has just run; then, if that instruction wrote the LH5810's port C
        // (PC0 = DATA IN, PC1 = STB, PC2 = CLK, PC3 = C0 and OUT ENBL, PC4 = C1, PC5 = C2), it gets
        // the whole byte, which is what it needs to detect the edges of STB and CLK. One write per
        // instruction is enough (see `Pd1990ac::write_port`).
        self.pd1990ac.sync(self.lh5801.timer_state());
        if self.lh5810.new_opc() {
            let opc = self.lh5810.get_reg(lh5810::Reg::OPC);
            self.pd1990ac.write_port(opc);
            self.lh5810.set_new_opc(false);
        }

        // Outputs of the µPD1990AC: TP → PB5 (the time base of `WAIT`/`PAUSE`/`BEEP`) and DATA OUT
        // → PB6 (the bits of `TIME`).
        self.lh5810
            .set_reg_bit(lh5810::Reg::OPB, 5, self.pd1990ac.tp());
        self.lh5810
            .set_reg_bit(lh5810::Reg::OPB, 6, self.pd1990ac.data_out());

        // ON/BRK → PB7, as a pulse of a single `run()` per key press (see `Keyboard::on_pulse`):
        // enough, because `Lh5810::step` latches it in bit 1 of `IF` and there it stays until the
        // ROM clears it when it serves the BREAK. With the machine off, the same key press switches
        // it on and does NOT also generate a BREAK, as on the real one: the `RESET` routine sets
        // `IF` to 0 ($E028) anyway.
        let on_pulse = self.keyboard.take_on_pulse();
        if on_pulse && !self.lh5801.bf() {
            self.lh5801.power_on();
        } else {
            self.lh5810.set_reg_bit(lh5810::Reg::OPB, 7, on_pulse);
        }

        self.lh5810.set_reg_bit(lh5810::Reg::OPB, 3, true); // Export model vs domestic model
        self.lh5810.set_reg_bit(lh5810::Reg::OPB, 4, false); // PB4 to GND

        self.lh5810.step(self.lh5801.timer_state());
    }

    /// Loads a file of LH5801 machine code (the `.lh5` format) into the
    /// PC-1500's user memory and sets the Program Counter to start running at
    /// the load address.
    ///
    /// # The file
    /// ```text
    /// bytes 0-1: the load address (u16 little-endian)
    /// bytes 2-3: the length of the code (u16 little-endian)
    /// bytes 4+ : the LH5801 machine code
    /// ```
    ///
    /// # Errors
    /// Returns [`lh5_loader::Lh5LoadError`] if the file does not exist, its
    /// format is wrong or the load address is outside user memory (`0x0100`-
    /// `0x47FF`, or `0x57FF` on a PC-1500A).
    pub fn load_lh5_file(&mut self, path: &Path) -> Result<(), lh5_loader::Lh5LoadError> {
        use lh5_loader::{read_lh5_file, validate_load_parameters};

        // A new `Pc1500` (`Pc1500::new()`) starts with a CPU reset pending
        // (`reset_flag`): the FIRST `step_cpu()`, whenever it runs, takes it by
        // jumping to the ROM's real boot vector ($FFFE), overwriting any PC set
        // before, including the one this method sets below. If nobody has taken
        // that reset yet (e.g. by calling `step_cpu()` once by hand, as the
        // compiler's test harness does), the program loaded never runs a single
        // instruction: it is lost as soon as the GUI's loop starts. Taking it
        // here, inside the load, makes `load_lh5_file` safe to call straight
        // after `Pc1500::new()` without every caller having to know.
        self.step_cpu();

        let (load_address, machine_code) = read_lh5_file(path)?;
        validate_load_parameters(load_address, machine_code.len(), self.user_memory_end())?;

        let start = load_address as usize;
        let end = start + machine_code.len();

        // Write the code straight into standard_user_memory.
        let offset = start - memory::STANDARD_USER_MEMORY_BEGIN as usize;
        self.memory.standard_user_memory[offset..offset + machine_code.len()]
            .copy_from_slice(&machine_code);

        // Point PC at the load address so that the CPU runs the code
        self.lh5801.set_pc(load_address);
        // If the CPU was still stopped (the previous program reached
        // END/STOP), take it out of HALT; otherwise step_cpu() runs nothing
        // even with PC at the program just loaded (see Lh5801::clear_halt).
        self.lh5801.clear_halt();
        // And if it was off (`OFF` or automatic power-off), switch it on without going through the
        // ROM's reset, which would overwrite the PC just set: with `BF` at 0 `step_cpu()` runs
        // nothing.
        self.lh5801.set_bf(true);

        println!(
            "LH5 file loaded: {} at 0x{:04X}..0x{:04X} ({} bytes), PC=0x{:04X}",
            path.display(),
            start,
            end - 1,
            machine_code.len(),
            load_address,
        );

        Ok(())
    }
}
