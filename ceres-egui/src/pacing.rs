//! Real-time scheduler of the emulation ("pacer"): decides how many CPU cycles to emulate in each
//! `update()` of the GUI so that the PC-1500 goes at the speed asked for, and measures the one it
//! really gets.
//!
//! # Why it is needed
//! The GUI used to emulate at most ONE frame (`step_frame()`, 15000 cycles = ~11.5 emulated ms) per
//! `update()`, and only if at least `FRAME_DURATION / multiplier` had gone by since the previous
//! one. With vsync at 60 Hz `update()` comes every ~16.7 ms, so the emulation went at most at 11.5
//! / 16.7 = 0.69x, whatever the slider asked for (on a 144 Hz monitor, at ~1.66x). Besides, the
//! time left over between frames was thrown away at each `update()` and there was no way to make up
//! for a hiccup.
//!
//! # How it works
//! It keeps a "debt" of cycles: each `update()` adds to it the real time elapsed ×
//! [`ceres_core::CPU_HZ`] × multiplier, and it emulates in pieces ([`CHUNK_TICKS`]) until it is
//! paid off or the `update()`'s budget of host time runs out (so that the GUI stays responsive).
//! The excess of the last instruction of each piece remains as negative debt, so there is no drift
//! in the long run.
//!
//! - If the host cannot keep up with the speed asked for and the budget runs out, the pending debt
//!   is DROPPED instead of being carried over: carrying it would make each `update()` owe more than
//!   the previous one (the "death spiral") and the GUI would end up frozen. It is reported with
//!   [`Measurement::host_limited`].
//! - The elapsed time is limited to [`MAX_DT`]: with the window minimized or hidden `update()` may
//!   not be called for seconds (eframe sleeps 10 ms per minimized frame and on Wayland a hidden
//!   window gets no repaints), and on coming back there must be no burst to "make up" that time.
//! - In [`SpeedMode::Max`] mode there is no debt: it emulates nonstop until the budget runs out.
//!
//! It is pure logic, without egui: the clock (`now`), the end of the budget (`deadline_passed`) and
//! the emulation (`run_ticks`) come as parameters, so the tests use a fake clock and emulation.

use std::time::{Duration, Instant};

/// [`ceres_core::CPU_HZ`] in floating point.
#[expect(clippy::cast_precision_loss, reason = "1.3e6 fits exactly in an f64")]
const CPU_HZ_F: f64 = ceres_core::CPU_HZ as f64;

/// Maximum size of each piece of emulation between two looks at the clock: one frame, ~80-150 µs of
/// host time in release. Smaller = a more precise budget but more calls; it makes no difference to
/// the emulation, which does not depend on how it is split (see `Pc1500::run_for_ticks`).
pub const CHUNK_TICKS: usize = ceres_core::TICKS_PER_FRAME;

/// Maximum real time accepted between two `update()`s (see the module's comment).
pub const MAX_DT: Duration = Duration::from_millis(100);

/// Window over which the effective speed is measured.
pub const MEASURE_WINDOW: Duration = Duration::from_millis(500);

/// Emulation speed asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpeedMode {
    /// Multiple of real hardware time (1.0 = a real PC-1500).
    Multiplier(f64),
    /// As much as the host gives ("turbo").
    Max,
}

/// Measurement of the effective speed during the last complete window of [`MEASURE_WINDOW`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    /// Emulated cycles per real second, as a multiple of [`ceres_core::CPU_HZ`] (1.0 = real
    /// hardware time).
    pub multiplier: f64,
    /// Emulated cycles per real second, in MHz.
    pub emulated_mhz: f64,
    /// `true` if in that window not everything asked for could be emulated (debt was dropped) or it
    /// was in Max mode: the speed is set by the host.
    pub host_limited: bool,
}

/// See the module's comment.
#[derive(Debug, Default)]
pub struct Pacer {
    /// Cycles owed to the emulation (it can be slightly negative: the excess of the last
    /// instruction of a piece).
    debt_ticks: f64,
    /// Instant of the previous `advance`, or `None` after [`Pacer::pause`].
    last_update: Option<Instant>,
    /// Start of the measurement window in progress.
    window_start: Option<Instant>,
    window_ticks: u64,
    window_host_limited: bool,
    measurement: Option<Measurement>,
}

