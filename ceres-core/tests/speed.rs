//! Tests de la API de emulación por presupuesto de ciclos
//! ([`Pc1500::run_for_ticks`], [`Pc1500::ticks`]) que usa el planificador
//! de tiempo real de la GUI, del avance rápido de la CPU parada en `HLT`
//! ([`Pc1500::set_fast_halt`]) y de que los ganchos "por frame" (el freno
//! del auto-apagado, el enganche de `DISP`) no dependan de en qué trozos
//! se reparta la emulación. Todo contra la ROM real, con el arnés.
//!
//! Al final hay un benchmark de rendimiento (`#[ignore]`, solo tiene
//! sentido en release):
//!
//! ```text
//! cargo test --release -p ceres-core --test speed -- --ignored --nocapture
//! ```

use std::{fs, path::Path, time::Instant};

use ceres_core::{
    CPU_HZ, Key, LCD_PERSISTENCE_TICKS, Pc1500, TICKS_PER_FRAME,
    harness::KeyStroke,
};

/// Ciclos por repintado de la GUI a 0.02x en un monitor de 60 Hz
/// (`1.3e6 × 0.02 / 60`): el trozo más pequeño que le llega a
/// `run_for_ticks` en uso normal.
const SLOW_CHUNK: usize = 433;

/// Contador de auto-apagado de la ROM, $7B0A-$7B0C.
const APOW_CTR: u32 = 0x7B0A;

/// RAM de usuario estándar más la expansión CE-161 ($0100-$47FF) y la RAM
/// del sistema ($7600-$7FFF): todo lo que puede cambiar la ROM.
const RAM_RANGES: [(u32, usize); 2] = [(0x0100, 0x4700), (0x7600, 0x0A00)];

/// Máximo de frames que se espera a que la ROM vuelva al teclado.
const READY_MAX_FRAMES: usize = 600;

fn booted() -> Pc1500 {
    let mut pc = Pc1500::new_deterministic();
    assert!(pc.boot().is_some(), "la ROM no lleg\u{f3} al prompt");
    pc
}

fn ram(pc: &Pc1500) -> Vec<u8> {
    RAM_RANGES
        .iter()
        .flat_map(|&(addr, len)| pc.read_bytes(addr, len))
        .collect()
}

fn enter_line(pc: &mut Pc1500, line: &str) {
    if let Err(err) = pc.enter_line(line) {
        panic!("{line:?}: {err}; LCD = [{}]", pc.lcd_text());
    }
}

fn type_keys(pc: &mut Pc1500, text: &str) {
    if let Err(c) = pc.type_text(text) {
        panic!("sin tecla: {c:?}");
    }
}

fn wait_ready(pc: &mut Pc1500) {
    assert!(
        pc.run_until_ready(READY_MAX_FRAMES).is_some(),
        "la ROM no volvi\u{f3} al teclado; LCD = [{}]",
        pc.lcd_text()
    );
}

/// Ejecuta exactamente hasta el ciclo `target` en trozos de `chunk`
/// ciclos (el último, más corto). Termina justo en `target` si `target`
/// es una frontera de instrucción de la misma emulación hecha en otros
/// trozos (ver `chunked_run_matches_whole_frames`).
fn run_chunked_until(pc: &mut Pc1500, target: usize, chunk: usize) {
    while pc.ticks() < target {
        let budget = chunk.min(target - pc.ticks());
        pc.run_for_ticks(budget);
    }
}

/// Escribe un `.lh5` en el directorio temporal de los tests y lo carga.
fn load_lh5(pc: &mut Pc1500, name: &str, load_addr: u16, code: &[u8]) {
    let mut file = load_addr.to_le_bytes().to_vec();
    let len = u16::try_from(code.len()).unwrap_or(u16::MAX);
    file.extend_from_slice(&len.to_le_bytes());
    file.extend_from_slice(code);
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if let Err(err) = fs::write(&path, &file) {
        panic!("{}: {err}", path.display());
    }
    if let Err(err) = pc.load_lh5_file(&path) {
        panic!("{}: {err}", path.display());
    }
}

