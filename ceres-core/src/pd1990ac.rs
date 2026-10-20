//! Reloj/calendario NEC `µPD1990AC` de la PC-1500 (el que lee y escribe la
//! instrucción BASIC `TIME`, y el que da la base de tiempos de `WAIT`,
//! `PAUSE` y `BEEP`).
//!
//! # Qué es el chip
//! El `µPD1990AC` es un reloj alimentado por la pila de respaldo, con un
//! contador interno de 40 bits en BCD (salvo el mes, que va en
//! hexadecimal) y un registro de desplazamiento de 40 bits por el que se
//! leen y escriben esos contadores en serie. Orden de los 40 bits, del
//! menos significativo (el primero que sale por `DATA OUT`) al más:
//!
//! ```text
//! bits  0-7 : segundos (BCD, 00-59)
//! bits  8-15: minutos  (BCD, 00-59)
//! bits 16-23: horas    (BCD, 00-23)
//! bits 24-31: día      (BCD, 01-31)
//! bits 32-35: día de la semana (0-6, domingo = 0)
//! bits 36-39: mes      (hexadecimal, 1-12)
//! ```
//!
//! Es exactamente el orden en que la ROM guarda los 5 bytes en `ARX`, de
//! $7A06 (segundos) hacia abajo hasta $7A02 (`mes<<4 | día_semana`).
//!
//! # Conexionado en la PC-1500
//! Todo pasa por el puerto C del LH5810 (`OPC`, $F008), como en `PockEmul`
//! (`pc1500.cpp`, líneas 722-756):
//!
//! ```text
//! PC0 = DATA IN    PC1 = STB    PC2 = CLK
//! PC3 = C0 y además OUT ENBL    PC4 = C1    PC5 = C2
//! ```
//!
//! y las dos salidas vuelven por el puerto B: `TP` en PB5 y `DATA OUT` en
//! PB6 (PB7 es la tecla ON/BRK, ver `Keyboard::on_pulse`).
//!
//! Que PC3 sea a la vez `C0` y `OUT ENBL` no es un descuido: la ROM
//! cuenta con ello. `ARX_2_TIME_1` ($E5A9-$E5AE, `ANI #(Y),$C7` /
//! `ORI #(Y),$08`) sube PC3 tras el comando 0 solo para habilitar la
//! salida, y los comandos que leen datos (1 = desplazar, $08; 3 = leer la
//! hora, $18) tienen `C0 = 1`, es decir, la salida habilitada.
//!
//! # Comandos
//! Con un flanco de subida de `STB` el chip memoriza el comando `C2C1C0`.
//! Hay dos grupos independientes (hoja de datos de NEC): los de `C2 = 0`
//! controlan el registro y los contadores, los de `C2 = 1` solo eligen la
//! frecuencia de `TP`, y cada grupo conserva su último comando sin que el
//! otro lo toque.
//!
//! - 0 `Register Hold`: no hace nada con el registro; `DATA OUT` = 1 Hz.
//! - 1 `Register Shift`: con cada flanco de subida de `CLK` el registro
//!   se desplaza un bit a la derecha, `DATA IN` entra por el bit 39 y
//!   `DATA OUT` es el bit 0.
//! - 2 `Time Set & Counter Hold`: copia el registro a los contadores y los
//!   para hasta el siguiente comando del grupo 0.
//! - 3 `Time Read`: copia los contadores al registro; `DATA OUT` = 0.5 Hz.
//! - 4/5/6: `TP` = 64/256/2048 Hz. 7: modo de test (solo se anota; la ROM
//!   no lo usa nunca).
//!
//! # Cómo lo usa la ROM A04
//! - `TIME` (lectura, `TIME2ARX` $E5B4): comando 3, y luego
//!   `WRITE_2_CLOCK` ($E52B): comando 1 y 40 pulsos de `CLK`, leyendo PB6
//!   ANTES de cada pulso. Después `BCMD_TIME` ($DE82) lo convierte al
//!   número `MMDDHH.MMSS`.
//! - `TIME=` (`ARX_2_VAR_5`..`13`, $DE1D-$DE56, y `ARX_2_TIME` $E59A):
//!   valida el número (mes > 12 → `ERROR 23`), mete los 40 bits con el
//!   comando 1 (el día de la semana queda a 0), y luego comando 2 (Time
//!   Set) y comando 0.
//! - `WAIT`, `PAUSE`, `BEEP` (`TIME_DELAY`, $E88C): comando 4 (`TP` a
//!   64 Hz) y cuentan flancos de subida de PB5, 15.625 ms cada uno.
//! - `RANDOM` ($F648) lee la hora para sacar la semilla.
//!
//! # Bases de tiempo
//! Ver [`RtcTimeBase`]: la GUI usa la hora local del host (el reloj sigue
//! la hora real, a cualquier velocidad de emulación, como el chip real
//! con su pila), los tests una hora fija que avanza con los ciclos
//! emulados, para que sean deterministas. `TP` y `DATA OUT` van siempre
//! en tiempo EMULADO, así que `WAIT` y `BEEP` se aceleran y frenan con la
//! velocidad del emulador igual que la CPU.

