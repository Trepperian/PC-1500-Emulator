//! Alimentador de teclas: convierte lo que hace el teclado del host (o el
//! ratón sobre las teclas de la pantalla, o un texto pegado) en
//! pulsaciones y sueltas de la PC-1500 medidas en TIEMPO EMULADO.
//!
//! # Por qué hace falta
//! La ROM no ve "eventos de tecla", ve la matriz pulsada o no en cada
//! despertar de su bucle de espera (`WAIT_4_KB`, $E243) y tiene sus
//! propios antirrebotes: una tecla tiene que estar pulsada varios frames
//! y luego el teclado vacío otros tantos antes de la siguiente (ver
//! [`KEY_HOLD_FRAMES`] y [`KEY_RELEASE_FRAMES`], medidos con el arnés).
//! Antes la GUI pulsaba y soltaba las teclas con un temporizador de 150
//! ms de tiempo de HOST: a 0.05x eso son ~0.65 frames emulados y la ROM
//! perdía las teclas; a 50x, decenas de frames. Aquí todo se cuenta en
//! frames emulados ([`crate::TICKS_PER_FRAME`] ciclos), así que una
//! pulsación dura lo mismo para la ROM a cualquier velocidad, en pausa
//! (no avanza) o en los tests.
//!
//! # Cómo se mide el tiempo
//! [`KeyFeeder::tick`] avanza la máquina de estados un paso por frame
//! emulado. Lo llama `Pc1500::run_frame_hooks` desde dentro de
//! [`Pc1500::run_for_ticks`] cada vez que se cumplen
//! [`crate::TICKS_PER_FRAME`] ciclos, NO una vez por llamada: la GUI
//! llama a `run_for_ticks` con presupuestos de cualquier tamaño (~433
//! ciclos por repintado a 0.02x, millones en modo Máx) y el ritmo de
//! las teclas no debe depender de eso.
//!
//! # Dos clases de pasos
//! - [`StepKind::Live`]: una tecla física del host (o del ratón). Empieza
//!   en cuanto le toca, sin esperar a la ROM, como un dedo de verdad, y
//!   se mantiene pulsada MIENTRAS el host la tenga pulsada (con un mínimo
//!   de [`KEY_HOLD_FRAMES`]): así funcionan la autorrepetición propia de
//!   la ROM en las flechas y los juegos con `INKEY$`. Si llega otra
//!   tecla viva mientras tanto, la nueva gana ("rollover", como en los
//!   teclados de PC).
//! - [`StepKind::Typed`]: un carácter de un texto pegado
//!   ([`Pc1500::queue_text`]). Pulsación de duración mínima, y antes de
//!   cada una se espera a que la ROM esté lista para la siguiente tecla
//!   ([`Pc1500::is_ready_for_key`]) durante 2 frames seguidos, o como
//!   mucho [`TYPED_IDLE_TIMEOUT_FRAMES`]: así un listado pegado
//!   sobrevive a lo que tarda la ROM en procesar cada ENTER.
//!
//! `ON/BRK` no pasa por la cola: no está en la matriz (va a PB7, ver
//! `Keyboard::on_pulse`) y es justo la tecla con la que se interrumpe lo
//! que esté pasando, así que se pulsa en el acto y además cancela el
//! texto pegado pendiente.
//!
//! El alimentador solo toca las teclas que pulsa él: lo que se pulse con
//! [`Pc1500::press`]/[`Pc1500::release`] (p. ej. el `tap` síncrono del
//! arnés) no lo ve ni lo suelta. No mezclar las dos cosas a la vez.

use std::collections::VecDeque;

use crate::{
    Key, Pc1500,
    harness::{KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES},
    keyboard::{KeyStroke, Keyboard, key_for_char},
};

/// Identificador de una tecla física del host.
///
/// Lo elige la GUI: p. ej. el código de la tecla física, o uno reservado
/// para el ratón. Sirve para
/// emparejar la suelta con su pulsación aunque entre medias cambien los
/// modificadores o la tecla lógica.
pub type HostKeyId = u32;

