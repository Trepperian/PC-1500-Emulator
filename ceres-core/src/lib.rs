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

/// Frecuencia de reloj de la CPU LH5801 de la PC-1500, en ciclos por
/// segundo.
///
/// Son ciclos de máquina ("ticks", [`Pc1500::ticks`]) por segundo de
/// tiempo real de hardware: el oscilador de 2.6 MHz dividido por 2 dentro
/// de la propia CPU (lo mismo que `setfrequency(2600000/2)` en `PockEmul`). Es la
/// misma constante con la que cuenta el tiempo el reloj `µPD1990AC`
/// emulado.
///
/// Es lo que necesita cualquier planificador de tiempo real (la GUI):
/// `segundos × CPU_HZ × multiplicador` ticks que ejecutar con
/// [`Pc1500::run_for_ticks`]. Mejor esto que [`FRAME_DURATION`]: al
/// calcular `NANOS_PER_TICK` con enteros, `1e9 / 1.3e6` se trunca a 769
/// ns y `FRAME_DURATION` sale un 0.03% corta.
pub const CPU_HZ: usize = pd1990ac::FREQUENCY;

/// Ciclos de CPU que ejecuta una llamada a [`Pc1500::step_frame`].
///
/// Son ~11.5 ms de tiempo real de hardware a [`CPU_HZ`]. Es la unidad de tiempo
/// del arnés de tests ("frames") y el periodo de los ganchos por frame de
/// [`Pc1500::run_for_ticks`].
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
/// Se mantiene por compatibilidad: la GUI ya no ritma frames enteros,
/// sino ticks sueltos con [`Pc1500::run_for_ticks`] y [`CPU_HZ`] (sin el
/// truncado de `NANOS_PER_TICK`), ver `ceres-egui/src/pacing.rs`.
pub const FRAME_DURATION: Duration = Duration::from_nanos(TICKS_PER_FRAME as u64 * NANOS_PER_TICK);

/// Persistencia del LCD, en ciclos de CPU.
///
/// El panel se sigue viendo encendido hasta este tiempo emulado después
/// de que la CPU apague la señal `DISP` (ver [`Pc1500::display`]). Un
/// frame, ~11.5 ms: de sobra para tapar el pulso `RDP`...`SDP` de ~1.6
/// ms (2090 ciclos) del bucle ocioso de teclado, y bastante menos que lo
/// que tarda en reaccionar un LCD real, así que un programa que apague
/// el LCD de verdad se sigue viendo apagado.
pub const LCD_PERSISTENCE_TICKS: usize = TICKS_PER_FRAME;

/// Inicio de la rutina de la ROM `WAIT_4_KB` ($E243): espera de una
/// tecla con cursor parpadeante y auto-apagado.
pub(crate) const WAIT_4_KB_START: u16 = 0xE243;

/// Fin (exclusivo) del bloque `WAIT_4_KB`..`AUTO_OFF` ($E33F). Mientras
/// la CPU esté detenida en `HLT` con el PC dentro de este rango, la ROM
/// está ociosa esperando teclado.
pub(crate) const WAIT_4_KB_END: u16 = 0xE33F;

/// `APOW_CTR_H` ($7B0A), primer byte del contador de inactividad de 24
/// bits (big-endian, $7B0A-$7B0C) que lleva la ROM para el
/// auto-apagado.
///
/// `WAIT_4_KB` lo inicializa a $FE1D1D al entrar (`WAIT_4_KB_3`, $E25B)
/// y lo incrementa en cada despertar sin tecla pulsada
/// (`WAIT_4_KB_13`, $E2E4); cuando desborda más allá de $FFFFFF salta a
/// `AUTO_OFF` ($E33F), que apaga la máquina. Son $1E2E3 (123619)
/// despertares; medido en el emulador, con el cursor parpadeando en el
/// prompt, ~4.9 por frame (~420 por segundo emulado), es decir unos 5
/// minutos de inactividad a velocidad real.
const APOW_CTR: u32 = 0x7B0A;

/// Valor inicial del contador de auto-apagado, el mismo que escribe la
/// ROM en $E25B-$E266 (`LDI A,$FE` / `LDI A,$1D` ×2).
const APOW_CTR_START: [u8; 3] = [0xFE, 0x1D, 0x1D];