impl Pacer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The emulation stops (pause, step by step, Reset): forgets the elapsed time and the debt, so
    /// that there is no jump on resuming, and drops the measurement (it makes no sense while it is
    /// not running).
    pub const fn pause(&mut self) {
        self.debt_ticks = 0.0;
        self.last_update = None;
        self.window_start = None;
        self.window_ticks = 0;
        self.window_host_limited = false;
        self.measurement = None;
    }

    /// The last measurement of effective speed, if there is already a complete window since the
    /// last [`Pacer::pause`].
    #[must_use]
    pub const fn measurement(&self) -> Option<Measurement> {
        self.measurement
    }

    /// Moves the emulation on as much as is due in this `update()` and returns the cycles emulated.
    ///
    /// - `now`: the instant of this `update()`.
    /// - `deadline_passed`: whether this `update()`'s budget of host time has run out already; it
    ///   is checked after each piece.
    /// - `run_ticks(n)`: emulates at least `n` cycles and returns those it emulated
    ///   (`Pc1500::run_for_ticks`).
    ///
    /// The first `advance` (or the first after [`Pacer::pause`]) only starts the clock: there is no
    /// elapsed time to emulate.
    pub fn advance(
        &mut self,
        now: Instant,
        mode: SpeedMode,
        mut deadline_passed: impl FnMut() -> bool,
        mut run_ticks: impl FnMut(usize) -> usize,
    ) -> u64 {
        let dt = self
            .last_update
            .map_or(Duration::ZERO, |t| now.saturating_duration_since(t))
            .min(MAX_DT);
        self.last_update = Some(now);
        let window_start = *self.window_start.get_or_insert(now);

        let mut ran: u64 = 0;
        match mode {
            SpeedMode::Max => {
                self.debt_ticks = 0.0;
                loop {
                    ran += to_u64(run_ticks(CHUNK_TICKS));
                    if deadline_passed() {
                        break;
                    }
                }
                self.window_host_limited = true;
            }
            SpeedMode::Multiplier(multiplier) => {
                self.debt_ticks += dt.as_secs_f64() * CPU_HZ_F * multiplier.max(0.0);
                loop {
                    if self.debt_ticks < 1.0 {
                        break;
                    }
                    let got = run_ticks(debt_chunk(self.debt_ticks));
                    ran += to_u64(got);
                    self.debt_ticks -= to_f64(got);
                    if self.debt_ticks >= 1.0 && deadline_passed() {
                        // No death spiral: what there was no time to emulate is dropped (see the
                        // module's comment).
                        self.debt_ticks = 0.0;
                        self.window_host_limited = true;
                    }
                }
            }
        }

        self.window_ticks += ran;
        let elapsed = now.saturating_duration_since(window_start);
        if elapsed >= MEASURE_WINDOW {
            let ticks_per_sec = to_f64_u64(self.window_ticks) / elapsed.as_secs_f64();
            self.measurement = Some(Measurement {
                multiplier: ticks_per_sec / CPU_HZ_F,
                emulated_mhz: ticks_per_sec / 1e6,
                host_limited: self.window_host_limited,
            });
            self.window_start = Some(now);
            self.window_ticks = 0;
            self.window_host_limited = false;
        }
        ran
    }
}

/// Budget of host time of an `update()` devoted to emulating, from egui's `predicted_dt` (how long
/// the GUI's frame is expected to last, 1/60 s with vsync at 60 Hz).
///
/// Usually 60% of the frame, to leave the rest to the GUI and keep its fps (10 ms at 60 Hz, 4.2 ms
/// at 144 Hz). In Max mode 150%: losing a vsync now and then is accepted (the GUI drops to ~30-40
/// fps) in exchange for devoting more of the host to emulating. Always between 2 and 25 ms.
#[must_use]
pub fn update_budget(predicted_dt: f32, max: bool) -> Duration {
    let frame = if predicted_dt.is_finite() {
        predicted_dt.clamp(1.0 / 240.0, 1.0 / 30.0)
    } else {
        1.0 / 60.0
    };
    let fraction = if max { 1.5 } else { 0.6 };
    Duration::from_secs_f32((frame * fraction).clamp(0.002, 0.025))
}