/// Máximo de frames emulados que un carácter pegado espera a la ROM.
///
/// Son ~4.6 s a 1x: lo que espera cada carácter de
/// [`Pc1500::queue_text`] a que la ROM esté lista para teclado. Pasado ese tiempo se teclea de todas formas: así un programa en marcha
/// que lee el teclado con `INKEY$` (y nunca se para en `WAIT_4_KB`)
/// también recibe el texto, aunque despacio.
pub const TYPED_IDLE_TIMEOUT_FRAMES: usize = 400;

/// Frames emulados mínimos que se mantiene pulsada `ON/BRK`.
///
/// La ROM solo necesita el flanco (ver `Keyboard::on_pulse`), así que
/// basta con poco.
pub const ON_MIN_HOLD_FRAMES: usize = 2;

/// Bit `SHIFT` del byte de indicadores $764E (ver
/// [`Pc1500::lcd_symbol_bytes`]): el prefijo `SHIFT` está "enganchado".
const SYMBOL_SHIFT: u8 = 0x02;
/// Bit `SML` (minúsculas) del byte de indicadores $764E.
const SYMBOL_SML: u8 = 0x08;
/// Dirección del byte de indicadores con `SHIFT`/`SML`/`DEF`.
const SYMBOLS_ADDR: u32 = 0x764E;

/// Cómo decide el alimentador si hace falta pulsar `SHIFT` (como prefijo,
/// ver [`KeyStroke`]) antes de la tecla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftPolicy {
    /// Lo de una persona con el teclado del host: pulsar `SHIFT` solo si
    /// la pulsación lo lleva y el indicador `SHIFT` no está ya
    /// enganchado. Un `SHIFT` que el usuario enganchó a propósito (F11 o
    /// la tecla de la pantalla) sigue valiendo para la siguiente tecla, y
    /// en modo `SML` las letras salen como saldrían en la máquina real.
    AddIfNeeded,
    /// Teclear EXACTAMENTE el carácter pedido (texto pegado, o letras con
    /// "distinguir mayúsculas/minúsculas"): se tiene en cuenta el modo
    /// `SML` (que invierte mayúsculas/minúsculas en las letras) y un
    /// `SHIFT` enganchado que sobre se quita pulsándolo otra vez.
    Exact,
}

/// Una tecla en la cola del alimentador.
#[derive(Clone, Copy, Debug)]
struct Step {
    kind: StepKind,
    stroke: KeyStroke,
    policy: ShiftPolicy,
}

#[derive(Clone, Copy, Debug)]
enum StepKind {
    /// Tecla física del host (`id`, o `None` para pulsaciones sueltas de
    /// [`Pc1500::host_tap`]): se mantiene hasta que el host la suelte
    /// (`host_released`).
    Live {
        id: Option<HostKeyId>,
        host_released: bool,
    },
    /// Carácter de un texto pegado: pulsación mínima, esperando antes a
    /// que la ROM esté lista.
    Typed,
}

/// En qué punto de la pulsación del primer paso de la cola está el
/// alimentador. El número es cuántos frames lleva en esa fase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Nada pulsado; el primer paso de la cola (si lo hay) todavía no ha
    /// empezado.
    Idle,
    /// `SHIFT` (prefijo) pulsado.
    ShiftDown(usize),
    /// `SHIFT` soltado, esperando a que la ROM vea el teclado vacío.
    ShiftGap(usize),
    /// La tecla del primer paso de la cola, pulsada.
    Down(usize),
    /// La tecla ya se soltó y su paso salió de la cola; esperando a que la
    /// ROM vea el teclado vacío antes del siguiente.
    Gap(usize),
}

/// `ON/BRK` pulsada desde el host.
#[derive(Clone, Copy, Debug)]
struct OnHold {
    id: HostKeyId,
    frames: usize,
    host_released: bool,
}