pub struct Pc1500 {
    lh5801: Lh5801,
    lh5810: Lh5810,
    pd1990ac: Pd1990ac,
    memory: MemoryBus,
    keyboard: Keyboard,
    display: DisplayController,
    /// Si es `false`, se impide el auto-apagado de la ROM (ver
    /// [`Pc1500::set_auto_power_off`]).
    auto_power_off: bool,
    /// "Enganche" de la señal `DISP` de la CPU: el último ciclo de CPU
    /// (`Lh5801::get_ticks`) al final de una instrucción con `DISP`
    /// activa, o `None` si no lo ha estado nunca desde [`Pc1500::new`].
    /// Lo actualiza [`Pc1500::run_for_ticks`] tras cada instrucción. Ver
    /// [`Pc1500::display`] para el porqué.
    disp_on_at: Option<usize>,
    /// Ciclo de CPU a partir del cual toca volver a ejecutar los ganchos
    /// por frame (ver `Pc1500::run_frame_hooks`). Se mide en ciclos
    /// emulados, no en llamadas: así da igual en qué trozos se reparta
    /// la emulación (frames enteros de [`Pc1500::step_frame`] o los
    /// presupuestos variables de la GUI con [`Pc1500::run_for_ticks`]).
    next_frame_hook_at: usize,
    /// El alimentador de teclas del host (teclado del PC, ratón, texto
    /// pegado), que pulsa y suelta las teclas en tiempo emulado: ver
    /// `key_feeder.rs` y [`Pc1500::host_key_down`].
    feeder: KeyFeeder,
    /// `CLOAD`/`CSAVE`/`MERGE`/`CHAIN` con archivos del host: carpeta,
    /// cinta, el comando a medias y los eventos para la GUI (ver
    /// `host_io.rs` y [`Pc1500::set_host_dir`]).
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

    /// Ejecuta un "frame": [`TICKS_PER_FRAME`] ciclos de CPU (~11.5 ms de
    /// tiempo real de hardware). Es exactamente
    /// `run_for_ticks(TICKS_PER_FRAME)`; ver [`Pc1500::run_for_ticks`].
    pub fn step_frame(&mut self) {
        self.run_for_ticks(TICKS_PER_FRAME);
    }

    /// Ejecuta instrucciones (con sus periféricos e interrupciones) hasta
    /// gastar al menos `budget` ciclos de CPU y devuelve los que gastó de
    /// verdad: `budget` más lo que se pase la última instrucción, que no
    /// se parte (unos pocos ciclos). `budget = 0` no ejecuta nada.
    ///
    /// Es la forma de avanzar la emulación en trozos de cualquier tamaño:
    /// la GUI lo llama con un presupuesto que sale del tiempo real
    /// transcurrido y del multiplicador de velocidad (unos 433 ciclos por
    /// repintado a 0.02x en un monitor de 60 Hz, millones en modo Máx), y
    /// [`Pc1500::step_frame`] es el caso de un frame entero. Para que
    /// ambas formas se comporten igual, TODO lo que antes se hacía "una
    /// vez por frame" vive aquí dentro y se mide en ciclos emulados, no
    /// en llamadas:
    ///
    /// - El enganche de `DISP` se actualiza tras cada instrucción (ver
    ///   [`Pc1500::display`]).
    /// - Los ganchos por frame (`Pc1500::run_frame_hooks`: el
    ///   alimentador de teclas de [`Pc1500::host_key_down`] y el freno
    ///   del auto-apagado de [`Pc1500::set_auto_power_off`]) se ejecutan
    ///   antes de la primera instrucción que empiece una vez cumplidos
    ///   [`TICKS_PER_FRAME`] ciclos desde la vez anterior. Con
    ///   `step_frame()` eso es justo al empezar cada frame, como antes;
    ///   con trozos más pequeños, cada `TICKS_PER_FRAME` ciclos emulados
    ///   igualmente, sin importar cuántas llamadas haya entre medias.
    ///
    /// Repartir los mismos ciclos en trozos distintos da exactamente la
    /// misma emulación, salvo por lo que haga el que llama entre trozo y
    /// trozo (pulsar teclas, mirar el LCD...).
    pub fn run_for_ticks(&mut self, budget: usize) -> usize {
        let start = self.lh5801.get_ticks();
        let end = start.saturating_add(budget);
        // Ver `Pc1500::halted_states` (avance rápido en `HLT`): nunca
        // saltarse ni el final del presupuesto ni el próximo gancho por
        // frame.
        self.lh5801
            .set_halt_deadline(end.min(self.next_frame_hook_at));

        while self.lh5801.get_ticks() < end {
            if self.lh5801.get_ticks() >= self.next_frame_hook_at {
                self.run_frame_hooks();
                self.lh5801
                    .set_halt_deadline(end.min(self.next_frame_hook_at));
            }

            self.run();

            // Ver `Pc1500::display`: enganchar `DISP` tras cada
            // instrucción para que el breve `RDP`...`SDP` del bucle de
            // teclado no apague el LCD.
            if self.lh5801.display_enabled() {
                self.disp_on_at = Some(self.lh5801.get_ticks());
            }
        }

        self.lh5801.get_ticks() - start
    }

