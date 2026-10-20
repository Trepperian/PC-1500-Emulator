//! Arnés "headless" (sin GUI) para manejar una [`Pc1500`] completa desde
//! código: arrancar la ROM real hasta el prompt de BASIC, pulsar teclas
//! durante un número exacto de frames emulados, teclear texto ASCII
//! convirtiéndolo a la secuencia de teclas físicas de la PC-1500 (con
//! `SHIFT` donde haga falta) y leer de vuelta lo que muestra el LCD como
//! texto.
//!
//! Existe para poder verificar automáticamente (`cargo test`) cualquier
//! funcionalidad nueva del emulador — BREAK, TIME, carga/guardado de
//! programas, mapeo de teclado... — contra el comportamiento de la ROM
//! auténtica, en vez de tener que comprobarlo a mano en la GUI. Las
//! piezas que no dependen de los tests (p. ej. [`text_to_keystrokes`],
//! útil para "pegar" texto en la GUI) son públicas a propósito.
//!
//! # Tiempos
//! Todo se mide en llamadas a [`Pc1500::step_frame`] ("frames"), no en
//! tiempo de host: un frame equivale siempre a [`crate::FRAME_DURATION`]
//! (~11.5 ms) de tiempo emulado, así que estos números son deterministas
//! e independientes de lo rápido que sea el ordenador que ejecuta los
//! tests. Ver [`KEY_HOLD_FRAMES`] y [`KEY_RELEASE_FRAMES`] para cómo se
//! midieron los mínimos.
//!
//! # Reloj (`TIME`)
//! [`Pc1500::new`] arranca el reloj `µPD1990AC` con la hora local del host,
//! que avanza en tiempo real: lo que quiere la GUI, pero no un test (lo
//! que lea `TIME`, y la semilla de `RANDOM`, dependerían de cuándo y a
//! qué velocidad se ejecuta). Los tests deben crear la máquina con
//! [`Pc1500::new_deterministic`] (reloj fijo en [`HARNESS_RTC_START`]
//! que avanza con los ciclos emulados), o elegir otra hora con
//! [`Pc1500::set_rtc_time_base`] justo después de [`Pc1500::new`].
//!
//! # Encendido y auto-apagado
//! La tecla `OFF` y el auto-apagado de la ROM (tras ~5 minutos emulados
//! esperando teclado sin que se pulse nada) apagan la máquina de verdad:
//! [`Pc1500::is_powered_on`] pasa a `false`, la CPU deja de ejecutar y
//! las funciones que esperan a la ROM ([`Pc1500::run_until_idle`],
//! [`Pc1500::enter_line`]...) ya no terminan. [`Pc1500::power_on`] la
//! vuelve a encender. Un test que deje correr la máquina ociosa mucho
//! rato (más de ~25000 frames) debe desactivarlo antes con
//! [`Pc1500::set_auto_power_off`]`(false)`.
//!
//! # Teclear líneas enteras
//! Para introducir líneas de programa o comandos uno detrás de otro hay
//! que usar [`Pc1500::enter_line`] / [`Pc1500::enter_lines`], no
//! [`Pc1500::type_text`] + [`Pc1500::run_until_idle`]: que la ROM esté
//! ociosa en `WAIT_4_KB` no significa que vaya a aceptar la siguiente
//! tecla. Ver [`Pc1500::is_ready_for_key`] para la causa exacta (el
//! "antirrebote de suelta" del bit 0 de `CURS_CTRL`).

use core::{array, error::Error, fmt};

use crate::{Key, Pc1500, RtcDateTime, RtcTimeBase, display::DISPLAY_WIDTH};

/// Hora a la que arranca el reloj de [`Pc1500::new_deterministic`]: el
/// 1 de enero a las 12:00:00, domingo (día de la semana 0). Lejos de
/// cualquier cambio de día para que un test que no mire el reloj nunca
/// vea pasar nada raro.
pub const HARNESS_RTC_START: RtcDateTime = RtcDateTime {
    month: 1,
    day: 1,
    hour: 12,
    minute: 0,
    second: 0,
    weekday: 0,
};