/// Lo que el alimentador necesita saber de la máquina en cada frame.
#[derive(Clone, Copy, Debug)]
pub struct FeedEnv {
    /// El byte de indicadores $764E (bits `SHIFT` y `SML`).
    pub symbols: u8,
    /// [`Pc1500::is_ready_for_key`].
    pub rom_ready: bool,
    /// [`Pc1500::is_powered_on`].
    pub powered_on: bool,
}

/// La máquina de estados del alimentador. Ver el comentario del módulo.
#[derive(Debug)]
pub struct KeyFeeder {
    queue: VecDeque<Step>,
    phase: Phase,
    on: Option<OnHold>,
    /// Frames seguidos (contados en cada [`KeyFeeder::tick`]) con la ROM
    /// lista para teclado.
    ready_streak: usize,
    /// Frames que lleva esperando a la ROM el carácter pegado del frente
    /// de la cola.
    waited: usize,
}

impl KeyFeeder {
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            phase: Phase::Idle,
            on: None,
            ready_streak: 0,
            waited: 0,
        }
    }

    /// `true` si el primer paso de la cola ya ha empezado (tiene `SHIFT`
    /// o su tecla pulsados).
    const fn front_active(&self) -> bool {
        matches!(
            self.phase,
            Phase::ShiftDown(_) | Phase::ShiftGap(_) | Phase::Down(_)
        )
    }

    /// Ver [`Pc1500::host_key_down_with`].
    pub fn key_down(
        &mut self,
        kb: &mut Keyboard,
        id: HostKeyId,
        stroke: KeyStroke,
        policy: ShiftPolicy,
    ) {
        if stroke.key == Key::On {
            self.on_down(kb, id, false);
            return;
        }
        let already_down = self.queue.iter().any(|step| {
            matches!(step.kind, StepKind::Live { id: Some(other), host_released: false } if other == id)
        });
        if !already_down {
            self.queue.push_back(Step {
                kind: StepKind::Live { id: Some(id), host_released: false },
                stroke,
                policy,
            });
        }
    }

    /// Ver [`Pc1500::host_key_up`].
    pub fn key_up(&mut self, id: HostKeyId) {
        if let Some(on) = &mut self.on
            && on.id == id
        {
            on.host_released = true;
        }
        for step in &mut self.queue {
            if let StepKind::Live { id: Some(other), host_released } = &mut step.kind
                && *other == id
            {
                *host_released = true;
            }
        }
    }

    /// Ver [`Pc1500::host_tap_with`].
    pub fn tap(&mut self, kb: &mut Keyboard, strokes: &[KeyStroke], policy: ShiftPolicy) {
        for &stroke in strokes {
            if stroke.key == Key::On {
                self.on_down(kb, TAP_ON_ID, true);
            } else {
                self.queue.push_back(Step {
                    kind: StepKind::Live { id: None, host_released: true },
                    stroke,
                    policy,
                });
            }
        }
    }

    /// Ver [`Pc1500::queue_text`].
    pub fn queue_text(&mut self, text: &str) -> Vec<char> {
        let mut skipped = Vec::new();
        for c in text.chars().filter(|&c| c != '\r') {
            match key_for_char(c) {
                Some(stroke) => self.queue.push_back(Step {
                    kind: StepKind::Typed,
                    stroke,
                    policy: ShiftPolicy::Exact,
                }),
                None => skipped.push(c),
            }
        }
        skipped
    }

    /// `ON/BRK` pulsada: en el acto, sin cola (ver el comentario del
    /// módulo), y cancelando el texto pegado pendiente.
    fn on_down(&mut self, kb: &mut Keyboard, id: HostKeyId, host_released: bool) {
        // Otra tecla del host que también es ON/BRK (Esc y F12, p. ej.)
        // mientras `ON` sigue pulsada (por la primera, o aún dentro de su
        // mínimo de frames): manda la última que se pulsó, así que `ON` se
        // suelta cuando se suelte esa, aunque la anterior siga pulsada (la
        // suelta de la anterior ya no cuenta). Tampoco hay pulso nuevo: la
        // ROM solo ve el flanco de subida de `ON` (`on_pulse`), así que dos
        // BREAK más rápidos que `ON_MIN_HOLD_FRAMES` cuentan como uno, que
        // para un BREAK da igual.
        if let Some(on) = &mut self.on {
            on.id = id;
            on.host_released = host_released;
        } else {
            kb.press(Key::On);
            self.on = Some(OnHold { id, frames: 0, host_released });
        }
        self.cancel_typing(kb);
    }

    /// Ver [`Pc1500::cancel_typing`].
    pub fn cancel_typing(&mut self, kb: &mut Keyboard) {
        let active_typed = self.front_active()
            && matches!(self.queue.front(), Some(Step { kind: StepKind::Typed, .. }));
        if active_typed {
            self.abort_front(kb);
        }
        self.queue.retain(|step| !matches!(step.kind, StepKind::Typed));
        // La espera a la ROM es la del carácter pegado del frente, que ya
        // no está: que el siguiente texto pegado empiece a contar de cero.
        // Si el frente es ahora una tecla viva, `waited` no le afecta.
        self.waited = 0;
    }

    /// Suelta lo que tenga pulsado el primer paso de la cola (ya
    /// empezado), lo saca de la cola y pasa a [`Phase::Gap`].
    fn abort_front(&mut self, kb: &mut Keyboard) {
        match self.phase {
            Phase::ShiftDown(_) => kb.release(Key::Shift),
            Phase::Down(_) => {
                if let Some(step) = self.queue.front() {
                    kb.release(step.stroke.key);
                }
            }
            Phase::Idle | Phase::ShiftGap(_) | Phase::Gap(_) => {}
        }
        self.queue.pop_front();
        self.phase = Phase::Gap(0);
    }

    /// Ver [`Pc1500::release_all_host_keys`].
    pub fn release_all(&mut self) {
        if let Some(on) = &mut self.on {
            on.host_released = true;
        }
        for step in &mut self.queue {
            if let StepKind::Live { host_released, .. } = &mut step.kind {
                *host_released = true;
            }
        }
    }

    /// Pasos en la cola, incluido el que se está tecleando.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Un frame emulado. Ver el comentario del módulo.
    pub fn tick(&mut self, kb: &mut Keyboard, env: FeedEnv) {
        self.ready_streak = if env.rom_ready { self.ready_streak + 1 } else { 0 };

        if let Some(on) = &mut self.on {
            on.frames += 1;
            if on.host_released && on.frames >= ON_MIN_HOLD_FRAMES {
                kb.release(Key::On);
                self.on = None;
            }
        }

        // Con la máquina apagada no hay nadie leyendo el teclado: el texto
        // pegado pendiente se descarta en vez de esperar al tiempo límite
        // tecla a tecla.
        if !env.powered_on {
            self.cancel_typing(kb);
        }

        match self.phase {
            Phase::Idle => self.start_front(kb, env),
            Phase::ShiftDown(frames) => {
                self.phase = if frames + 1 >= KEY_HOLD_FRAMES {
                    kb.release(Key::Shift);
                    Phase::ShiftGap(0)
                } else {
                    Phase::ShiftDown(frames + 1)
                };
            }
            Phase::ShiftGap(frames) => {
                if frames + 1 >= KEY_RELEASE_FRAMES {
                    if let Some(step) = self.queue.front() {
                        kb.press(step.stroke.key);
                    }
                    self.phase = Phase::Down(0);
                } else {
                    self.phase = Phase::ShiftGap(frames + 1);
                }
            }
            Phase::Down(frames) => {
                let frames = frames + 1;
                if frames >= KEY_HOLD_FRAMES && self.front_may_release() {
                    self.abort_front(kb);
                } else {
                    self.phase = Phase::Down(frames);
                }
            }
            Phase::Gap(frames) => {
                if frames + 1 >= KEY_RELEASE_FRAMES {
                    self.phase = Phase::Idle;
                    self.start_front(kb, env);
                } else {
                    self.phase = Phase::Gap(frames + 1);
                }
            }
        }
    }

    /// `true` si la tecla del primer paso (ya pulsada el mínimo) se puede
    /// soltar: siempre si es un carácter pegado; si es una tecla viva,
    /// cuando el host la ha soltado o cuando hay otra tecla viva más
    /// nueva esperando (rollover).
    fn front_may_release(&self) -> bool {
        let Some(front) = self.queue.front() else {
            return true;
        };
        match front.kind {
            StepKind::Typed => true,
            StepKind::Live { host_released, .. } => {
                host_released
                    || self
                        .queue
                        .iter()
                        .skip(1)
                        .any(|step| matches!(step.kind, StepKind::Live { .. }))
            }
        }
    }

    /// Empieza el primer paso de la cola, si lo hay y le toca: pulsa
    /// `SHIFT` (prefijo) si hace falta según su [`ShiftPolicy`], o la
    /// tecla directamente.
    fn start_front(&mut self, kb: &mut Keyboard, env: FeedEnv) {
        let Some(&step) = self.queue.front() else {
            self.waited = 0;
            return;
        };
        if matches!(step.kind, StepKind::Typed)
            && self.ready_streak < 2
            && self.waited < TYPED_IDLE_TIMEOUT_FRAMES
        {
            self.waited += 1;
            return;
        }
        self.waited = 0;

        let latched = env.symbols & SYMBOL_SHIFT != 0;
        let sml = env.symbols & SYMBOL_SML != 0;
        let stroke = step.stroke;
        let tap_shift = stroke.key != Key::Shift
            && match step.policy {
                ShiftPolicy::AddIfNeeded => stroke.shift && !latched,
                ShiftPolicy::Exact => (stroke.shift ^ (stroke.key.is_letter() && sml)) != latched,
            };
        if tap_shift {
            kb.press(Key::Shift);
            self.phase = Phase::ShiftDown(0);
        } else {
            kb.press(stroke.key);
            self.phase = Phase::Down(0);
        }
    }
}