    /// Lo que hay que hacer una vez por frame emulado (cada
    /// [`TICKS_PER_FRAME`] ciclos), llamado desde
    /// [`Pc1500::run_for_ticks`]. Cualquier gancho por frame nuevo va
    /// aquí, no en [`Pc1500::step_frame`], o se lo saltaría la GUI, que
    /// llama a `run_for_ticks` directamente.
    ///
    /// El alimentador de teclas (`Pc1500::feed_keys`) va aquí por eso
    /// mismo: sus pulsaciones se cuentan en frames emulados, sea cual sea
    /// la velocidad y el tamaño de los trozos en que la GUI reparta la
    /// emulación.
    fn run_frame_hooks(&mut self) {
        self.next_frame_hook_at = self.lh5801.get_ticks() + TICKS_PER_FRAME;
        self.feed_keys();
        self.hold_auto_power_off_counter();
    }

    /// Ciclos de CPU (estados del LH5801, [`CPU_HZ`] por segundo de
    /// tiempo real de hardware) ejecutados desde [`Pc1500::new`] o el
    /// último [`Pc1500::reset`]. Nunca retrocede salvo con `reset`.
    #[must_use]
    pub fn ticks(&self) -> usize {
        self.lh5801.get_ticks()
    }

    /// Activa (`true`, lo predeterminado) o desactiva el avance rápido
    /// de la CPU parada en `HLT` (ver `Pc1500::halted_states`).
    ///
    /// Es una optimización sin efecto visible en la emulación: con ella
    /// la ROM ociosa en el prompt va unas 7 veces más rápida en el host.
    /// El interruptor existe para poder comprobarlo (los tests ejecutan
    /// el mismo guion con y sin ella y comparan ciclos, LCD y RAM) y para
    /// descartarla si alguna vez se sospecha de ella.
    pub const fn set_fast_halt(&mut self, enabled: bool) {
        self.lh5801.set_fast_halt(enabled);
    }

    /// `true` si el avance rápido en `HLT` está activado (ver
    /// [`Pc1500::set_fast_halt`]).
    #[must_use]
    pub const fn fast_halt(&self) -> bool {
        self.lh5801.fast_halt()
    }

    /// `true` si la máquina está encendida (flag `BF` del LH5801). Pasa a
    /// `false` cuando la ROM ejecuta `OFF`: con la tecla `OFF` o por el
    /// auto-apagado tras ~5 minutos sin pulsar nada (ver
    /// [`Pc1500::set_auto_power_off`]). Mientras está apagada el LCD está
    /// en blanco, la CPU no ejecuta nada y solo la tecla `ON` la vuelve a
    /// encender.
    #[must_use]
    pub const fn is_powered_on(&self) -> bool {
        self.lh5801.bf()
    }

    /// Activa (`true`, lo predeterminado, como en la máquina real) o
    /// desactiva el auto-apagado de la ROM.
    ///
    /// La PC-1500 se apaga sola tras unos 5 minutos esperando teclado sin
    /// que se pulse nada (más rápido en tiempo de host si el emulador
    /// corre por encima de 1x). La ROM no tiene ninguna opción para
    /// evitarlo, así que, desactivado, el emulador lo impide desde fuera:
    /// una vez por frame emulado (cada [`TICKS_PER_FRAME`] ciclos, ver
    /// [`Pc1500::run_for_ticks`]), si la ROM está en
    /// `WAIT_4_KB` y su contador de inactividad ($7B0A-$7B0C) ya ha
    /// pasado de $FF0000, lo devuelve a su valor inicial $FE1D1D, el
    /// mismo que escribe la propia ROM al entrar en `WAIT_4_KB`. Desde
    /// $FF0000 faltan todavía 65536 despertares (~13000 frames, unos 2.5
    /// minutos emulados) para el desbordamiento, así que no hay forma de que se escape entre dos
    /// frames, y fuera de `WAIT_4_KB` ni se toca (la RAM es de la ROM y
    /// de los programas). También útil para tests largos.
    pub const fn set_auto_power_off(&mut self, enabled: bool) {
        self.auto_power_off = enabled;
    }

    /// `true` si el auto-apagado está activado (ver
    /// [`Pc1500::set_auto_power_off`]).
    #[must_use]
    pub const fn auto_power_off(&self) -> bool {
        self.auto_power_off
    }