/// Número de caracteres que caben en el LCD de la PC-1500: 156 columnas
/// de píxeles / 6 columnas por carácter (5 de glifo + 1 de separación).
pub const LCD_CHARS: usize = 26;

/// Columnas de píxeles que ocupa cada carácter en el LCD: 5 columnas de
/// glifo (la matriz 5x7 de la ROM) seguidas de 1 columna vacía de
/// separación. Comprobado sobre el mensaje de arranque en frío
/// `NEW0? :CHECK`, cuya "N" ocupa exactamente las columnas 0-4.
pub const LCD_CELL_WIDTH: usize = 6;

/// Frames que hay que mantener pulsada una tecla para que la ROM la
/// registre de forma fiable.
///
/// El bucle de espera de teclado de la ROM (`WAIT_4_KB`, $E243) duerme
/// en `HLT` ($E2A8) entre interrupciones de temporizador; al detectar
/// una tecla la convierte con `KEY_2_ASCII` ($E42C), la procesa, y no
/// acepta otra hasta haber visto el teclado vacío durante un rato.
/// Medido tecleando un texto de 23 caracteres distintos (y otro con
/// teclas repetidas, `AABB1122`) con todas las combinaciones de 1 a 7
/// frames de pulsación (`h`) y de suelta (`r`): funciona si y solo si
/// `r >= 2` y `h + r >= 6` (con menos se pierden teclas, p. ej. `h=r=2`
/// teclea solo una de cada dos). Mantener la tecla pulsada mucho más
/// tiempo (probado hasta 80 frames) no produce autorrepetición para
/// teclas de carácter. Los valores por defecto, 4 + 4 = 8 frames por
/// tecla (~92 ms emulados), dejan 2 frames de margen sobre el mínimo
/// en ambos lados; `tests/harness.rs` comprueba que el mínimo `3 + 3`
/// sigue funcionando para detectar si algún cambio futuro lo empeora.
pub const KEY_HOLD_FRAMES: usize = 4;

/// Frames que hay que dejar SIN ninguna tecla pulsada tras soltar una,
/// antes de pulsar la siguiente: la ROM tiene que ver el teclado vacío
/// para "rearmar" la detección (si no, la siguiente pulsación se toma
/// como la misma tecla mantenida y se pierde). Ver [`KEY_HOLD_FRAMES`]
/// para la medida.
pub const KEY_RELEASE_FRAMES: usize = 4;

/// Máximo de frames que [`Pc1500::boot`] espera a que la ROM llegue al
/// bucle de espera de teclado. En la práctica el arranque en frío tarda
/// 36 frames (~0.4 s emulados: test de memoria, borrado del LCD y
/// mensaje `NEW0? :CHECK`) y [`Pc1500::boot`] completo, con el `CL`, 46
/// (medido con la tabla virtual de `CLOAD`/`CSAVE` de $9800, cuyo vector
/// de inicialización llama la ROM al arrancar: sin ella eran 35 y 45);
/// el límite solo evita colgar un test si el emulador se rompe.
pub const BOOT_MAX_FRAMES: usize = 600;

/// Máximo de frames que [`Pc1500::enter_line`] espera a que la ROM
/// termine de procesar una línea y vuelva a estar lista para teclado
/// (~34 s emulados). Meter una línea de programa o ejecutar un comando
/// directo sencillo tarda de 2 a ~10 frames; el límite solo evita colgar
/// un test si algo se rompe. Para comandos que tardan más (p. ej. `RUN`
/// de un programa largo), usar [`Pc1500::type_text`] y
/// [`Pc1500::run_until_ready`] con un límite propio.
pub const LINE_MAX_FRAMES: usize = 3000;

/// `CURS_CTRL` ($7B0E), byte de control del cursor/teclado de la ROM.
/// Ver [`Pc1500::is_ready_for_key`].
const CURS_CTRL: u32 = 0x7B0E;