/// Identificador que usa [`Pc1500::host_tap`] para un `ON/BRK` suelto
/// (pulsado y soltado a la vez).
const TAP_ON_ID: HostKeyId = HostKeyId::MAX;

impl Pc1500 {
    /// Gancho por frame (ver `Pc1500::run_frame_hooks`): un paso del
    /// alimentador de teclas.
    pub(crate) fn feed_keys(&mut self) {
        let env = FeedEnv {
            symbols: self.read_byte(SYMBOLS_ADDR),
            rom_ready: self.is_ready_for_key(),
            powered_on: self.is_powered_on(),
        };
        self.feeder.tick(&mut self.keyboard, env);
    }

    /// Una tecla física del host (identificada por `id`) se acaba de
    /// pulsar y genera `stroke`: se teclea en cuanto le toque y se
    /// mantiene pulsada en la PC-1500 hasta [`Pc1500::host_key_up`] (como
    /// mínimo [`KEY_HOLD_FRAMES`] frames emulados, por breve que haya
    /// sido en el host). Con [`ShiftPolicy::AddIfNeeded`]; ver
    /// [`Pc1500::host_key_down_with`].
    pub fn host_key_down(&mut self, id: HostKeyId, stroke: KeyStroke) {
        self.host_key_down_with(id, stroke, ShiftPolicy::AddIfNeeded);
    }