use core::fmt;
use std::time::{Duration, Instant};

use chrono::{Datelike, Timelike};

/// Ciclos de máquina por segundo del LH5801 (oscilador de 2.6 MHz entre
/// 2). Es la unidad de `timer_state`/`emu_cycles`: el reloj emulado y
/// `TP` cuentan en estos ciclos. Lo usa también `lh5810.rs`.
pub const FREQUENCY: usize = 2_600_000 / 2;

/// [`FREQUENCY`] como `u64`, para la aritmética interna.
const FREQ: u64 = FREQUENCY as u64;

const NS_PER_SEC: u64 = 1_000_000_000;

const SECONDS_PER_DAY: u64 = 86_400;

/// Máscara de los 40 bits del registro de desplazamiento.
const SHIFT_MASK: u64 = (1 << 40) - 1;

/// Frecuencia de `TP` tras encender (comando 4, el que usa la ROM).
const TP_DEFAULT_HZ: u32 = 64;

/// Fecha y hora del `µPD1990AC`, en binario (no BCD).
///
/// El chip no tiene año. Los valores no se validan: [`Pd1990ac`] guarda
/// lo que se le escriba (`TIME=` acepta p. ej. el mes 0), igual que el
/// chip real.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RtcDateTime {
    /// Mes, 1-12 (`TIME=` admite también 0).
    pub month: u8,
    /// Día del mes, 1-31.
    pub day: u8,
    /// Hora, 0-23.
    pub hour: u8,
    /// Minuto, 0-59.
    pub minute: u8,
    /// Segundo, 0-59.
    pub second: u8,
    /// Día de la semana, 0-6 con domingo = 0 (el ejemplo de la hoja de
    /// datos: "July 16 (Sun.)" con un 0). La ROM lo pone a 0 en cada
    /// `TIME=`.
    pub weekday: u8,
}

impl fmt::Display for RtcDateTime {
    /// Formato día/mes y hora: `28/02 23:59:50`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02}/{:02} {:02}:{:02}:{:02}",
            self.day, self.month, self.hour, self.minute, self.second
        )
    }
}

/// De dónde saca el `µPD1990AC` la hora inicial y a qué ritmo cuenta los
/// segundos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RtcTimeBase {
    /// Lo predeterminado (la GUI): empieza en la hora LOCAL del host
    /// (`chrono::Local`, no UTC) y los segundos avanzan con el tiempo
    /// real (`std::time::Instant`), independientemente de la velocidad de
    /// emulación o de las pausas, como el chip real con su pila. El
    /// desfase horario del host se toma una sola vez al crearlo (no sigue
    /// cambios de horario de verano posteriores).
    HostLocal,
    /// Para los tests: empieza en la hora dada y los segundos avanzan con
    /// los ciclos emulados del LH5801 ([`FREQUENCY`] por segundo), así que
    /// todo es determinista.
    Emulated(RtcDateTime),
}

/// Línea de tiempo con la que cuenta los segundos (ver [`RtcTimeBase`]).
#[derive(Clone, Copy, Debug)]
enum Base {
    /// Tiempo real. El `Instant` es el inicio del segundo en curso del
    /// host en el momento de crear el chip (retrasado los nanosegundos ya
    /// transcurridos de ese segundo), para que los cambios de segundo del
    /// chip coincidan con los del reloj del host.
    Host(Instant),
    /// Ciclos emulados.
    Emulated,
}