/// Bit 0 de `CURS_CTRL`: "hay una tecla que todavía no se ha visto
/// soltar". Ver [`Pc1500::is_ready_for_key`].
const CURS_CTRL_KEY_DOWN: u8 = 0x01;

/// Tabla de caracteres 5x7 de la ROM (`$FCA0`, "Character set table"):
/// 96 glifos para los códigos $20-$7F, 5 bytes por glifo, un byte por
/// columna con el bit 0 = fila superior y el bit 6 = fila inferior (la
/// misma disposición que decodifica `display.rs`). La usa `CHAR_2_ADDR`
/// ($EE48): dirección = $FC00 + 5 × código.
const FONT_TABLE: u32 = 0xFCA0;
const FONT_FIRST_CHAR: u8 = 0x20;
const FONT_CHARS: u8 = 96;

// `KeyStroke`, `key_for_char` y `text_to_keystrokes` viven en
// `keyboard.rs` (los usa también el alimentador de teclas de la GUI, ver
// `key_feeder.rs`); se reexportan aquí para que la API del arnés no
// cambie.
pub use crate::keyboard::{KeyStroke, key_for_char, text_to_keystrokes};

/// Por qué falló [`Pc1500::enter_line`] / [`Pc1500::enter_lines`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterLineError {
    /// El texto contiene un carácter sin tecla (ver [`key_for_char`]).
    /// No se ha pulsado nada.
    Unmappable(char),
    /// [`Pc1500::enter_line`] recibió más de una línea (un `\n` que no
    /// es el final). Usar [`Pc1500::enter_lines`]. No se ha pulsado nada.
    MultipleLines,
    /// La ROM no quedó lista para la siguiente tecla en
    /// [`LINE_MAX_FRAMES`] frames (un programa que no termina, un
    /// `INPUT` que no llega a pedir teclado, un emulador roto...). Puede
    /// haberse tecleado parte del texto.
    NotReady,
}

impl fmt::Display for EnterLineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unmappable(c) => write!(f, "el car\u{e1}cter '{c}' no tiene tecla en la PC-1500"),
            Self::MultipleLines => f.write_str("enter_line recibe una sola l\u{ed}nea"),
            Self::NotReady => write!(
                f,
                "la ROM no volvi\u{f3} a estar lista para teclado en {LINE_MAX_FRAMES} frames"
            ),
        }
    }
}

impl Error for EnterLineError {}

impl Pc1500 {
    /// Como [`Pc1500::new`], pero con el reloj `µPD1990AC` en
    /// [`RtcTimeBase::Emulated`], empezando en [`HARNESS_RTC_START`]: una hora fija
    /// que avanza con los ciclos emulados, sin mirar el reloj del host.
    /// Así todo lo que haga la máquina (incluidos `TIME` y `RANDOM`)
    /// depende solo de lo que se le teclee. Es la forma de crear la
    /// máquina en los tests.
    #[must_use]
    pub fn new_deterministic() -> Self {
        let mut pc = Self::new();
        pc.set_rtc_time_base(RtcTimeBase::Emulated(HARNESS_RTC_START));
        pc
    }

    /// Ejecuta exactamente `frames` llamadas a [`Pc1500::step_frame`].
    pub fn run_frames(&mut self, frames: usize) {
        for _ in 0..frames {
            self.step_frame();
        }
    }

    /// `true` si la ROM está ociosa esperando una tecla: la CPU está
    /// detenida en el `HLT` del bucle `WAIT_4_KB` ($E243-$E33E), que es
    /// donde pasa casi todo el tiempo cuando muestra el prompt (o cuando
    /// un `INPUT` espera datos). Solo es fiable mirado entre frames: el
    /// bucle se despierta en cada interrupción de temporizador para
    /// parpadear el cursor y vuelve a dormirse enseguida.
    #[must_use]
    ///
    /// Con la máquina apagada (ver [`Pc1500::is_powered_on`]) es siempre
    /// `false`: no llamar a [`Pc1500::run_until_idle`] en ese estado.
    pub fn is_waiting_for_key(&self) -> bool {
        self.lh5801.bf() && self.lh5801.is_halted() && self.cpu_in_wait_4_kb()
    }