    /// Como [`Pc1500::host_key_down`], eligiendo cómo se decide el
    /// `SHIFT` (ver [`ShiftPolicy`]).
    ///
    /// Una segunda pulsación del mismo `id` sin haberlo soltado (la
    /// autorrepetición del sistema operativo) se ignora: la ROM ya hace
    /// su propia autorrepetición. `ON/BRK` ([`Key::On`]) no espera turno:
    /// se pulsa en el acto y cancela el texto pegado pendiente (ver
    /// [`Pc1500::cancel_typing`]).
    pub fn host_key_down_with(&mut self, id: HostKeyId, stroke: KeyStroke, policy: ShiftPolicy) {
        self.feeder.key_down(&mut self.keyboard, id, stroke, policy);
    }

    /// La tecla física `id` se ha soltado en el host: su tecla de la
    /// PC-1500 se suelta en cuanto haya cumplido el mínimo. Si no estaba
    /// pulsada, no hace nada.
    pub fn host_key_up(&mut self, id: HostKeyId) {
        self.feeder.key_up(id);
    }

    /// Pulsaciones sueltas de duración fija en tiempo emulado (pulsar y
    /// soltar), en orden, como teclas vivas ya soltadas: p. ej. la tecla
    /// Retroceso del host (← y luego `DEL`) o, a más de 1x, cada pulsación
    /// del host. Con [`ShiftPolicy::AddIfNeeded`].
    pub fn host_tap(&mut self, strokes: &[KeyStroke]) {
        self.host_tap_with(strokes, ShiftPolicy::AddIfNeeded);
    }