#[test]
fn run_for_ticks_spends_at_least_the_budget() {
    let mut pc = Pc1500::new_deterministic();
    assert_eq!(pc.ticks(), 0);
    assert_eq!(pc.run_for_ticks(0), 0, "presupuesto 0: no ejecuta nada");
    assert_eq!(pc.ticks(), 0);

    let first = pc.run_for_ticks(TICKS_PER_FRAME);
    assert!(
        (TICKS_PER_FRAME..TICKS_PER_FRAME + 64).contains(&first),
        "ran = {first}: como mucho una instrucci\u{f3}n de m\u{e1}s"
    );
    assert_eq!(pc.ticks(), first);

    // Trozos pequeños, también con la CPU parada en `HLT` (el prompt).
    assert!(pc.boot().is_some());
    for budget in [1, 2, 3, 41, 42, 43, SLOW_CHUNK] {
        let before = pc.ticks();
        let ran = pc.run_for_ticks(budget);
        assert!((budget..budget + 64).contains(&ran), "budget {budget}: ran {ran}");
        assert_eq!(pc.ticks() - before, ran);
    }
}

#[test]
fn step_frame_is_run_for_ticks_of_one_frame() {
    let mut a = Pc1500::new_deterministic();
    let mut b = Pc1500::new_deterministic();
    for _ in 0..300 {
        a.step_frame();
        b.run_for_ticks(TICKS_PER_FRAME);
        assert_eq!(a.ticks(), b.ticks());
    }
    assert_eq!(a.lcd_text(), b.lcd_text());
    assert_eq!(a.lcd_symbol_bytes(), b.lcd_symbol_bytes());
    assert_eq!(ram(&a), ram(&b));
}

/// Repartir la misma emulación en trozos de 433 ciclos (0.02x en la GUI)
/// o en frames enteros da exactamente lo mismo, con y sin avance rápido
/// en `HLT`, incluso terminando en el mismo ciclo.
#[test]
fn chunked_run_matches_whole_frames() {
    for fast_halt in [true, false] {
        let mut a = booted();
        let mut b = booted();
        a.set_fast_halt(fast_halt);
        b.set_fast_halt(fast_halt);
        type_keys(&mut a, "12");
        type_keys(&mut b, "12");
        assert_eq!(a.ticks(), b.ticks());

        a.run_frames(400);
        run_chunked_until(&mut b, a.ticks(), SLOW_CHUNK);

        assert_eq!(a.ticks(), b.ticks(), "fast_halt = {fast_halt}");
        assert_eq!(a.lcd_text(), b.lcd_text());
        assert_eq!(a.lcd_symbol_bytes(), b.lcd_symbol_bytes());
        assert_eq!(ram(&a), ram(&b), "fast_halt = {fast_halt}");
        assert_eq!(a.cpu().p(), b.cpu().p());
    }
}

/// Una "foto" del estado visible tras un paso de `halt_script`: ciclos,
/// LCD, indicadores y PC.
type Shot = (usize, String, [u8; 2], u16);

/// Guion que ejercita la CPU parada en `HLT` de todas las formas que se
/// le ocurren a un usuario: arranque en frío, tecleo y edición con
/// autorrepetición de las flechas, un programa con `WAIT` (que cuenta
/// pulsos del reloj `µPD1990AC` en PB5), BREAK con `ON` (IR2 desde el
/// LH5810, el único despertar que no es el temporizador) y `OFF`/`ON`.
/// Devuelve una "foto" (ciclos, LCD, indicadores, PC) tras cada paso y la
/// RAM final.
fn halt_script(fast_halt: bool) -> (Vec<Shot>, Vec<u8>) {
    let mut pc = Pc1500::new_deterministic();
    pc.set_fast_halt(fast_halt);
    assert_eq!(pc.fast_halt(), fast_halt, "interruptor");
    let mut shots = Vec::new();
    let mut shot = |machine: &Pc1500| {
        shots.push((
            machine.ticks(),
            machine.lcd_text(),
            machine.lcd_symbol_bytes(),
            machine.cpu().p(),
        ));
    };

    let boot_frames = pc.boot();
    assert!(boot_frames.is_some(), "la ROM no lleg\u{f3} al prompt");
    shot(&pc);

    enter_line(&mut pc, "NEW");
    enter_line(&mut pc, "10 A=1");
    enter_line(&mut pc, "LIST");
    shot(&pc);

    type_keys(&mut pc, "ABCDEFGH");
    // 4 frames: un solo paso del cursor; 100: autorrepetición de la ROM.
    pc.tap_key_for(Key::Left, 4, 4);
    type_keys(&mut pc, "X");
    wait_ready(&mut pc);
    shot(&pc);
    pc.tap_key_for(Key::Left, 100, 4);
    type_keys(&mut pc, "Y");
    wait_ready(&mut pc);
    shot(&pc);

    // BREAK en el prompt: borra la línea.
    pc.tap_key_for(Key::On, 1, 0);
    wait_ready(&mut pc);
    shot(&pc);

    enter_line(&mut pc, "NEW");
    enter_line(&mut pc, "10 WAIT 30:PRINT \"HI\"");
    enter_line(&mut pc, "20 GOTO 10");
    pc.tap(KeyStroke::plain(Key::Mode));
    wait_ready(&mut pc);
    type_keys(&mut pc, "RUN\n");
    pc.run_frames(200);
    shot(&pc);
    pc.tap_key_for(Key::On, 1, 0);
    wait_ready(&mut pc);
    shot(&pc);

    pc.tap_key_for(Key::Off, 4, 4);
    pc.run_frames(20);
    assert!(!pc.is_powered_on(), "OFF no apag\u{f3}");
    shot(&pc);
    assert!(pc.power_on().is_some(), "ON no encendi\u{f3}");
    shot(&pc);
    pc.run_frames(300);
    shot(&pc);

    (shots, ram(&pc))
}