    /// Ejecuta frames hasta que `done(self)` sea cierto, comprobándolo
    /// después de cada frame, con un máximo de `max_frames`. Devuelve el
    /// número de frames ejecutados, o `None` si se agotó el límite.
    pub fn run_until<F: FnMut(&Self) -> bool>(
        &mut self,
        max_frames: usize,
        mut done: F,
    ) -> Option<usize> {
        for frame in 1..=max_frames {
            self.step_frame();
            if done(self) {
                return Some(frame);
            }
        }
        None
    }

    /// Ejecuta frames hasta que la ROM vuelva a esperar teclado (ver
    /// [`Pc1500::is_waiting_for_key`]) durante 2 frames seguidos — uno
    /// solo podría ser un `HLT` momentáneo a mitad de un comando que
    /// todavía no ha terminado de pintar el resultado. Devuelve los
    /// frames ejecutados o `None` si no ocurre en `max_frames`.
    pub fn run_until_idle(&mut self, max_frames: usize) -> Option<usize> {
        let mut streak = 0;
        self.run_until(max_frames, |pc| {
            streak = if pc.is_waiting_for_key() { streak + 1 } else { 0 };
            streak >= 2
        })
    }

    /// `true` si la ROM no solo está ociosa en `WAIT_4_KB` (ver
    /// [`Pc1500::is_waiting_for_key`]) sino que además aceptará la
    /// PRÓXIMA tecla que se pulse como una pulsación nueva.
    ///
    /// # Por qué no basta con `is_waiting_for_key`
    /// `WAIT_4_KB` ($E243) no detecta "flancos" de tecla: en cada
    /// despertar del temporizador mira si hay ALGUNA tecla pulsada, y el
    /// bit 0 de `CURS_CTRL` ($7B0E) recuerda si en el despertar anterior
    /// ya la había:
    ///
    /// - Tecla pulsada con el bit 0 a 0 (`WAIT_4_KB_7`, $E2AC): tecla
    ///   nueva. La ROM pone el bit 0 a 1, guarda el código en `KEY_LAST`
    ///   ($7B0F) y sale de `WAIT_4_KB` devolviéndola.
    /// - Tecla pulsada con el bit 0 a 1: se considera la MISMA tecla
    ///   mantenida. Sin autorrepetición activa (bit 6 a 0, lo normal
    ///   salvo con las flechas) se ignora (`WAIT_4_KB_16`, $E2FF) y
    ///   además se reinicia el contador de suelta (`UH = $F8`) — da igual
    ///   que sea otra tecla distinta.
    /// - Ninguna tecla con el bit 0 a 1 (`WAIT_4_KB_10`, $E2C4): se
    ///   incrementa `UH`, y solo cuando llega a 0 (8 despertares
    ///   seguidos sin tecla) se borra el bit 0 ($E2D4).
    ///
    /// Ese bit 0 SOLO se borra dentro de `WAIT_4_KB`: la entrada a la
    /// rutina ($E24A) borra el bit 1 pero no el 0. Así que si la ROM
    /// tarda en procesar una tecla más que lo que dura su pulsación (lo
    /// típico con ENTER en una línea larga: tokenizar, insertarla en el
    /// programa y volver a pintarla), cuando vuelve a `WAIT_4_KB` y se
    /// duerme en `HLT` el bit 0 sigue a 1 aunque ya no haya ninguna
    /// tecla pulsada, y la ROM necesita todavía esos 8 despertares
    /// (~1-2 frames) sin teclas antes de aceptar otra. Una tecla pulsada
    /// en ese hueco se pierde ENTERA: medido tecleando `jackpot` línea a
    /// línea, tras `170 SI=112:DO=105:RE=93:MI=82:SO=68:TP=80` (41
    /// caracteres) `run_until_idle` volvía con `CURS_CTRL = $01` y la
    /// línea siguiente `200 REM ...` llegaba como `00 REM ...` (ERROR
    /// 1). Es el comportamiento de la ROM auténtica, no un fallo del
    /// emulador: en la máquina real pasaría lo mismo tecleando así de
    /// rápido. Esta es la condición exacta que faltaba (antes se tapaba
    /// con ~30 frames fijos de espera "por si acaso").
    #[must_use]
    pub fn is_ready_for_key(&self) -> bool {
        self.is_waiting_for_key() && self.read_byte(CURS_CTRL) & CURS_CTRL_KEY_DOWN == 0
    }