    /// Reinicia la máquina como si se le quitaran y volvieran a poner las
    /// pilas principales pero NO la del reloj: todo vuelve al estado de
    /// [`Pc1500::new`] (CPU, RAM con el programa de arranque, LCD,
    /// teclado, LH5810...) y la ROM arranca en frío (`NEW0? :CHECK`).
    ///
    /// # Qué sobrevive al reset
    /// Esta es la lista completa; cualquier estado nuevo que deba
    /// conservarse tiene que añadirse aquí explícitamente:
    ///
    /// - El reloj `µPD1990AC` entero (hora, una hora puesta con `TIME=`, su
    ///   base de tiempo de [`Pc1500::set_rtc_time_base`]), que en la
    ///   máquina real tiene su propia pila de respaldo. La rutina `RESET`
    ///   de la ROM solo le manda el comando 0 ($E0FF-$E101), que no toca
    ///   la hora.
    /// - La opción de auto-apagado ([`Pc1500::set_auto_power_off`]), que
    ///   es una opción del emulador, no estado de la máquina.
    /// - La opción de avance rápido en `HLT` ([`Pc1500::set_fast_halt`]),
    ///   igual.
    /// - La configuración de `CLOAD`/`CSAVE` con archivos del host: la
    ///   carpeta ([`Pc1500::set_host_dir`]) y el archivo de cinta
    ///   ([`Pc1500::set_tape`]), y los eventos que la GUI aún no haya
    ///   recogido ([`Pc1500::take_host_events`]). Un comando a medias no.
    ///
    /// Lo que NO sobrevive, a propósito: el alimentador de teclas (teclas
    /// del host pulsadas y texto pegado pendiente, ver
    /// [`Pc1500::host_key_down`]) empieza vacío, igual que la matriz de
    /// teclado. Una tecla del host que siguiera pulsada durante el reset
    /// no se vuelve a pulsar sola: su `host_key_up` posterior no hace
    /// nada.
    ///
    /// [`Pc1500::ticks`] vuelve a 0.
    pub fn reset(&mut self) {
        let fresh = if self.is_pc1500a() { Self::new_pc1500a() } else { Self::new() };
        let old = mem::replace(self, fresh);
        self.set_fast_halt(old.fast_halt());
        self.pd1990ac = old.pd1990ac;
        // La CPU nueva empieza con `timer_state` a 0.
        self.pd1990ac.rebase_timer(self.lh5801.timer_state());
        self.auto_power_off = old.auto_power_off;
        self.host_io.keep_config_from(old.host_io);
    }

    /// Cambia la base de tiempo del reloj `µPD1990AC` (ver [`RtcTimeBase`])
    /// por un chip nuevo que empieza en esa hora. [`Pc1500::new`] usa
    /// [`RtcTimeBase::HostLocal`] (la hora local del host, lo que quiere
    /// la GUI); los tests usan [`RtcTimeBase::Emulated`] para ser
    /// deterministas (ver [`Pc1500::new_deterministic`]).
    ///
    /// Pensado para llamarse justo después de [`Pc1500::new`], antes de
    /// emular nada: el chip nuevo pierde el estado de sus patillas y del
    /// comando memorizado (la ROM vuelve a mandarlos en cada uso, así que
    /// en la práctica da igual, pero no es lo que pasaría en la máquina
    /// real).
    pub fn set_rtc_time_base(&mut self, time_base: RtcTimeBase) {
        self.pd1990ac = Pd1990ac::with_time_base(time_base);
        self.pd1990ac.rebase_timer(self.lh5801.timer_state());
    }

    /// Fecha y hora actuales del reloj `µPD1990AC` (la que leería `TIME`).
    pub fn rtc_now(&mut self) -> RtcDateTime {
        self.pd1990ac.sync(self.lh5801.timer_state());
        self.pd1990ac.now()
    }

    /// `true` si el PC está dentro de la rutina `WAIT_4_KB` de la ROM
    /// ($E243-$E33E), dormida o no en su `HLT`.
    pub(crate) const fn cpu_in_wait_4_kb(&self) -> bool {
        let p = self.lh5801.p();
        p >= WAIT_4_KB_START && p < WAIT_4_KB_END
    }

    /// Ver [`Pc1500::set_auto_power_off`].
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