    /// Como [`Pc1500::host_tap`], eligiendo cómo se decide el `SHIFT`.
    pub fn host_tap_with(&mut self, strokes: &[KeyStroke], policy: ShiftPolicy) {
        self.feeder.tap(&mut self.keyboard, strokes, policy);
    }

    /// Teclea `text` (p. ej. pegado del portapapeles) carácter a
    /// carácter, exactamente ([`ShiftPolicy::Exact`]: respeta
    /// mayúsculas/minúsculas también con `SML`), esperando antes de cada
    /// tecla a que la ROM esté lista (ver el comentario del módulo
    /// `key_feeder`). `\r` se ignora y `\n` es ENTER.
    ///
    /// Devuelve los caracteres que no se pueden teclear (sin tecla en la
    /// PC-1500, ver [`key_for_char`]), que se saltan; el resto se teclea
    /// igualmente.
    pub fn queue_text(&mut self, text: &str) -> Vec<char> {
        self.feeder.queue_text(text)
    }

    /// Descarta el texto pegado pendiente (y suelta la tecla que se
    /// estuviera tecleando de él). Las teclas vivas del host no se tocan.
    pub fn cancel_typing(&mut self) {
        self.feeder.cancel_typing(&mut self.keyboard);
    }

    /// Da por soltadas todas las teclas del host (al perder el foco la
    /// ventana, o al dejar de capturar el teclado): cada una se suelta en
    /// cuanto cumpla su mínimo, así que ninguna se queda pegada ni se
    /// pierde. El texto pegado pendiente sigue.
    pub fn release_all_host_keys(&mut self) {
        self.feeder.release_all();
    }

    /// Pasos pendientes en el alimentador (teclas vivas y caracteres
    /// pegados), incluido el que se está tecleando ahora.
    #[must_use]
    pub fn pending_keystrokes(&self) -> usize {
        self.feeder.pending()
    }

    /// `true` si `key` está pulsada ahora mismo en la matriz emulada (por
    /// el alimentador o por [`Pc1500::press`]). Para pintar las teclas.
    #[must_use]
    pub const fn is_key_pressed(&self, key: Key) -> bool {
        self.keyboard.is_pressed(key)
    }
}

#[cfg(test)]
mod tests {
    use super::{FeedEnv, KeyFeeder, ShiftPolicy, TYPED_IDLE_TIMEOUT_FRAMES};
    use crate::{
        Key,
        harness::{KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES},
        keyboard::{KeyStroke, Keyboard},
    };

    const READY: FeedEnv = FeedEnv { symbols: 0, rom_ready: true, powered_on: true };
    const BUSY: FeedEnv = FeedEnv { symbols: 0, rom_ready: false, powered_on: true };