/// Binario (0-99) a BCD.
const fn to_bcd(v: u8) -> u8 {
    ((v / 10) << 4) | (v % 10)
}

/// BCD a binario. No valida: un BCD inválido (p. ej. $5A) da un valor
/// fuera de rango, como el que leería la ROM.
const fn from_bcd(v: u8) -> u8 {
    (v >> 4) * 10 + (v & 0x0F)
}

/// Incremento BCD de un contador que vuelve a 0 al llegar a `limit` (en
/// BCD: $60 para segundos y minutos, $24 para las horas). Devuelve `true`
/// si ha vuelto a 0 (acarreo al contador siguiente).
const fn bcd_inc(v: u8, limit: u8) -> (u8, bool) {
    let mut n = v.wrapping_add(1);
    if n & 0x0F >= 10 {
        n = (n & 0xF0).wrapping_add(0x10);
    }
    if n >= limit { (0, true) } else { (n, false) }
}

/// `true` si `v` es un BCD válido menor que `limit`.
const fn bcd_below(v: u8, limit: u8) -> bool {
    v & 0x0F < 10 && v < limit
}

/// Convierte un campo de `chrono` (siempre pequeño: mes, día, hora...) a
/// `u8`.
fn small(v: u32) -> u8 {
    u8::try_from(v).unwrap_or(u8::MAX)
}

pub struct Pd1990ac {
    base: Base,
    /// Contadores, en el mismo orden que el registro de 40 bits:
    /// `[segundos BCD, minutos BCD, horas BCD, día BCD, mes<<4 | día de la
    /// semana]`.
    counter: [u8; 5],
    /// Registro de desplazamiento de 40 bits (los 24 bits altos, a 0).
    shift_reg: u64,
    /// `false` mientras el comando 2 (Time Set & Counter Hold) tiene los
    /// contadores parados.
    counting: bool,
    /// Inicio del segundo en curso, en nanosegundos de la línea de tiempo
    /// de `base` (ver [`Pd1990ac::now_ns`]).
    sec_origin_ns: u64,
    /// Último comando del grupo 0 (0-3).
    mode: u8,
    /// Frecuencia de `TP` en Hz (último comando del grupo 1).
    tp_freq: u32,
    /// Comando 7 (modo de test). Solo se anota.
    test_mode: bool,
    /// `emu_cycles` cuando se eligió la frecuencia de `TP` actual: origen
    /// de fase de la onda.
    tp_origin_cycles: u64,
    /// Ciclos emulados desde que se creó el chip. Monótono: sobrevive a
    /// los resets de la CPU (ver [`Pd1990ac::sync`]).
    emu_cycles: u64,
    /// Último `timer_state` del LH5801 visto por [`Pd1990ac::sync`].
    last_timer_state: usize,
    /// Últimos niveles de las patillas de entrada.
    pins: Pins,
}

/// Últimos niveles de las patillas de entrada del `µPD1990AC` que hacen
/// falta para detectar flancos (`STB`, `CLK`) o que deciden una salida
/// (`OUT ENBL`). `DATA IN` no se guarda: solo cuenta en el flanco de
/// `CLK`, y se lee del mismo byte de `OPC` (ver
/// [`Pd1990ac::write_port`]).
#[derive(Clone, Copy, Debug, Default)]
struct Pins {
    stb: bool,
    clk: bool,
    out_enable: bool,
}

impl Pd1990ac {
    /// Chip con la hora local del host ([`RtcTimeBase::HostLocal`]).
    pub fn new() -> Self {
        Self::with_time_base(RtcTimeBase::HostLocal)
    }