    /// Como [`Pc1500::run_until_idle`], pero esperando a
    /// [`Pc1500::is_ready_for_key`] (también 2 frames seguidos): tras
    /// esto la siguiente tecla que se pulse no se pierde. Devuelve los
    /// frames ejecutados o `None` si no ocurre en `max_frames` — p. ej.
    /// si hay una tecla mantenida pulsada, porque entonces el bit 0 de
    /// `CURS_CTRL` nunca se borra.
    pub fn run_until_ready(&mut self, max_frames: usize) -> Option<usize> {
        let mut streak = 0;
        self.run_until(max_frames, |pc| {
            streak = if pc.is_ready_for_key() { streak + 1 } else { 0 };
            streak >= 2
        })
    }

    /// Arranca la ROM real desde un `Pc1500` recién creado hasta que
    /// queda esperando teclado. Tras un arranque en frío el LCD muestra
    /// `NEW0? :CHECK` (la ROM no reconoce la RAM como inicializada); la
    /// primera tecla que se pulse solo borra ese mensaje, por eso este
    /// método pulsa además `CL`, dejando el LCD con el prompt `>`.
    ///
    /// El programa BASIC tokenizado que `MemoryBus::new()` inyecta como
    /// fixture en $0100 sigue en memoria después de esto (no se hace
    /// `NEW`): el test que no lo quiera puede teclear `NEW\n`.
    ///
    /// Devuelve los frames que tardó, o `None` si la ROM no llegó al
    /// prompt en [`BOOT_MAX_FRAMES`].
    pub fn boot(&mut self) -> Option<usize> {
        let boot_frames = self.run_until_idle(BOOT_MAX_FRAMES)?;
        self.tap(KeyStroke::plain(Key::Cl));
        // `run_until_ready` y no `run_until_idle`: así la primera tecla
        // que pulse el test tras `boot()` nunca cae en el antirrebote de
        // suelta del `CL` (ver `is_ready_for_key`).
        let after_cl = self.run_until_ready(BOOT_MAX_FRAMES)?;
        Some(boot_frames + KEY_HOLD_FRAMES + KEY_RELEASE_FRAMES + after_cl)
    }

    /// Enciende una máquina apagada (ver [`Pc1500::is_powered_on`]) como
    /// lo haría una persona: pulsa `ON` con los tiempos por defecto y
    /// espera a que la ROM termine su rutina `RESET` y quede lista para
    /// la siguiente tecla (ver [`Pc1500::is_ready_for_key`]). Lo que
    /// muestra luego el LCD depende de cómo se apagó: tras la tecla `OFF`
    /// el prompt `>` (`COLD_START`, con el programa conservado); tras el
    /// auto-apagado, la línea que se estaba editando, tal cual.
    ///
    /// Esperar a [`Pc1500::is_ready_for_key`] (y no solo a
    /// [`Pc1500::is_waiting_for_key`]) es lo que hace falta aquí: al
    /// apagarse con la tecla `OFF` la ROM corta la alimentación con esa
    /// tecla todavía pulsada, así que el bit 0 de `CURS_CTRL` ("tecla
    /// sin soltar") sigue a 1 al encender, y la primera tecla pulsada
    /// antes de que `WAIT_4_KB` vea el teclado vacío durante 8
    /// despertares se pierde entera. No hace falta pulsar `CL` después.
    ///
    /// Devuelve los frames que tardó la ROM tras soltar `ON`, o `None` si
    /// no quedó lista en [`BOOT_MAX_FRAMES`] (p. ej. si la máquina ya
    /// estaba encendida y un programa en marcha ignora el BREAK). Con la
    /// máquina encendida, pulsar `ON` es un BREAK.
    pub fn power_on(&mut self) -> Option<usize> {
        self.tap_key_for(Key::On, KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES);
        self.run_until_ready(BOOT_MAX_FRAMES)
    }

