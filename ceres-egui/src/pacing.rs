//! Planificador de tiempo real de la emulación ("pacer"): decide cuántos
//! ciclos de CPU emular en cada `update()` de la GUI para que la PC-1500
//! vaya a la velocidad pedida, y mide la que consigue de verdad.
//!
//! # Por qué hace falta
//! Antes la GUI emulaba como mucho UN frame (`step_frame()`, 15000 ciclos
//! = ~11.5 ms emulados) por `update()`, y solo si había pasado al menos
//! `FRAME_DURATION / multiplicador` desde el anterior. Con vsync a 60 Hz
//! `update()` llega cada ~16.7 ms, así que la emulación iba como mucho a
//! 11.5 / 16.7 = 0.69x, pidiera lo que pidiera el slider (en un monitor
//! de 144 Hz, a ~1.66x). Además el tiempo sobrante entre frames se tiraba
//! en cada `update()` y no había forma de recuperar un tirón.
//!
//! # Cómo funciona
//! Lleva una "deuda" de ciclos: cada `update()` le suma el tiempo real
//! transcurrido × [`ceres_core::CPU_HZ`] × multiplicador, y emula en
//! trozos ([`CHUNK_TICKS`]) hasta saldarla o hasta agotar el presupuesto
//! de tiempo de host del `update()` (para que la interfaz siga
//! respondiendo). El exceso de la última instrucción de cada trozo queda
//! como deuda negativa, así que no hay deriva a largo plazo.
//!
//! - Si el host no da para la velocidad pedida y se agota el presupuesto,
//!   la deuda pendiente se TIRA en vez de arrastrarse: arrastrarla haría
//!   que cada `update()` debiera más que el anterior (la "espiral de la
//!   muerte") y la interfaz acabaría congelada. Se avisa con
//!   [`Measurement::host_limited`].
//! - El tiempo transcurrido se limita a [`MAX_DT`]: con la ventana
//!   minimizada u oculta `update()` puede no llamarse en segundos (eframe
//!   duerme 10 ms por frame minimizado y en Wayland una ventana oculta no
//!   recibe repintados), y al volver no debe haber una ráfaga para
//!   "recuperar" ese tiempo.
//! - En modo [`SpeedMode::Max`] no hay deuda: se emula sin parar hasta
//!   agotar el presupuesto.
//!
//! Es lógica pura, sin egui: el reloj (`now`), el fin del presupuesto
//! (`deadline_passed`) y la emulación (`run_ticks`) llegan como
//! parámetros, así que los tests usan un reloj y una emulación falsos.

use std::time::{Duration, Instant};

/// [`ceres_core::CPU_HZ`] en coma flotante.
#[expect(clippy::cast_precision_loss, reason = "1.3e6 cabe exacto en un f64")]
const CPU_HZ_F: f64 = ceres_core::CPU_HZ as f64;

/// Tamaño máximo de cada trozo de emulación entre dos miradas al reloj:
/// un frame, ~80-150 µs de host en release. Más pequeño = presupuesto más
/// preciso pero más llamadas; da igual para la emulación, que no depende
/// de cómo se trocee (ver `Pc1500::run_for_ticks`).
pub const CHUNK_TICKS: usize = ceres_core::TICKS_PER_FRAME;

/// Tiempo real máximo que se acepta entre dos `update()` (ver el
/// comentario del módulo).
pub const MAX_DT: Duration = Duration::from_millis(100);

/// Ventana sobre la que se mide la velocidad efectiva.
pub const MEASURE_WINDOW: Duration = Duration::from_millis(500);

/// Velocidad de emulación pedida.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpeedMode {
    /// Múltiplo del tiempo real de hardware (1.0 = una PC-1500 real).
    Multiplier(f64),
    /// Todo lo que dé el host ("turbo").
    Max,
}

/// Medida de la velocidad efectiva durante la última ventana completa de
/// [`MEASURE_WINDOW`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    /// Ciclos emulados por segundo real, como múltiplo de
    /// [`ceres_core::CPU_HZ`] (1.0 = tiempo real de hardware).
    pub multiplier: f64,
    /// Ciclos emulados por segundo real, en MHz.
    pub emulated_mhz: f64,
    /// `true` si en esa ventana no se pudo emular todo lo pedido (se tiró
    /// deuda) o se estaba en modo Máx: la velocidad la marca el host.
    pub host_limited: bool,
}

/// Ver el comentario del módulo.
#[derive(Debug, Default)]
pub struct Pacer {
    /// Ciclos que se deben a la emulación (puede ser algo negativa: el
    /// exceso de la última instrucción de un trozo).
    debt_ticks: f64,
    /// Instante del `advance` anterior, o `None` tras [`Pacer::pause`].
    last_update: Option<Instant>,
    /// Inicio de la ventana de medida en curso.
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

    /// La emulación se para (pausa, paso a paso, Reset): olvida el tiempo
    /// transcurrido y la deuda, para que al reanudar no haya un salto, y
    /// descarta la medida (no tiene sentido mientras no corre).
    pub const fn pause(&mut self) {
        self.debt_ticks = 0.0;
        self.last_update = None;
        self.window_start = None;
        self.window_ticks = 0;
        self.window_host_limited = false;
        self.measurement = None;
    }

    /// La última medida de velocidad efectiva, si ya hay una ventana
    /// completa desde el último [`Pacer::pause`].
    #[must_use]
    pub const fn measurement(&self) -> Option<Measurement> {
        self.measurement
    }