/// Piece to emulate with `debt` cycles of debt: between 1 and [`CHUNK_TICKS`].
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "it is clamped to [1, CHUNK_TICKS] before converting"
)]
const fn debt_chunk(debt: f64) -> usize {
    debt.clamp(1.0, to_f64(CHUNK_TICKS)) as usize
}

fn to_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

#[expect(clippy::cast_precision_loss, reason = "cycles of an update(): far below 2^52")]
const fn to_f64(n: usize) -> f64 {
    n as f64
}

#[expect(clippy::cast_precision_loss, reason = "cycles of a measurement window: far below 2^52")]
const fn to_f64_u64(n: u64) -> f64 {
    n as f64
}

/// Text of a speed multiplier for the slider and the labels: 2 decimals below 10 (`0.02`, `1.00`,
/// `2.50`) and none from there on (`10`, `1000`).
#[must_use]
pub fn format_multiplier(multiplier: f64) -> String {
    if multiplier < 9.995 {
        format!("{multiplier:.2}")
    } else {
        format!("{multiplier:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GUI frame at 60 Hz.
    const FRAME_60HZ: Duration = Duration::from_nanos(16_666_667);

    /// Fake emulation: emulates exactly what is asked, plus `overshoot` cycles (the last
    /// instruction, which is not split), and counts.
    struct FakeEmu {
        overshoot: usize,
        total: u64,
        calls: usize,
    }

    impl FakeEmu {
        const fn new(overshoot: usize) -> Self {
            Self { overshoot, total: 0, calls: 0 }
        }

        fn run(&mut self, n: usize) -> usize {
            self.calls += 1;
            let got = n + self.overshoot;
            self.total += to_u64(got);
            got
        }
    }

    /// `frames` updates in a row every `dt` (plus the first, which only starts the clock), never
    /// running out of budget.
    fn run_updates(
        pacer: &mut Pacer,
        emu: &mut FakeEmu,
        start: Instant,
        dt: Duration,
        frames: u32,
        mode: SpeedMode,
    ) {
        let mut now = start;
        for _ in 0..=frames {
            pacer.advance(now, mode, || false, |n| emu.run(n));
            now += dt;
        }
    }

    #[test]
    fn real_time_at_1x() {
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(0);
        run_updates(&mut pacer, &mut emu, Instant::now(), FRAME_60HZ, 60, SpeedMode::Multiplier(1.0));
        let expected = 1_300_000_i64;
        let diff = (i64::try_from(emu.total).unwrap_or(i64::MAX) - expected).abs();
        assert!(diff <= i64::try_from(CHUNK_TICKS).unwrap_or(0), "total = {}", emu.total);
    }

    #[test]
    fn overshoot_does_not_drift() {
        // Each piece goes over by 30 cycles: the negative debt makes up for it.
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(30);
        run_updates(&mut pacer, &mut emu, Instant::now(), FRAME_60HZ, 600, SpeedMode::Multiplier(1.0));
        let expected = 13_000_000_i64;
        let diff = (i64::try_from(emu.total).unwrap_or(i64::MAX) - expected).abs();
        assert!(diff <= 1000, "total = {}", emu.total);
    }

    #[test]
    fn slow_motion_runs_a_little_every_update() {
        // 0.02x at 60 Hz: ~433 cycles per update, instead of a whole frame every 575 ms.
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(0);
        let start = Instant::now();
        pacer.advance(start, SpeedMode::Multiplier(0.02), || false, |n| emu.run(n));
        let ran = pacer.advance(start + FRAME_60HZ, SpeedMode::Multiplier(0.02), || false, |n| emu.run(n));
        assert!((432..=434).contains(&ran), "ran = {ran}");
    }

    #[test]
    fn first_advance_only_starts_the_clock() {
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(0);
        assert_eq!(pacer.advance(Instant::now(), SpeedMode::Multiplier(1.0), || false, |n| emu.run(n)), 0);
        assert_eq!(emu.calls, 0);
    }

    #[test]
    fn pause_forgets_elapsed_time() {
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(0);
        let start = Instant::now();
        pacer.advance(start, SpeedMode::Multiplier(1.0), || false, |n| emu.run(n));
        pacer.pause();
        let ran = pacer.advance(start + Duration::from_secs(1), SpeedMode::Multiplier(1.0), || false, |n| emu.run(n));
        assert_eq!(ran, 0, "after the pause the stopped time is not made up");
        assert!(pacer.measurement().is_none());
    }

    #[test]
    fn long_gaps_are_capped() {
        // Window minimized for 5 s: only MAX_DT = 0.1 s is owed.
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(0);
        let start = Instant::now();
        pacer.advance(start, SpeedMode::Multiplier(1.0), || false, |n| emu.run(n));
        let ran = pacer.advance(start + Duration::from_secs(5), SpeedMode::Multiplier(1.0), || false, |n| emu.run(n));
        assert!((129_999..=130_001).contains(&ran), "ran = {ran}");
    }

    #[test]
    fn owed_time_is_dropped_when_the_host_cannot_keep_up() {
        // 1000x with a host that only manages 3 pieces per update: the debt does not grow from one
        // update to the next.
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(0);
        let mut now = Instant::now();
        pacer.advance(now, SpeedMode::Multiplier(1000.0), || false, |n| emu.run(n));
        let mut per_update = Vec::new();
        for _ in 0..40 {
            now += FRAME_60HZ;
            let mut checks = 0;
            let ran_now = pacer.advance(
                now,
                SpeedMode::Multiplier(1000.0),
                || {
                    checks += 1;
                    checks >= 3
                },
                |n| emu.run(n),
            );
            per_update.push(ran_now);
            assert!(pacer.debt_ticks.abs() < 1.0, "debt carried over: {}", pacer.debt_ticks);
        }
        assert!(per_update.iter().all(|&r| r == to_u64(3 * CHUNK_TICKS)), "{per_update:?}");
        let m = pacer.measurement().unwrap_or_else(|| panic!("no measurement"));
        assert!(m.host_limited);
        // 3 frames for each update of 16.7 ms = 2.7 M cycles/s = ~2.08x.
        assert!((m.multiplier - 2.077).abs() < 0.01, "{m:?}");
    }

    #[test]
    fn max_mode_runs_until_the_deadline() {
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(0);
        let start = Instant::now();
        let mut checks = 0;
        let ran = pacer.advance(
            start,
            SpeedMode::Max,
            || {
                checks += 1;
                checks >= 5
            },
            |n| emu.run(n),
        );
        assert_eq!(ran, to_u64(5 * CHUNK_TICKS), "even if it is the first update");
        // Always at least one piece, even if the budget has already run out.
        let ran_late = pacer.advance(start + FRAME_60HZ, SpeedMode::Max, || true, |n| emu.run(n));
        assert_eq!(ran_late, to_u64(CHUNK_TICKS));
    }

    #[test]
    fn measures_effective_speed() {
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(0);
        let start = Instant::now();
        run_updates(&mut pacer, &mut emu, start, FRAME_60HZ, 60, SpeedMode::Multiplier(2.0));
        let m = pacer.measurement().unwrap_or_else(|| panic!("no measurement"));
        assert!((m.multiplier - 2.0).abs() < 0.02, "{m:?}");
        assert!((m.emulated_mhz - 2.6).abs() < 0.03, "{m:?}");
        assert!(!m.host_limited);
    }

    fn assert_budget_ms(predicted_dt: f32, max: bool, expected_ms: f64) {
        let got = update_budget(predicted_dt, max).as_secs_f64() * 1000.0;
        assert!((got - expected_ms).abs() < 0.001, "{predicted_dt} {max}: {got} ms");
    }

    #[test]
    fn budget_follows_the_ui_frame() {
        assert_budget_ms(1.0 / 60.0, false, 10.0);
        assert_budget_ms(1.0 / 60.0, true, 25.0);
        assert_budget_ms(1.0 / 144.0, false, 0.6 * 1000.0 / 144.0);
        // Very slow monitors or absurd dt values: bounded.
        assert_budget_ms(1.0, false, 20.0);
        assert_budget_ms(1.0, true, 25.0);
        assert_budget_ms(0.0, false, 2.5);
        assert_budget_ms(f32::NAN, false, 10.0);
    }

    #[test]
    fn multiplier_text() {
        assert_eq!(format_multiplier(0.02), "0.02");
        assert_eq!(format_multiplier(1.0), "1.00");
        assert_eq!(format_multiplier(9.5), "9.50");
        assert_eq!(format_multiplier(10.0), "10");
        assert_eq!(format_multiplier(1000.0), "1000");
    }
}