    /// Pulsa `key` durante `hold` frames y la suelta durante `release`
    /// frames.
    pub fn tap_key_for(&mut self, key: Key, hold: usize, release: usize) {
        self.press(key);
        self.run_frames(hold);
        self.release(key);
        self.run_frames(release);
    }

    /// Teclea una [`KeyStroke`] con los tiempos por defecto
    /// ([`KEY_HOLD_FRAMES`] / [`KEY_RELEASE_FRAMES`]), pulsando y
    /// soltando antes `SHIFT` si hace falta.
    pub fn tap(&mut self, stroke: KeyStroke) {
        self.tap_with_timing(stroke, KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES);
    }

    /// Igual que [`Pc1500::tap`] pero con tiempos explícitos (para
    /// medirlos o para programas que leen el teclado más despacio).
    pub fn tap_with_timing(&mut self, stroke: KeyStroke, hold: usize, release: usize) {
        if stroke.shift {
            self.tap_key_for(Key::Shift, hold, release);
        }
        self.tap_key_for(stroke.key, hold, release);
    }

    /// Teclea `text` (ver [`text_to_keystrokes`]) con los tiempos por
    /// defecto. Si contiene algún carácter sin tecla devuelve `Err(c)`
    /// sin haber pulsado nada.
    ///
    /// No espera a que la ROM termine de procesar un ENTER, así que con
    /// varias líneas seguidas en el mismo texto la primera tecla de una
    /// línea se puede perder si la anterior tarda en procesarse (ver
    /// [`Pc1500::is_ready_for_key`]). Para teclear líneas completas usar
    /// [`Pc1500::enter_line`] / [`Pc1500::enter_lines`]; `type_text` es
    /// para lo demás (eco de caracteres sueltos, responder a un `INPUT`
    /// o a un `PRINT` que espera ENTER dentro de un programa que luego
    /// sigue ejecutándose...), seguido de [`Pc1500::run_until_ready`].
    pub fn type_text(&mut self, text: &str) -> Result<(), char> {
        self.type_text_with_timing(text, KEY_HOLD_FRAMES, KEY_RELEASE_FRAMES)
    }

    /// Igual que [`Pc1500::type_text`] pero con tiempos explícitos.
    pub fn type_text_with_timing(
        &mut self,
        text: &str,
        hold: usize,
        release: usize,
    ) -> Result<(), char> {
        for stroke in text_to_keystrokes(text)? {
            self.tap_with_timing(stroke, hold, release);
        }
        Ok(())
    }