    /// Avanza la emulación lo que toque en este `update()` y devuelve los
    /// ciclos emulados.
    ///
    /// - `now`: el instante de este `update()`.
    /// - `deadline_passed`: si se ha agotado ya el presupuesto de tiempo
    ///   de host de este `update()`; se consulta tras cada trozo.
    /// - `run_ticks(n)`: emula al menos `n` ciclos y devuelve los que
    ///   emuló (`Pc1500::run_for_ticks`).
    ///
    /// El primer `advance` (o el primero tras [`Pacer::pause`]) solo
    /// pone en marcha el reloj: no hay tiempo transcurrido que emular.
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
                        // Sin espiral de la muerte: lo que no dio tiempo
                        // a emular se tira (ver el comentario del módulo).
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

/// Presupuesto de tiempo de host de un `update()` dedicado a emular,
/// a partir de `predicted_dt` de egui (lo que se espera que dure el frame
/// de la interfaz, 1/60 s con vsync a 60 Hz).
///
/// Normalmente el 60% del frame, para dejar el resto a la interfaz y
/// mantener sus fps (10 ms a 60 Hz, 4.2 ms a 144 Hz). En modo Máx el
/// 150%: se acepta perder algún vsync (la interfaz baja a ~30-40 fps) a
/// cambio de dedicar más host a emular. Siempre entre 2 y 25 ms.
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

/// Trozo a emular con `debt` ciclos de deuda: entre 1 y [`CHUNK_TICKS`].
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "se recorta a [1, CHUNK_TICKS] antes de convertir"
)]
const fn debt_chunk(debt: f64) -> usize {
    debt.clamp(1.0, to_f64(CHUNK_TICKS)) as usize
}

fn to_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

#[expect(clippy::cast_precision_loss, reason = "ciclos de un update(): muy por debajo de 2^52")]
const fn to_f64(n: usize) -> f64 {
    n as f64
}

#[expect(clippy::cast_precision_loss, reason = "ciclos de una ventana de medida: muy por debajo de 2^52")]
const fn to_f64_u64(n: u64) -> f64 {
    n as f64
}

/// Texto de un multiplicador de velocidad para el slider y las etiquetas:
/// 2 decimales por debajo de 10 (`0.02`, `1.00`, `2.50`) y ninguno a
/// partir de ahí (`10`, `1000`).
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

    /// Un frame de interfaz a 60 Hz.
    const FRAME_60HZ: Duration = Duration::from_nanos(16_666_667);

    /// Emulación falsa: emula exactamente lo pedido, más `overshoot`
    /// ciclos (la última instrucción que no se parte), y cuenta.
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

    /// `frames` updates seguidos cada `dt` (más el primero, que solo
    /// pone en marcha el reloj), sin agotar nunca el presupuesto.
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
        // Cada trozo se pasa 30 ciclos: la deuda negativa lo compensa.
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(30);
        run_updates(&mut pacer, &mut emu, Instant::now(), FRAME_60HZ, 600, SpeedMode::Multiplier(1.0));
        let expected = 13_000_000_i64;
        let diff = (i64::try_from(emu.total).unwrap_or(i64::MAX) - expected).abs();
        assert!(diff <= 1000, "total = {}", emu.total);
    }

    #[test]
    fn slow_motion_runs_a_little_every_update() {
        // 0.02x a 60 Hz: ~433 ciclos por update, en vez de un frame entero
        // cada 575 ms.
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
        assert_eq!(ran, 0, "tras la pausa no se recupera el tiempo parado");
        assert!(pacer.measurement().is_none());
    }

    #[test]
    fn long_gaps_are_capped() {
        // Ventana minimizada 5 s: solo se deben MAX_DT = 0.1 s.
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(0);
        let start = Instant::now();
        pacer.advance(start, SpeedMode::Multiplier(1.0), || false, |n| emu.run(n));
        let ran = pacer.advance(start + Duration::from_secs(5), SpeedMode::Multiplier(1.0), || false, |n| emu.run(n));
        assert!((129_999..=130_001).contains(&ran), "ran = {ran}");
    }

    #[test]
    fn owed_time_is_dropped_when_the_host_cannot_keep_up() {
        // 1000x con un host que solo da para 3 trozos por update: la
        // deuda no crece de un update a otro.
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
            assert!(pacer.debt_ticks.abs() < 1.0, "deuda arrastrada: {}", pacer.debt_ticks);
        }
        assert!(per_update.iter().all(|&r| r == to_u64(3 * CHUNK_TICKS)), "{per_update:?}");
        let m = pacer.measurement().unwrap_or_else(|| panic!("sin medida"));
        assert!(m.host_limited);
        // 3 frames por cada update de 16.7 ms = 2.7 M ciclos/s = ~2.08x.
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
        assert_eq!(ran, to_u64(5 * CHUNK_TICKS), "aunque sea el primer update");
        // Siempre al menos un trozo, aunque el presupuesto ya esté agotado.
        let ran_late = pacer.advance(start + FRAME_60HZ, SpeedMode::Max, || true, |n| emu.run(n));
        assert_eq!(ran_late, to_u64(CHUNK_TICKS));
    }

    #[test]
    fn measures_effective_speed() {
        let mut pacer = Pacer::new();
        let mut emu = FakeEmu::new(0);
        let start = Instant::now();
        run_updates(&mut pacer, &mut emu, start, FRAME_60HZ, 60, SpeedMode::Multiplier(2.0));
        let m = pacer.measurement().unwrap_or_else(|| panic!("sin medida"));
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
        // Monitores lentísimos o dt absurdos: acotado.
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