    pub fn with_time_base(time_base: RtcTimeBase) -> Self {
        let (base, start) = match time_base {
            RtcTimeBase::HostLocal => {
                let now = chrono::Local::now();
                // `nanosecond()` pasa de 1e9 durante un segundo intercalar.
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
            // El segundo en curso empezó en el instante 0 de la línea de
            // tiempo (ver `Base::Host`).
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

    /// Avanza el reloj emulado hasta `timer_state` (el contador de ciclos
    /// del LH5801). Se llama tras cada instrucción.
    ///
    /// Si `timer_state` es MENOR que el último visto, la CPU es nueva (un
    /// reset de [`crate::Pc1500::reset`] que conserva el chip) y empezó
    /// desde 0: se cuenta `timer_state` entero. Normalmente no pasa,
    /// porque el reset de la ROM ya no pone `timer_state` a 0 (ver
    /// `cpu_internal_reset` en `lh5801.rs`) y `Pc1500::reset` llama a
    /// [`Pd1990ac::rebase_timer`]; es solo una red de seguridad para que
    /// el reloj no salte nunca hacia atrás ni la resta desborde.
    pub fn sync(&mut self, timer_state: usize) {
        let delta = timer_state
            .checked_sub(self.last_timer_state)
            .unwrap_or(timer_state);
        self.emu_cycles += delta as u64;
        self.last_timer_state = timer_state;
    }

    /// Hace que el siguiente [`Pd1990ac::sync`] cuente desde
    /// `timer_state` sin sumar nada: para cuando el chip pasa a otra CPU
    /// (reset) o se instala en una máquina que ya llevaba ciclos.
    pub const fn rebase_timer(&mut self, timer_state: usize) {
        self.last_timer_state = timer_state;
    }

    /// Tiempo emulado en nanosegundos.
    fn emu_ns(&self) -> u64 {
        let ns = u128::from(self.emu_cycles) * u128::from(NS_PER_SEC) / u128::from(FREQ);
        u64::try_from(ns).unwrap_or(u64::MAX)
    }

    /// "Ahora" en la línea de tiempo de la base activa, en nanosegundos.
    /// Con [`Base::Host`] llama a `Instant::now()`: solo se usa en los
    /// comandos Time Read / Time Set y en [`Pd1990ac::now`], nunca por
    /// instrucción.
    fn now_ns(&self) -> u64 {
        match self.base {
            Base::Host(epoch) => u64::try_from(epoch.elapsed().as_nanos()).unwrap_or(u64::MAX),
            Base::Emulated => self.emu_ns(),
        }
    }

    /// Suma a los contadores los segundos enteros transcurridos desde
    /// `sec_origin_ns`. Los contadores no se actualizan en cada segundo,
    /// sino solo cuando alguien los mira (Time Read o [`Pd1990ac::now`]):
    /// el resultado es el mismo y no cuesta nada por instrucción.
    fn catch_up(&mut self) {
        if !self.counting {
            return;
        }
        let elapsed = self.now_ns().saturating_sub(self.sec_origin_ns);
        let seconds = elapsed / NS_PER_SEC;
        self.sec_origin_ns += seconds * NS_PER_SEC;
        self.advance_seconds(seconds);
    }

    /// Avanza `seconds` segundos. Los días enteros se saltan de golpe con
    /// [`Pd1990ac::increment_day`] (si la hora es BCD válida, sumar 86400
    /// segundos deja la misma hora y un día más), y el resto segundo a
    /// segundo.
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

    /// Un segundo más, con acarreo segundos → minutos → horas → día.
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

    /// Un día más: día de la semana (6 → 0) y día del mes, con paso al
    /// mes siguiente (12 → 1).
    ///
    /// Duración de los meses según la hoja de datos: 30 días abril, junio,
    /// septiembre y noviembre; 28 febrero; 31 el resto (también los meses
    /// inválidos 0 y 13-15 que se puedan escribir). Sin años bisiestos:
    /// "leap year is not automatically performed... possible to set
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
        // El límite lo pone el mes, no el contador: $FF solo evita que
        // `bcd_inc` lo ponga a 0.
        let (mut day, _) = bcd_inc(self.counter[3], 0xFF);
        let mut month = month;
        if day > last_day {
            day = 0x01;
            month = if month >= 12 { 1 } else { month + 1 };
        }
        self.counter[3] = day;
        self.counter[4] = (month << 4) | weekday;
    }

    /// Nuevo valor del registro `OPC` del LH5810 (ver el conexionado en la
    /// documentación del módulo).
    ///
    /// Basta con llamarlo una vez por instrucción: todas las secuencias de
    /// la ROM cambian una sola patilla por instrucción, y además escriben
    /// cada nivel dos veces (`STA #(Y)` dos veces, parejas `ORI`/`ANI`).
    pub fn write_port(&mut self, opc: u8) {
        let data_in = opc & 0x01 != 0;
        let stb = opc & 0x02 != 0;
        let clk = opc & 0x04 != 0;
        self.pins.out_enable = opc & 0x08 != 0;

        // Los comandos se memorizan en el flanco de SUBIDA de STB (antes
        // se miraba el nivel).
        if stb && !self.pins.stb {
            self.strobe((opc >> 3) & 0x07);
        }
        self.pins.stb = stb;

        // Desplazamiento en el flanco de subida de CLK, muestreando DATA
        // IN en ese mismo flanco (diagrama de tiempos de la hoja de datos,
        // página 7). DATA OUT pasa a ser el nuevo bit 0.
        if clk && !self.pins.clk && self.mode == 1 {
            self.shift_reg = ((self.shift_reg >> 1) | (u64::from(data_in) << 39)) & SHIFT_MASK;
        }
        self.pins.clk = clk;
    }

    /// Ejecuta el comando `C2C1C0` memorizado por un flanco de `STB`.
    fn strobe(&mut self, command: u8) {
        if command < 4 {
            // Grupo 0. Salir de Time Set suelta los contadores con un
            // segundo recién empezado: el comando 2 pone a 0 las etapas
            // 11-15 del divisor.
            if !self.counting && command != 2 {
                self.counting = true;
                self.sec_origin_ns = self.now_ns();
            }
            match command {
                // Register Hold. Saca además del modo de test.
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
                // 0 sin modo de test, y 1 (Register Shift): nada que hacer
                // al memorizarse.
                _ => {}
            }
            self.mode = command;
        } else {
            // Grupo 1: solo TP; el comando del grupo 0 sigue vigente.
            match command {
                4 => self.set_tp(64),
                5 => self.set_tp(256),
                6 => self.set_tp(2048),
                _ => self.test_mode = true,
            }
        }
    }

    /// Cambia la frecuencia de `TP`. La fase de la onda solo se reinicia
    /// si cambia la frecuencia.
    const fn set_tp(&mut self, hz: u32) {
        if hz != self.tp_freq {
            self.tp_freq = hz;
            self.tp_origin_cycles = self.emu_cycles;
        }
    }

    /// Nivel de la salida `TP` (PB5): onda cuadrada al 50% de la
    /// frecuencia elegida, en tiempo EMULADO, sin deriva.
    ///
    /// En [`FREQUENCY`] ciclos (1 s) caben exactamente `tp_freq` periodos,
    /// así que tomar el tiempo módulo 1 s no cambia la fase y evita
    /// cualquier desbordamiento.
    pub fn tp(&self) -> bool {
        let elapsed = (self.emu_cycles - self.tp_origin_cycles) % FREQ;
        (elapsed * 2 * u64::from(self.tp_freq) / FREQ) & 1 == 1
    }

    /// Nivel de la salida `DATA OUT` (PB6).
    ///
    /// - Con `OUT ENBL` (PC3) a 0 la salida está en alta impedancia y se
    ///   lee a 1 (resistencia de pull-up; MAME, `upd1990a.cpp`, hace lo
    ///   mismo). No comprobado en el esquema de la PC-1500, pero la ROM
    ///   nunca la lee en ese estado.
    /// - Comandos 1 y 2: el bit 0 del registro.
    /// - Comando 0: onda de 1 Hz; comando 3: de 0.5 Hz. Sacadas del tiempo
    ///   emulado, no del divisor de 1 s del chip (la fase puede no ser la
    ///   real). La ROM A04 nunca las muestrea: PB6 solo se lee en $E544,
    ///   con el comando 1.
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

    /// Fecha y hora actuales de los contadores (para la GUI y los tests).
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

    /// Bits 6-7 de `OPC` tal como los deja la tabla de E/S del reset de la
    /// ROM ($E169: $C0). El chip los ignora; se incluyen para escribir
    /// exactamente los mismos bytes que la ROM.
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

    /// Copia de `TIMER_MODE` ($E573, `VMJ ($5A)`): `OPC = (OPC & $C0) + A`
    /// dos veces, `+2` (STB a 1) dos veces y `-2` (STB a 0) dos veces.
    fn rom_mode(rtc: &mut Pd1990ac, a: u8) {
        let opc = OPC_HIGH | a;
        for value in [opc, opc, opc + 2, opc + 2, opc, opc] {
            rtc.write_port(value);
        }
    }

    /// Copia de `WRITE_2_CLOCK` ($E52B): comando 1 y, para `ARX` de $7A06
    /// a $7A02, 8 bits por byte: muestrea `DATA OUT` (PB6) ANTES del pulso,
    /// `ROR` (la muestra entra por el bit 7 y el bit saliente va a DATA
    /// IN, así que sale primero el LSB), DATA IN dos veces, CLK a 1 dos
    /// veces y CLK a 0 una vez. Lee y escribe a la vez, como la ROM.
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

    /// `TIME`: `TIME2ARX` ($E5B4) = comando 3 + `WRITE_2_CLOCK`, y luego
    /// comando 0. Devuelve `ARX[2..7]`.
    fn rom_read(rtc: &mut Pd1990ac) -> [u8; 5] {
        let mut arx = [0; 8];
        rom_mode(rtc, 0x18);
        rom_write2clock(rtc, &mut arx);
        rom_mode(rtc, 0x00);
        let mut out = [0; 5];
        out.copy_from_slice(&arx[2..7]);
        out
    }

    /// `TIME=`: `ARX_2_TIME` ($E59A) = `WRITE_2_CLOCK` con los 5 bytes,
    /// comando 2, comando 0 y PC3 a 1.
    fn rom_set(rtc: &mut Pd1990ac, bytes: [u8; 5]) {
        let mut arx = [0; 8];
        arx[2..7].copy_from_slice(&bytes);
        rom_write2clock(rtc, &mut arx);
        rom_mode(rtc, 0x10);
        rom_mode(rtc, 0x00);
        rtc.write_port(OPC_HIGH | 0x08);
    }

    /// Avanza el reloj emulado `cycles` ciclos.
    fn advance(rtc: &mut Pd1990ac, cycles: u64) {
        let target = rtc.last_timer_state + usize::try_from(cycles).unwrap_or(usize::MAX);
        rtc.sync(target);
    }

    #[test]
    fn read_shifts_counters_lsb_first() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        assert_eq!(rom_read(&mut rtc), [0xC6, 0x31, 0x23, 0x59, 0x58], "lectura");
    }

    #[test]
    fn year_rollover() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        advance(&mut rtc, 3 * FREQ);
        // 1 de enero, 00:00:01, domingo (6 → 0).
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
        // TIME=101912.3045: ARX_2_VAR_12 deja $7A02 = $A0 (mes 10 en
        // hexadecimal, día de la semana 0).
        rom_set(&mut rtc, [0xA0, 0x19, 0x12, 0x30, 0x45]);
        advance(&mut rtc, 2 * FREQ + 100);
        assert_eq!(rom_read(&mut rtc), [0xA0, 0x19, 0x12, 0x30, 0x47], "tras TIME=");
    }