    /// Teclea UNA línea seguida de ENTER y espera a que la ROM la haya
    /// procesado y vuelva a estar lista para la siguiente tecla (ver
    /// [`Pc1500::is_ready_for_key`]), así que se puede llamar una vez
    /// tras otra sin perder teclas. Sirve para líneas de programa
    /// (`"10 PRINT 5"`) y para comandos directos que vuelven al prompt o
    /// esperan teclado (`"LIST"`, `"PRINT 1+2"`, `"RUN"` de un programa
    /// que hace `PRINT`...). `""` pulsa solo ENTER.
    ///
    /// El `\n` final es opcional (`"10 A=1"` y `"10 A=1\n"` son lo
    /// mismo, y se acepta también `\r\n`); cualquier otro `\n` es un
    /// error ([`EnterLineError::MultipleLines`]): para varias líneas,
    /// [`Pc1500::enter_lines`]. Si la ROM no estaba lista al empezar
    /// (p. ej. justo después de un `type_text`), primero espera a que lo
    /// esté.
    ///
    /// Devuelve los frames que tardó la ROM en quedar lista tras el
    /// ENTER (sin contar los del propio tecleo).
    pub fn enter_line(&mut self, line: &str) -> Result<usize, EnterLineError> {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let body = body.strip_suffix('\r').unwrap_or(body);
        if body.contains('\n') {
            return Err(EnterLineError::MultipleLines);
        }
        let mut strokes = text_to_keystrokes(body).map_err(EnterLineError::Unmappable)?;
        strokes.push(KeyStroke::plain(Key::Enter));
        self.type_strokes_when_ready(&strokes)
    }

    /// Teclea varias líneas con [`Pc1500::enter_line`], una detrás de
    /// otra (separadas por `\n` o `\r\n`; el salto de línea final es
    /// opcional y las líneas vacías intermedias son un ENTER sin más).
    /// Comprueba TODO el texto antes de pulsar nada, así que un
    /// [`EnterLineError::Unmappable`] no deja líneas a medias. Devuelve
    /// la suma de los frames de espera de cada línea.
    pub fn enter_lines(&mut self, text: &str) -> Result<usize, EnterLineError> {
        let lines: Vec<Vec<KeyStroke>> = text
            .lines()
            .map(|line| {
                let mut strokes = text_to_keystrokes(line)?;
                strokes.push(KeyStroke::plain(Key::Enter));
                Ok(strokes)
            })
            .collect::<Result<_, char>>()
            .map_err(EnterLineError::Unmappable)?;
        let mut total = 0;
        for strokes in &lines {
            total += self.type_strokes_when_ready(strokes)?;
        }
        Ok(total)
    }

    /// Núcleo de [`Pc1500::enter_line`]: espera (si hace falta) a que la
    /// ROM esté lista, teclea `strokes` con los tiempos por defecto y
    /// vuelve a esperar a que esté lista, devolviendo los frames de esa
    /// última espera.
    fn type_strokes_when_ready(&mut self, strokes: &[KeyStroke]) -> Result<usize, EnterLineError> {
        if !self.is_ready_for_key() {
            self.run_until_ready(LINE_MAX_FRAMES)
                .ok_or(EnterLineError::NotReady)?;
        }
        for &stroke in strokes {
            self.tap(stroke);
        }
        self.run_until_ready(LINE_MAX_FRAMES)
            .ok_or(EnterLineError::NotReady)
    }

    /// Las 156 columnas de píxeles del LCD, leídas directamente de la RAM
    /// de pantalla con la misma correspondencia que `display.rs`: cada
    /// columna es un byte con el bit 0 = fila superior. Columnas 0-38 =
    /// nibbles bajos de $7600-$764D, 39-77 = bajos de $7700-$774D, 78-116
    /// = altos de $7600.., 117-155 = altos de $7700... (el nibble de la
    /// dirección par da las filas 0-3 y el de la impar las 4-6).
    ///
    /// No mira si el LCD está encendido (flag `DISP` de la CPU): devuelve
    /// lo que hay en la RAM de pantalla.
    #[must_use]
    pub fn lcd_columns(&self) -> [u8; DISPLAY_WIDTH] {
        let mut columns = [0; DISPLAY_WIDTH];
        for ind in (0..0x4D_u32).step_by(2) {
            for (base, first_x) in [(0x7600_u32, 0_usize), (0x7700, 39)] {
                let even = self.read_byte(base | ind);
                let odd = self.read_byte((base | ind) + 1);
                let x = first_x + (ind as usize >> 1);
                columns[x] = ((even & 0x0F) | (odd << 4)) & 0x7F;
                columns[x + 78] = ((even >> 4) | (odd & 0xF0)) & 0x7F;
            }
        }
        columns
    }