    /// El LCD tal como se ve ahora mismo (ver [`DisplayController`]).
    ///
    /// El LCD se ve encendido si la máquina está encendida (`BF`) y la
    /// señal `DISP` de la CPU (`SDP`/`RDP`) lo está ahora o lo estuvo en
    /// algún momento de los últimos [`LCD_PERSISTENCE_TICKS`] ciclos
    /// emulados: `lcd_on = bf && (enganche || disp)`.
    ///
    /// # Por qué un enganche y no el valor instantáneo de `DISP`
    /// La ROM apaga el LCD un instante a propósito: en el bucle ocioso de
    /// teclado (`WAIT_4_KB_17`, $E30D `RDP` / `LDI UL,$BC` / `LOP UL` /
    /// $E313 `SDP`, unos 1.6 ms, 2090 ciclos) y en `RESET` ($E006 `RDP`, $E00C `SDP`,
    /// $E012 `RDP`). En la máquina real eso no se ve: el cristal líquido
    /// tarda bastante más en reaccionar. Antes se miraba solo el valor de
    /// `DISP` en el instante de llamar a `display()` (entre dos frames), y
    /// si ese instante caía dentro del pulso la GUI pintaba un frame
    /// entero con el LCD en blanco: un parpadeo que la máquina real no
    /// tiene. Ahora [`Pc1500::run_for_ticks`] apunta tras cada
    /// instrucción el último ciclo con `DISP` activa, así que basta con
    /// que lo haya estado una sola vez en la ventana.
    ///
    /// # Por qué una ventana en ciclos emulados
    /// La primera versión del enganche cubría el intervalo entre dos
    /// llamadas a `display()`, que la GUI hace una vez por repintado. Eso
    /// valía mientras cada repintado emulaba al menos un frame entero,
    /// pero con el planificador de tiempo real (ver
    /// [`Pc1500::run_for_ticks`]) a 0.02x cada repintado emula solo unos
    /// 433 ciclos: ventanas mucho más cortas que el propio pulso `RDP`
    /// (medido en el prompt: 2090 ciclos, cada ~6.3 millones, unos 4.8 s
    /// emulados), así que a cámara lenta el LCD se habría apagado unos
    /// cuantos repintados seguidos en cada pulso: el parpadeo de antes,
    /// a cámara lenta. Medida en ciclos emulados la persistencia no depende de
    /// cada cuánto mire nadie el LCD ni de la velocidad de emulación: es
    /// la misma a 0.02x, a 1x y en modo Máx, y en los tests.
    ///
    /// Llamar a `display()` dos veces seguidas sin emular nada entre
    /// medias da el mismo resultado.
    ///
    /// Con la máquina apagada (`OFF` o auto-apagado, ver
    /// [`Pc1500::is_powered_on`]) el LCD entero, indicadores incluidos, se
    /// ve en blanco aunque la RAM de pantalla conserve lo que había (la
    /// ROM lo vuelve a pintar al reanudar).
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
        // µPD1990AC (ver `pd1990ac.rs`): primero se pone al día su reloj
        // emulado con los ciclos de la instrucción que acaba de ejecutar
        // la CPU; luego, si esa instrucción escribió el puerto C del
        // LH5810 (PC0 = DATA IN, PC1 = STB, PC2 = CLK, PC3 = C0 y OUT
        // ENBL, PC4 = C1, PC5 = C2), se le pasa el byte entero, que es lo
        // que necesita para detectar los flancos de STB y CLK. Una
        // escritura por instrucción basta (ver `Pd1990ac::write_port`).
        self.pd1990ac.sync(self.lh5801.timer_state());
        if self.lh5810.new_opc() {
            let opc = self.lh5810.get_reg(lh5810::Reg::OPC);
            self.pd1990ac.write_port(opc);
            self.lh5810.set_new_opc(false);
        }

        // Salidas del µPD1990AC: TP → PB5 (la base de tiempos de
        // `WAIT`/`PAUSE`/`BEEP`) y DATA OUT → PB6 (los bits de `TIME`).
        self.lh5810
            .set_reg_bit(lh5810::Reg::OPB, 5, self.pd1990ac.tp());
        self.lh5810
            .set_reg_bit(lh5810::Reg::OPB, 6, self.pd1990ac.data_out());

        // ON/BRK → PB7, como un pulso de un solo `run()` por pulsación
        // (ver `Keyboard::on_pulse`): basta, porque `Lh5810::step` lo
        // engancha en el bit 1 de `IF` y ahí se queda hasta que la ROM lo
        // borra al atender el BREAK. Con la máquina apagada la misma
        // pulsación la enciende y NO genera además un BREAK, igual que en
        // la real: la rutina `RESET` pone `IF` a 0 ($E028) de todas
        // formas.
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
        // Y si estaba apagada (`OFF` o auto-apagado), encenderla sin
        // pasar por el reset de la ROM, que machacaría el PC recién
        // fijado: con `BF` a 0 `step_cpu()` no ejecuta nada.
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