    #[test]
    fn time_set_holds_counters() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        let mut arx = [0, 0, 0xA0, 0x19, 0x12, 0x30, 0x45, 0];
        rom_write2clock(&mut rtc, &mut arx);
        rom_mode(&mut rtc, 0x10);
        advance(&mut rtc, 5 * FREQ);
        assert_eq!(rtc.now().second, 45, "parado con el comando 2");
        // Salir de Time Set empieza un segundo nuevo en ese instante.
        rom_mode(&mut rtc, 0x00);
        advance(&mut rtc, FREQ - 10);
        assert_eq!(rtc.now().second, 45, "a\u{fa}n no ha pasado 1 s");
        advance(&mut rtc, 10);
        assert_eq!(rtc.now().second, 46, "1 s tras soltarlo");
    }

    /// Fija la fecha `month`/`day` a las 23:59:59 y avanza 1 s.
    fn next_day(month: u8, day: u8) -> (u8, u8) {
        let mut rtc = emulated(RtcDateTime { month, day, hour: 23, minute: 59, second: 59, weekday: 0 });
        advance(&mut rtc, FREQ);
        let now = rtc.now();
        (now.month, now.day)
    }

    #[test]
    fn month_lengths_without_leap_years() {
        assert_eq!(next_day(2, 28), (3, 1), "28 feb");
        assert_eq!(next_day(2, 29), (3, 1), "29 feb puesto a mano");
        assert_eq!(next_day(4, 30), (5, 1), "30 abr");
        assert_eq!(next_day(7, 30), (7, 31), "30 jul");
        assert_eq!(next_day(7, 31), (8, 1), "31 jul");
        assert_eq!(next_day(9, 9), (9, 10), "acarreo BCD del d\u{ed}a");
        assert_eq!(next_day(0, 31), (1, 1), "mes 0 (lo admite TIME=)");
    }

    #[test]
    fn long_gaps_skip_whole_days() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        // 3 s + 400 días: 31 dic 23:59:58 → 1 ene 00:00:01 (domingo) →
        // 365 días después otra vez 1 ene (el chip no tiene años, así que
        // tampoco bisiestos) → 35 días más, 5 feb.
        advance(&mut rtc, (400 * SECONDS_PER_DAY + 3) * FREQ);
        let now = rtc.now();
        assert_eq!((now.month, now.day, now.hour, now.minute, now.second), (2, 5, 0, 0, 1), "fecha");
        // 400 días desde un domingo: 400 % 7 = 1 → lunes.
        assert_eq!(now.weekday, 1, "d\u{ed}a de la semana");
    }

    /// Flancos de subida de `TP` en 1 s emulado.
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
        assert_eq!(tp_edges_in_one_second(&mut rtc), 64, "comando 4");
        rom_mode(&mut rtc, 0x28);
        assert_eq!(tp_edges_in_one_second(&mut rtc), 256, "comando 5");
        rom_mode(&mut rtc, 0x30);
        assert_eq!(tp_edges_in_one_second(&mut rtc), 2048, "comando 6");
        rom_mode(&mut rtc, 0x00);
        assert_eq!(tp_edges_in_one_second(&mut rtc), 2048, "el grupo 0 no toca TP");
    }

    #[test]
    fn tp_command_keeps_group_0_mode() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        // Registro = contadores (segundos $58 = %0101_1000), comando 1.
        rom_mode(&mut rtc, 0x18);
        rom_mode(&mut rtc, 0x08);
        // Comando 5 (TP 256 Hz; con C0 = 1, salida habilitada): el modo
        // sigue siendo Register Shift.
        rom_mode(&mut rtc, 0x28);
        assert!(!rtc.data_out(), "bit 0");
        for _ in 0..3 {
            rtc.write_port(OPC_HIGH | 0x28 | 0x04);
            rtc.write_port(OPC_HIGH | 0x28);
        }
        assert!(rtc.data_out(), "bit 3 tras 3 pulsos de CLK");
    }

    #[test]
    fn data_out_is_high_when_output_disabled() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        // Registro a 0 con 40 ceros por DATA IN.
        rom_write2clock(&mut rtc, &mut [0; 8]);
        assert!(!rtc.data_out(), "OUT ENBL = 1: bit 0 = 0");
        // Comando 2 (C0 = 0): DATA OUT en alta impedancia.
        rom_mode(&mut rtc, 0x10);
        assert!(rtc.data_out(), "OUT ENBL = 0");
    }

    #[test]
    fn cpu_reset_does_not_rewind_the_clock() {
        let mut rtc = emulated(NEW_YEAR_EVE);
        rtc.sync(FREQUENCY);
        assert_eq!(rtc.now().second, 59, "1 s");
        // CPU nueva con timer_state desde 0, sin `rebase_timer`.
        rtc.sync(FREQUENCY / 2);
        rtc.sync(FREQUENCY);
        assert_eq!(rtc.now().second, 0, "1.5 s + 0.5 s m\u{e1}s");
    }

    #[test]
    fn host_local_follows_local_time() {
        // Reintento por si justo cambia la hora entre las dos lecturas.
        let matches = (0..2).any(|_| {
            let mut rtc = Pd1990ac::new();
            let now = rtc.now();
            let local = chrono::Local::now();
            (now.hour, now.day, now.month) == (small(local.hour()), small(local.day()), small(local.month()))
        });
        assert!(matches, "la hora del chip es la hora LOCAL del host");
    }

    #[test]
    fn display_format() {
        let dt = RtcDateTime { month: 2, day: 8, hour: 7, minute: 5, second: 3, weekday: 0 };
        assert_eq!(dt.to_string(), "08/02 07:05:03", "formato");
    }
}