    /// Glifo 5x7 de la ROM para el código `code` ($20-$7F).
    fn font_glyph(&self, code: u8) -> [u8; 5] {
        let base = FONT_TABLE + 5 * u32::from(code - FONT_FIRST_CHAR);
        let mut glyph = [0; 5];
        for (addr, column) in (base..).zip(glyph.iter_mut()) {
            *column = self.read_byte(addr) & 0x7F;
        }
        glyph
    }

    /// Las 26 celdas de carácter del LCD, cada una reconocida comparando
    /// sus 5 columnas de glifo con la tabla de caracteres de la ROM
    /// ($FCA0). `None` si la celda no coincide con ningún glifo (gráficos
    /// de `GPRINT`, katakana, el bloque del cursor de inserción...) o si
    /// su columna de separación no está vacía.
    ///
    /// El cursor normal es el glifo de `_`, así que una celda con el
    /// cursor visible se lee como `'_'` (y como `' '` en la fase apagada
    /// del parpadeo).
    #[must_use]
    pub fn lcd_cells(&self) -> [Option<char>; LCD_CHARS] {
        let columns = self.lcd_columns();
        // El código $7F es el bloque del cursor de inserción, no un
        // carácter imprimible: se excluye de la búsqueda.
        let glyphs: Vec<(u8, [u8; 5])> = (FONT_FIRST_CHAR..FONT_FIRST_CHAR + FONT_CHARS - 1)
            .map(|code| (code, self.font_glyph(code)))
            .collect();
        array::from_fn(|cell| {
            let start = cell * LCD_CELL_WIDTH;
            if columns[start + 5] != 0 {
                return None;
            }
            let cell_cols = &columns[start..start + 5];
            glyphs
                .iter()
                .find(|(_, glyph)| glyph == cell_cols)
                .map(|&(code, _)| char::from(code))
        })
    }

    /// El LCD como texto de exactamente 26 caracteres (ver
    /// [`Pc1500::lcd_cells`]), con `'?'` en las celdas no reconocidas.
    /// Ojo: un `?` auténtico también se lee como `'?'`; usar
    /// [`Pc1500::lcd_cells`] si hay que distinguirlos.
    #[must_use]
    pub fn lcd_text(&self) -> String {
        self.lcd_cells().iter().map(|c| c.unwrap_or('?')).collect()
    }

    /// Bytes crudos de los indicadores del LCD: `[$764E, $764F]`.
    ///
    /// Observado en el arranque (modo PRO, DEG): `[$40, $23]`. Con la
    /// asignación de bits de PockEmul ($764E: BUSY $01, SHIFT $02, KANA
    /// $04, SML $08, III $10, II $20, I $40, DEF $80; $764F: DE $01, G
    /// $02, RAD $04, RESERVE $10, PRO $20, RUN $40) eso es "I", "DEG" y
    /// "PRO": un bit a 1 = indicador encendido.
    #[must_use]
    pub fn lcd_symbol_bytes(&self) -> [u8; 2] {
        [self.read_byte(0x764E), self.read_byte(0x764F)]
    }

    /// Lee `len` bytes consecutivos desde `addr` (espacio ME0 si
    /// `addr < 0x10000`, ME1 si se suma `0x10000`).
    #[must_use]
    pub fn read_bytes(&self, addr: u32, len: usize) -> Vec<u8> {
        (addr..).take(len).map(|a| self.read_byte(a)).collect()
    }

    /// Lee una palabra de 16 bits big-endian (el orden que usa la ROM
    /// para sus punteros de RAM del sistema, p. ej. `BASPRG_ST_H` $7865 /
    /// `BASPRG_ST_L` $7866).
    #[must_use]
    pub fn read_word_be(&self, addr: u32) -> u16 {
        u16::from_be_bytes([self.read_byte(addr), self.read_byte(addr + 1)])
    }
}