/// El avance rápido en `HLT` no cambia nada de la emulación: el mismo
/// guion da los mismos ciclos, LCD, indicadores, PC y RAM en cada paso.
#[test]
fn fast_halt_is_equivalent_to_two_state_steps() {
    let (fast_shots, fast_ram) = halt_script(true);
    let (slow_shots, slow_ram) = halt_script(false);
    assert_eq!(fast_shots.len(), slow_shots.len());
    for (i, (fast, slow)) in fast_shots.iter().zip(&slow_shots).enumerate() {
        assert_eq!(fast, slow, "paso {i}");
    }
    assert!(fast_ram == slow_ram, "la RAM final difiere");

    // Y el guion de verdad hace algo (no compara dos máquinas colgadas).
    assert!(fast_shots[2].1.starts_with("ABCDEFGX"), "LCD = [{}]", fast_shots[2].1);
    assert!(fast_shots[4].1.trim_end() == ">", "LCD = [{}]", fast_shots[4].1);
    assert!(fast_shots[6].1.trim_start().starts_with("BREAK IN 10"), "LCD = [{}]", fast_shots[6].1);
}

/// El avance rápido se conserva con `reset()` (es una opción del
/// emulador) y `ticks()` vuelve a 0.
#[test]
fn reset_keeps_fast_halt_option() {
    let mut pc = booted();
    pc.set_fast_halt(false);
    pc.reset();
    assert!(!pc.fast_halt());
    assert_eq!(pc.ticks(), 0);
    assert!(Pc1500::new().fast_halt(), "activado por defecto");
}

/// El freno del auto-apagado es un gancho por frame emulado: funciona
/// igual cuando la GUI emula en trozos pequeños con `run_for_ticks`.
#[test]
fn auto_off_hold_works_with_small_chunks() {
    let mut pc = booted();
    pc.set_auto_power_off(false);
    for addr in APOW_CTR..APOW_CTR + 3 {
        pc.write_byte(addr, 0xFF);
    }
    let target = pc.ticks() + 200 * TICKS_PER_FRAME;
    run_chunked_until(&mut pc, target, SLOW_CHUNK);
    assert!(pc.is_powered_on(), "no deber\u{ed}a auto-apagarse");
    assert_eq!(pc.read_byte(APOW_CTR), 0xFE, "contador devuelto al inicio");

    // Activado, la ROM sí se apaga, también en trozos.
    pc.set_auto_power_off(true);
    for addr in APOW_CTR..APOW_CTR + 3 {
        pc.write_byte(addr, 0xFF);
    }
    let deadline = pc.ticks() + 500 * TICKS_PER_FRAME;
    while pc.is_powered_on() && pc.ticks() < deadline {
        pc.run_for_ticks(SLOW_CHUNK);
    }
    assert!(!pc.is_powered_on(), "deber\u{ed}a auto-apagarse");
}

/// A 0.02x la GUI mira el LCD cada 433 ciclos emulados, menos que el
/// pulso `RDP`...`SDP` del bucle ocioso de teclado (2090 ciclos, cada
/// ~6.3 millones en el prompt): con un enganche de `DISP` medido entre
/// llamadas a `display()` el LCD se habría visto apagado varios
/// repintados seguidos en cada pulso. Con la persistencia en ciclos
/// emulados no se apaga nunca.
#[test]
fn lcd_never_blanks_at_slow_motion() {
    let mut pc = booted();
    let mut windows_without_disp = 0;
    let mut window_start = pc.ticks();
    let mut disp_seen = pc.cpu().display_enabled();
    let end = pc.ticks() + 500 * TICKS_PER_FRAME;
    while pc.ticks() < end {
        // Una instrucción (o un paso de `HLT`) cada vez, para ver `DISP`
        // tras cada una como lo ve el enganche.
        pc.run_for_ticks(1);
        disp_seen |= pc.cpu().display_enabled();
        if pc.ticks() - window_start >= SLOW_CHUNK {
            if !disp_seen {
                windows_without_disp += 1;
            }
            assert!(pc.display().is_on(), "LCD apagado en el ciclo {}", pc.ticks());
            window_start = pc.ticks();
            disp_seen = pc.cpu().display_enabled();
        }
    }
    assert!(
        windows_without_disp > 0,
        "ninguna ventana de {SLOW_CHUNK} ciclos cay\u{f3} entera dentro del pulso RDP"
    );
}