    /// Ejecuta `frames` ticks y devuelve, para cada uno, las teclas
    /// pulsadas después del tick.
    fn run(feeder: &mut KeyFeeder, kb: &mut Keyboard, env: FeedEnv, frames: usize) -> Vec<Vec<Key>> {
        (0..frames)
            .map(|_| {
                feeder.tick(kb, env);
                Key::ALL.into_iter().filter(|&k| kb.is_pressed(k)).collect()
            })
            .collect()
    }

    #[test]
    fn short_live_press_gets_the_minimum_hold() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.key_down(&mut kb, 1, KeyStroke::plain(Key::A), ShiftPolicy::AddIfNeeded);
        feeder.key_up(1);
        let frames = run(&mut feeder, &mut kb, BUSY, KEY_HOLD_FRAMES + KEY_RELEASE_FRAMES);
        let held = frames.iter().filter(|keys| keys == &&vec![Key::A]).count();
        assert_eq!(held, KEY_HOLD_FRAMES, "{frames:?}");
        assert_eq!(feeder.pending(), 0);
    }

    #[test]
    fn live_key_is_held_while_the_host_holds_it() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.key_down(&mut kb, 1, KeyStroke::plain(Key::Left), ShiftPolicy::AddIfNeeded);
        run(&mut feeder, &mut kb, BUSY, 50);
        assert!(kb.is_pressed(Key::Left), "sigue pulsada");
        // Repetición del SO: ignorada.
        feeder.key_down(&mut kb, 1, KeyStroke::plain(Key::Left), ShiftPolicy::AddIfNeeded);
        assert_eq!(feeder.pending(), 1);
        feeder.key_up(1);
        run(&mut feeder, &mut kb, BUSY, 1);
        assert!(!kb.is_pressed(Key::Left), "suelta en el siguiente frame");
    }

    #[test]
    fn shifted_live_key_taps_shift_first_unless_latched() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.tap(&mut kb, &[KeyStroke::shifted(Key::LeftParen)], ShiftPolicy::AddIfNeeded);
        let frames = run(&mut feeder, &mut kb, BUSY, 2 * (KEY_HOLD_FRAMES + KEY_RELEASE_FRAMES));
        assert_eq!(frames[0], vec![Key::Shift]);
        assert!(frames.iter().all(|keys| keys.len() <= 1), "nunca dos a la vez");
        assert!(frames.iter().any(|keys| keys == &vec![Key::LeftParen]));

        // Con SHIFT ya enganchado no se vuelve a pulsar.
        let latched = FeedEnv { symbols: 0x02, ..BUSY };
        feeder.tap(&mut kb, &[KeyStroke::shifted(Key::LeftParen)], ShiftPolicy::AddIfNeeded);
        let after_latch = run(&mut feeder, &mut kb, latched, 1);
        assert_eq!(after_latch[0], vec![Key::LeftParen]);
    }

    #[test]
    fn exact_policy_follows_sml_and_clears_a_stale_latch() {
        let mut kb = Keyboard::new();
        let sml = FeedEnv { symbols: 0x08, ..READY };
        let cases = [
            // (texto, entorno, ¿SHIFT primero?)
            ('A', READY, false),
            ('a', READY, true),
            ('A', sml, true),
            ('a', sml, false),
            ('A', FeedEnv { symbols: 0x02, ..READY }, true),
            ('(', FeedEnv { symbols: 0x02, ..READY }, true),
            ('(', sml, false),
        ];
        for (c, env, shift_first) in cases {
            let mut feeder = KeyFeeder::new();
            assert!(feeder.queue_text(&c.to_string()).is_empty());
            let frames = run(&mut feeder, &mut kb, env, 2);
            let first = frames.iter().find(|keys| !keys.is_empty()).cloned();
            assert_eq!(
                first.as_deref() == Some(&[Key::Shift][..]),
                shift_first,
                "{c:?} con $764E={:#04X}: {frames:?}",
                env.symbols
            );
            run(&mut feeder, &mut kb, env, 20);
            assert_eq!(feeder.pending(), 0);
        }
    }

    #[test]
    fn typed_steps_wait_for_the_rom_until_the_timeout() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.queue_text("A");
        let frames = run(&mut feeder, &mut kb, BUSY, TYPED_IDLE_TIMEOUT_FRAMES);
        assert!(frames.iter().all(Vec::is_empty), "espera a la ROM");
        run(&mut feeder, &mut kb, BUSY, 1);
        assert!(kb.is_pressed(Key::A), "pasado el l\u{ed}mite, se teclea");

        let mut ready_feeder = KeyFeeder::new();
        let mut ready_kb = Keyboard::new();
        ready_feeder.queue_text("A");
        let ready_frames = run(&mut ready_feeder, &mut ready_kb, READY, 2);
        assert_eq!(ready_frames, vec![vec![], vec![Key::A]], "2 frames lista");
    }

    #[test]
    fn rollover_releases_the_older_key_after_the_minimum() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.key_down(&mut kb, 1, KeyStroke::plain(Key::H), ShiftPolicy::AddIfNeeded);
        run(&mut feeder, &mut kb, BUSY, 1);
        feeder.key_down(&mut kb, 2, KeyStroke::plain(Key::I), ShiftPolicy::AddIfNeeded);
        let frames = run(&mut feeder, &mut kb, BUSY, 30);
        assert!(kb.is_pressed(Key::I), "la nueva queda mantenida");
        assert!(!kb.is_pressed(Key::H));
        assert!(frames.iter().all(|keys| keys.len() <= 1));
        feeder.key_up(1); // la vieja ya no estaba: no pasa nada
        feeder.key_up(2);
        run(&mut feeder, &mut kb, BUSY, 1);
        assert!(!kb.is_pressed(Key::I));
    }

    #[test]
    fn on_is_immediate_and_cancels_typing() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.queue_text("HELLO");
        run(&mut feeder, &mut kb, READY, 3);
        assert!(kb.is_pressed(Key::H));
        feeder.key_down(&mut kb, 9, KeyStroke::plain(Key::On), ShiftPolicy::AddIfNeeded);
        assert!(kb.is_pressed(Key::On), "en el acto");
        assert!(kb.take_on_pulse());
        assert!(!kb.is_pressed(Key::H), "la tecla pegada se suelta");
        assert_eq!(feeder.pending(), 0, "texto cancelado");
        feeder.key_up(9);
        run(&mut feeder, &mut kb, READY, 1);
        assert!(kb.is_pressed(Key::On), "m\u{ed}nimo de 2 frames");
        run(&mut feeder, &mut kb, READY, 1);
        assert!(!kb.is_pressed(Key::On));
    }

    #[test]
    fn release_all_lets_everything_finish() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.key_down(&mut kb, 1, KeyStroke::plain(Key::A), ShiftPolicy::AddIfNeeded);
        feeder.key_down(&mut kb, 2, KeyStroke::plain(Key::On), ShiftPolicy::AddIfNeeded);
        run(&mut feeder, &mut kb, BUSY, 10);
        feeder.release_all();
        run(&mut feeder, &mut kb, BUSY, 1);
        assert!(Key::ALL.iter().all(|&k| !kb.is_pressed(k)));
        assert_eq!(feeder.pending(), 0);
    }

    #[test]
    fn powered_off_machine_drops_typed_text() {
        let mut feeder = KeyFeeder::new();
        let mut kb = Keyboard::new();
        feeder.queue_text("AB");
        feeder.tap(&mut kb, &[KeyStroke::plain(Key::C)], ShiftPolicy::AddIfNeeded);
        let off = FeedEnv { powered_on: false, ..BUSY };
        run(&mut feeder, &mut kb, off, 1);
        assert_eq!(feeder.pending(), 1, "solo queda la tecla viva");
    }
}