/// Un programa que apaga el LCD de verdad (`RDP` en bucle) sí se ve
/// apagado una vez pasada la persistencia, y no antes.
#[test]
fn lcd_persistence_is_measured_in_emulated_ticks() {
    let mut pc = booted();
    // `RIE` (FD BE), `RDP` (FD C0), `BCH -2` (9E 02): bucle infinito.
    load_lh5(&mut pc, "speed_rdp_loop.lh5", 0x0100, &[0xFD, 0xBE, 0xFD, 0xC0, 0x9E, 0x02]);
    pc.run_for_ticks(20);
    assert!(!pc.cpu().display_enabled());
    let off_since = pc.ticks();
    pc.run_for_ticks(LCD_PERSISTENCE_TICKS / 2);
    assert!(pc.display().is_on(), "todav\u{ed}a dentro de la persistencia");
    while pc.ticks() < off_since + LCD_PERSISTENCE_TICKS {
        pc.run_for_ticks(SLOW_CHUNK);
    }
    assert!(!pc.display().is_on());
    assert!(!pc.display().is_on(), "segunda muestra sin emular");
}

/// Velocidad de emulación en el host, como múltiplo del tiempo real
/// (×100, para no usar coma flotante en el núcleo).
fn speed_x100(ticks: usize, micros: u128) -> u128 {
    let ticks = u128::try_from(ticks).unwrap_or(u128::MAX);
    let hz = u128::try_from(CPU_HZ).unwrap_or(1);
    ticks * 100_000_000 / (hz * micros.max(1))
}

fn bench(pc: &mut Pc1500, frames: usize) -> (u128, u128) {
    let start_ticks = pc.ticks();
    let start = Instant::now();
    for _ in 0..frames {
        pc.step_frame();
    }
    let micros = start.elapsed().as_micros();
    let x100 = speed_x100(pc.ticks() - start_ticks, micros);
    (x100, micros)
}

fn print_bench(label: &str, frames: usize, (x100, micros): (u128, u128)) {
    let fps = u128::try_from(frames).unwrap_or(0) * 1_000_000 / micros.max(1);
    let mhz_x100 = x100 * u128::try_from(CPU_HZ).unwrap_or(0) / 1_000_000;
    println!(
        "{label:<32} {frames} frames en {micros} \u{b5}s: {fps} frames/s = x{}.{:02} tiempo real = {}.{:02} MHz",
        x100 / 100,
        x100 % 100,
        mhz_x100 / 100,
        mhz_x100 % 100,
    );
}

/// Benchmark (solo release): prompt ocioso con y sin avance rápido en
/// `HLT` y un bucle activo de la CPU (`NOP` / `BCH -3`).
#[test]
#[ignore = "benchmark: ejecutar en release con --ignored --nocapture"]
fn throughput_benchmark() {
    const FRAMES: usize = 3000;

    let mut idle_fast = booted();
    let fast = bench(&mut idle_fast, FRAMES);
    print_bench("ocioso, avance r\u{e1}pido en HLT", FRAMES, fast);

    let mut idle_slow = booted();
    idle_slow.set_fast_halt(false);
    let slow = bench(&mut idle_slow, FRAMES);
    print_bench("ocioso, HLT de 2 en 2", FRAMES, slow);

    let mut busy = booted();
    // $4000: `NOP` (38), `BCH -3` (9E 03).
    load_lh5(&mut busy, "speed_busy_loop.lh5", 0x4000, &[0x38, 0x9E, 0x03]);
    let active = bench(&mut busy, FRAMES);
    print_bench("bucle activo NOP/BCH", FRAMES, active);

    // Holgado: el portátil de referencia da x465-x511 en ocioso con el
    // avance rápido y x58-x132 sin él, con mucho ruido por el escalado
    // de frecuencia.
    assert!(fast.0 > slow.0, "el avance r\u{e1}pido deber\u{ed}a ganar en ocioso");
    assert!(fast.0 >= 150 * 100, "ocioso con avance r\u{e1}pido por debajo de x150");
}
