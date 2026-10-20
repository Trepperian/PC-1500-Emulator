# Nuevas funcionalidades del emulador PC-1500: resumen de cambios

Este documento reúne **todo lo nuevo y lo modificado** en esta tanda de trabajo: qué hace cada funcionalidad, cómo se usa, qué ficheros toca, qué tests la cubren y qué es el *harness* de pruebas. Nada de esto está commiteado todavía; todo está en el árbol de trabajo sobre el commit `e1c45f1`.

> **Estado:** los 9 pasos (0 a 8) están implementados, y cada uno ha pasado sus dos revisiones independientes. La revisión final del conjunto confirma que **no falta nada de lo pedido**. Se ha arrancado la interfaz real con una captura automática: el LCD gris, los indicadores, la carátula, el tecleo y la velocidad real x1.00 a 1,30 MHz funcionan. Quedan las pruebas manuales de interacción de §7.

---

## 1. Resumen

| # | Funcionalidad | Estado | Dónde se nota |
|---|---|---|---|
| 0 | Preparación: harness más fiable y logs silenciados | ✅ | Tests, consola |
| 1 | **BREAK** (tecla ON/BRK) y OFF, auto-apagado y encendido reales | ✅ | `Esc` corta un programa; OFF apaga |
| 2 | **LCD gris STN** e indicadores (PRO, RUN, DEG…) correctos | ✅ | Pantalla |
| 3 | **TIME** (reloj µPD1990AC reescrito) | ✅ | `TIME`, `TIME=`, `WAIT`, `RANDOM` |
| 4 | **Velocidad** más allá de 2x, hasta lo que dé el host | ✅ | Deslizador hasta 1000x y botón Máx |
| 5 | **Teclado completo** del PC a la PC-1500 | ✅ | Todas las teclas y cualquier distribución |
| 6 | **Cargar y guardar BASIC** (ASCII, tokenizado, CE-158) desde la interfaz | ✅ | Panel "Programa BASIC" |
| 7 | **CLOAD / CSAVE / MERGE / CHAIN** dentro del BASIC emulado | ✅ | Comandos BASIC con archivos del PC |
| 8 | **Carátula procedural** de la PC-1500 | ✅ | Vista principal |

Cifras: **223 tests en verde** (115 de integración y 108 unitarios) y 1 benchmark ignorado a propósito. Sin warnings de compilación, tanto en dev como en release.

---

## 2. Mapa de ficheros

### Nuevos

| Fichero | Para qué |
|---|---|
| `ceres-core/src/harness.rs` | Arnés de pruebas: arrancar la ROM sin ventana, pulsar teclas y leer el LCD como texto (ver §4) |
| `ceres-core/src/key_feeder.rs` | Alimentador de teclas: el único planificador de pulsaciones, que cuenta el tiempo en ticks emulados |
| `ceres-core/src/basic.rs` | Tokenizador y destokenizador (tabla de palabras clave leída de la ROM), formatos ASCII, tokenizado y CE-158, y carga y guardado en la RAM |
| `ceres-core/src/host_io.rs` | CLOAD/CSAVE/MERGE/CHAIN: tabla de palabras clave virtual en `$9800` y "trampas" que ejecutan la E/S de archivos en Rust |
| `ceres-core/tests/*.rs` | 8 ficheros de tests de integración (ver §5) |
| `ceres-core/tests/data/` | `mole`, `jackpot`, `bathyscaph` en `.bas` y `.bin`, la verdad de referencia del tokenizador |
| `ceres-egui/src/lcd_view.rs` | Pintado procedural del LCD STN gris con indicadores dentro del cristal |
| `ceres-egui/src/pacing.rs` | `Pacer`: ritmo en tiempo real, acumulador, presupuesto por fotograma, modo Máx y medición de la velocidad |
| `ceres-egui/src/host_keymap.rs` | Traducción de eventos de teclado de egui a acciones de la PC-1500, tabla de teclas especiales y textos de ayuda |
| `ceres-egui/src/basic_io.rs` | Lógica del panel BASIC: elección de formato, carga en cola y argumento de línea de comandos |
| `ceres-egui/src/faceplate.rs` | Carátula de la PC-1500 dibujada por código, con teclas clicables |

### Modificados

| Fichero | Cambio principal |
|---|---|
| `ceres-core/src/lib.rs` | `run_for_ticks`, ganchos por fotograma, `reset()` con lista de lo que sobrevive, PB7 (BREAK), reloj (OPB5/OPB6), auto-apagado y API pública nueva |
| `ceres-core/src/lh5801.rs` | Estado apagado (`bf`), `power_on`, eliminado `timer_state = 0` en el reset (provocaba un *underflow*), avance rápido en `HLT` y gancho de trampas para CLOAD/CSAVE |
| `ceres-core/src/keyboard.rs` | Tabla `MATRIX` (misma lógica que antes, verificada contra la ROM), `Key::ALL`, etiquetas, pulso ON, eliminado `Key::Quote` (no existía), y aquí viven ahora `KeyStroke`/`key_for_char` |
| `ceres-core/src/display.rs` | Indicadores con la polaridad correcta y por tabla (DE, G y RAD separados), columnas, `is_on()` y paleta gris |
| `ceres-core/src/pd1990ac.rs` | Reescritura completa del reloj |
| `ceres-core/src/memory.rs` | `LOG_UNMAPPED` (silencia los `println!`), lectura de la tabla virtual `$9800` y `read_banked`. **Tu línea `include_bytes!("../../mole.bin")` está intacta.** |
| `ceres-egui/src/pc1500_app.rs` | Interfaz nueva: velocidad, teclado, paneles BASIC y CLOAD/CSAVE, ayuda de teclado, LCD y casillas |
| `ceres-egui/src/main.rs` | Módulos nuevos, ventana de 1280×760 y el argumento de línea de comandos se enruta por extensión |

---

## 3. Cada funcionalidad en detalle

### 3.1 BREAK, y OFF y ON reales (paso 1)

**Qué fallaba.** En la PC-1500 real, la tecla ON/BRK no está en la matriz del teclado: va a la patilla **PB7** del LH5810, que la ROM consulta en `CHK_BRK` (`$E451`, `BII #$F00B,$02`). El emulador nunca activaba PB7, así que BREAK no podía funcionar.

**Qué se ha hecho.**
- ON genera un **pulso único** en PB7 en el flanco de pulsación, como `CheckKon` de PockEmul. Si fuera un nivel, la ROM vería un segundo BREAK al mantener la tecla y borraría el mensaje `BREAK IN nn`.
- **OFF real.** La instrucción `OFF` (FD 4C) apaga de verdad: pantalla en blanco, CPU parada y RAM conservada.
- **ON con la máquina apagada** reinicia la CPU, y la ROM restaura la sesión con su firma en `$7A10`.
- **Auto-apagado** fiel (unos 5 minutos sin teclas a 1x), **activado por defecto**, con la casilla **"Auto-apagado"** para desactivarlo. La casilla mantiene el contador `$7B0A`-`$7B0C` lejos del desbordamiento.
- Corregido un fallo latente: el reset ponía `timer_state = 0`, y en el siguiente reset las restas de temporización hacían *underflow*.

**Uso.** `Esc` (o F12, Fin, Ctrl+C) es BREAK. `Ctrl+F12` es OFF. Con la máquina apagada se ve el aviso "Apagada — pulsa ON (Esc)".

**API.** `Pc1500::is_powered_on()`, `set_auto_power_off(bool)`, `auto_power_off()`. Harness: `power_on()`.

### 3.2 LCD gris e indicadores (paso 2)

**Qué fallaba.** `display.rs` leía los indicadores **al revés**: un bit a 1 significa encendido. Además asignaba RAD dos veces y encendía siempre la batería.

**Qué se ha hecho.**
- Tabla de bits sacada de la ROM:
  - `$764E`: BUSY, SHIFT, KANA, SMALL, III, II, I, DEF.
  - `$764F`: DE, G, RAD, RESERVE, PRO, RUN.
- **DEG = DE+G** y **GRAD = G+RAD**, como en el cristal real.
- Pintado procedural del LCD (`lcd_view.rs`): fondo gris STN, píxeles gris oscuro y "fantasma" opcional de los píxeles apagados. Los indicadores van **dentro del cristal**, en el mismo color y en su posición real, con una fuente propia de 3×5.
- La ROM apaga la pantalla unos 1,3 ms en cada vuelta del bucle de teclado. Ahora hay persistencia medida en ticks emulados, así que la pantalla ya no parpadea.

**Uso.** Casillas "Fantasma LCD" y "LCD ampliado".

### 3.3 TIME (paso 3)

**Qué fallaba.** El reloj no implementaba el modo 3 (*Time Read*), nunca avanzaba la hora, usaba UTC y desplazaba los bits en un orden distinto al que espera la rutina `WRITE_2_CLOCK` de la ROM. `TIME=` se perdía.

**Qué se ha hecho.**
- Reescritura del µPD1990AC según su hoja de datos:
  - registro de desplazamiento de 40 bits;
  - comandos 0-3 y frecuencias TP (4-7) memorizados por separado en el flanco de STB;
  - desplazamiento en el flanco de CLK;
  - salida OE en PC3.
- La hora viene del **reloj local del PC**, avanza en tiempo real a cualquier velocidad y **sobrevive al botón Reset**.
- El cambio de día y de mes es en BCD, sin años bisiestos, igual que el chip real.
- TP (64/256/2048 Hz) va en tiempo emulado, así que `WAIT`, `BEEP` y las pausas de `PRINT` siguen la velocidad de emulación.

**Uso.** `TIME` devuelve `MMDDHH.MMSS`. `TIME=101912.3045` pone la hora. Un mes inválido da `ERROR 23`. La interfaz muestra "Reloj: dd/mm hh:mm:ss".

**API.** `RtcDateTime`, `RtcTimeBase::{HostLocal, Emulated}`, `Pc1500::set_rtc_time_base`, `rtc_now`, `reset()`.

### 3.4 Velocidad (paso 4)

**Qué fallaba.** La interfaz ejecutaba como mucho un `step_frame()` (11,5 ms emulados) por repintado. Con un monitor de 60 Hz, el máximo real era **0,69x**, así que incluso "1x" iba lento.

**Qué se ha hecho.**
- Núcleo:
  - `run_for_ticks(presupuesto)`, del que ahora depende `step_frame()`. Todos los ganchos por fotograma se miden en ticks emulados.
  - **Avance rápido en `HLT`.** Cuando la CPU duerme, salta directamente al siguiente límite de 42 estados del temporizador. Hay un test que demuestra que el resultado es idéntico.
- Interfaz, con `Pacer`:
  - un acumulador de tiempo con presupuesto por fotograma, para que la interfaz no se congele;
  - si el PC no da abasto, descarta el tiempo pendiente en vez de acumularlo;
  - un tope al volver de una ventana minimizada.
- Rendimiento medido en release:
  - **unas x396 en reposo** (antes unas x83);
  - unas **x159 en un bucle activo**.

**Uso.** Deslizador logarítmico **0,02x-1000x**, botones de valores predefinidos, botón **"⏩ Máx"** y etiqueta con la velocidad real medida y los MHz emulados. Aviso "limitado por el host" cuando no se alcanza la velocidad pedida.

> Ojo: en modo Máx con el auto-apagado activado, la máquina en reposo se apaga en segundos (es el comportamiento real, acelerado). Desactiva "Auto-apagado" si molesta.

### 3.5 Teclado completo (paso 5)

**Qué fallaba.** El Shift del PC mantenía pulsado el SHIFT de la PC-1500, pero en la máquina real SHIFT es un **prefijo**: se pulsa y se suelta antes de la siguiente tecla. Por eso fallaba todo carácter con Shift. Además, el mapeo dependía de la distribución US: en un teclado español, `(` llegaba como `8`.

**Qué se ha hecho.**
- Los caracteres imprimibles llegan como **eventos de texto**, así que funciona cualquier distribución. La lista exacta de caracteres admitidos está en §4.
- El **alimentador de teclas** (`key_feeder.rs`) mide las pulsaciones en tiempo emulado y añade solo el SHIFT de prefijo cuando hace falta.
- Pulsación según la velocidad:
  - a ≤1x la tecla se mantiene mientras la mantienes, así que sirve para juegos con `INKEY$`;
  - a >1x cada pulsación es un toque de duración fija.
- **Ctrl+V** teclea el portapapeles, y "Cancelar tecleo" lo detiene.
- La interfaz no se roba teclas: Tab y Espacio no activan botones, y lo que escribes en un campo de ruta no llega al emulador.
- Ventana de ayuda **"⌨ Teclado"**, generada con las mismas tablas que usa el mapeo.

**Teclas especiales:**

| PC | PC-1500 |
|---|---|
| Esc, F12, Fin, Ctrl+C | ON/BRK (BREAK) |
| Ctrl+F12 | OFF |
| Inicio / Shift+Inicio | CL / CA |
| Intro | ENTER |
| Retroceso | borrar a la izquierda (← y luego DEL) |
| Supr / Insert | DEL / INS |
| Flechas, Re Pág, Av Pág | cursores |
| Shift+↑ / Shift+↓ | `[` / `]` (en el LCD se ven como √ y π) |
| Tab / Shift+Tab | MODE (RUN/PRO) / RESERVE |
| F1-F6, Shift+F1-F6 | teclas reservadas / `! " # $ % &` |
| F7 | DEF |
| F8 | SML (minúsculas) |
| F9 | RSV |
| F10 | RCL |
| F11 | SHIFT de la PC-1500 (prefijo) |

Las letras salen en **mayúsculas** por defecto. Con la casilla **"Distinguir mayúsculas/minúsculas"** se respeta el caso del PC. La casilla **"Capturar teclado del PC"** activa o desactiva la captura.

### 3.6 Cargar y guardar BASIC desde la interfaz (paso 6)

**Qué se ha hecho.**
- **Tokenizador**, copia fiel de la rutina `TOK_INBUF` de la ROM.
  - Las palabras clave se **leen de la propia ROM**, recorriendo las tablas en el mismo orden que la ROM, así que incluye la tabla virtual de CLOAD/CSAVE.
  - Validado tecleando **3.596 líneas** del corpus en la ROM real: 0 diferencias de bytes.
- **Formatos:**
  - ASCII (`10 PRINT` o `10:PRINT`, con finales de línea CRLF, CR o LF);
  - volcado tokenizado sin cabecera (`.bin`, como `mole.bin`);
  - tokenizado con la **cabecera CE-158** de 27 bytes, tal como la escribe el CE-158 real o los ficheros de Jeff Birt.
  - La detección del formato es automática.
- **Carga en la RAM:** actualiza los mismos punteros de sistema que la ROM (`BASPRG_ST`/`END`/`EDT`, `VAR_START`, puntero de DATA…), de modo que después funcionan `LIST`, `RUN`, editar y añadir líneas.
- **Arreglado tu fixture de arranque.** `mole.bin` se cargaba sin el `0xFF` final y sin `VAR_START`, y por eso **editar una línea tras arrancar corrompía el programa**. El arreglo está en `Pc1500::new()`, **sin tocar `MemoryBus::new()` ni tu línea**.

**Uso.** Panel "Programa BASIC":
- campo de ruta;
- formato: Auto, ASCII CRLF, ASCII CR (`.asc`), Tokenizado (`.bin`) o CE-158 (`.bas`);
- botones "Cargar BASIC" y "Guardar BASIC";
- también se puede **arrastrar un fichero** a la ventana o pasarlo por **línea de comandos**: `cargo run -- programa.bas`.
- Si la máquina no está en el prompt, la carga espera en cola.

Al guardar, la extensión decide el formato:

| Extensión | Formato |
|---|---|
| `.bas` | CE-158 (tokenizado con cabecera) |
| `.asc` | ASCII con CR |
| `.txt` | ASCII con CRLF |
| `.bin` | tokenizado sin cabecera |

**API.** `basic::{KeywordTable, Keywords, Program, detect_format, …}` y `Pc1500::{is_at_basic_prompt, load_basic_program, load_basic_file, save_basic_file, basic_program, basic_program_bytes}`.

### 3.7 CLOAD, CSAVE, MERGE y CHAIN en el BASIC emulado (paso 7)

**Cómo funciona.** Hay una pequeña tabla de palabras clave virtual en `$9800`, que la ROM descubre sola al arrancar. Usa los **tokens reales**: CLOAD=`F089`, CSAVE=`F095`, MERGE=`F08F`, CHAIN=`F0B2`. Por eso los programas que ya contienen estos comandos se listan y se ejecutan bien. Cuando la CPU llega al manejador, el emulador hace la E/S de archivos en Rust (reutilizando `basic.rs`) y devuelve el control a la ROM, o muestra `ERROR n`.

| Comando | Efecto |
|---|---|
| `CSAVE "X"` | `X.BAS`, tokenizado con cabecera CE-158, como el CE-158 real |
| `CSAVEa "X"` | `X.ASC`, en texto ASCII |
| `CSAVE "X.BIN"` / `"X.TXT"` | tokenizado sin cabecera / texto CRLF |
| `CLOAD "X"` | detecta solo el formato (ASCII, tokenizado o CE-158). **Solo en modo directo** |
| `CLOAD? "X"` | compara con lo que hay en memoria (`ERROR 43` si no coincide) |
| `MERGE "X"` | añade al final del programa actual (solo en modo directo) |
| `CHAIN "X"[,línea]` | carga y ejecuta conservando las variables (dentro de programas) |
| `CLOAD` / `CSAVE` sin nombre | usa el fichero "Cinta" de la interfaz; sin él, `ERROR 40` |

Errores, como en el hardware real:

| Error | Causa |
|---|---|
| 27 | no hay carpeta configurada |
| 61 | el fichero no existe |
| 40 | nombre no válido |
| 42 | el programa no cabe |
| 44 | error de lectura |
| 67 | línea ASCII sin número |

Por seguridad, los nombres con `/`, `..` o que empiezan por `.` se rechazan.

**Uso.** En la interfaz: "Carpeta CLOAD/CSAVE" (por defecto, la carpeta actual) y "Cinta".

**API.** `Pc1500::{set_host_dir, host_dir, set_tape, tape, take_host_events}`, `HostEvent` y `HostCommand`.

### 3.8 Carátula (paso 8)

Carátula de la PC-1500 original (franja superior bronce) **dibujada por código**, sin fotos ni dependencias nuevas:
- panel de aluminio cepillado;
- las 65 teclas en su posición y color reales, con las leyendas de SHIFT;
- el LCD de §3.2 integrado;
- teclas clicables y mantenibles, con su tecla del PC en el *tooltip*.

Sustituye a la botonera genérica actual.

---

## 4. El harness de pruebas: qué es y qué hace

**Qué es.** Un módulo público, `ceres_core::harness` (`ceres-core/src/harness.rs`), que permite usar la PC-1500 emulada **sin ventana**, desde un test, igual que la usaría una persona: arranca la ROM real, pulsa teclas y lee lo que sale en la pantalla.

**Para qué sirve.** Es lo que permite comprobar automáticamente cada funcionalidad contra la **ROM A04 real**, en lugar de fiarse de lo que "debería" pasar. Por ejemplo: teclear `RUN`, pulsar ON y comprobar que el LCD dice `BREAK IN 10`.

**Cómo funciona por dentro.**
- **Tiempo determinista.** Todo se mide en *fotogramas emulados* (15.000 ciclos, unos 11,5 ms). `Pc1500::new_deterministic()` fija además el reloj en una hora conocida, para que `TIME` y `RANDOM` den siempre lo mismo.
- **Pulsar teclas.** Cada tecla se mantiene 4 fotogramas y se suelta otros 4. Lo midió barriendo valores: por debajo de "soltar ≥2 y mantener+soltar ≥6", la ROM pierde teclas. Los caracteres con SHIFT se teclean como en la máquina real: primero un toque de SHIFT y después la tecla.
- **Saber cuándo la ROM está lista.** `is_waiting_for_key()` comprueba que la CPU está dormida en el bucle de teclado `WAIT_4_KB` (`$E243`-`$E33F`). `is_ready_for_key()` comprueba además que la ROM ya ha visto soltar la tecla anterior (bit 0 de `$7B0E`). Este detalle se descubrió en el paso 0: sin él, la primera tecla de la línea siguiente se perdía (`200` acababa en `00`).
- **Leer el LCD como texto.** Decodifica la RAM de pantalla (`$7600`/`$7700`) en 156 columnas y compara cada celda de 6 columnas con la **fuente 5×7 de la propia ROM** (`$FCA0`). Así obtiene los 26 caracteres de la pantalla.

**Funciones principales:**

| Función | Qué hace |
|---|---|
| `Pc1500::new_deterministic()` | Máquina nueva con reloj fijo (para tests) |
| `boot()` | Arranca en frío, pulsa CL y deja el prompt `>` en modo PRO. Conserva el programa `mole.bin` |
| `run_frames(n)` / `run_until(n, cond)` | Emula n fotogramas, o hasta que se cumple una condición |
| `run_until_idle(n)` / `run_until_ready(n)` | Espera a que la ROM espere una tecla / a que esté lista para la siguiente |
| `tap(KeyStroke)`, `tap_key_for(key, mantener, soltar)` | Pulsa una tecla, con SHIFT de prefijo si hace falta |
| `type_text("...")` | Teclea texto (`\n` = ENTER) |
| `enter_line("10 PRINT 5")` / `enter_lines(...)` | Teclea líneas completas de forma fiable, esperando a la ROM antes y después |
| `power_on()` | Pulsa ON y espera a que la ROM esté lista para la siguiente tecla |
| `lcd_text()` / `lcd_cells()` / `lcd_columns()` | Pantalla como texto / por celdas / por columnas de píxeles |
| `lcd_symbol_bytes()` | Bytes crudos de los indicadores (`$764E`, `$764F`) |
| `read_bytes(dir, n)`, `read_word_be(dir)` | Leer memoria |
| `key_for_char(c)`, `text_to_keystrokes(s)` | Qué tecla, con o sin SHIFT, produce cada carácter |

**Ejemplo real (simplificado) de un test:**

```rust
let mut pc = Pc1500::new_deterministic();
pc.boot();                                     // prompt ">"
pc.type_text("NEW\n").unwrap();
pc.enter_line("10 GOTO 10").unwrap();          // programa en bucle infinito
pc.tap(KeyStroke::plain(Key::Mode));           // pasa a modo RUN
pc.type_text("RUN\n").unwrap();
pc.run_frames(30);
pc.press(Key::On); pc.release(Key::On);        // BREAK
pc.run_until_idle(500);
assert!(pc.lcd_text().contains("BREAK IN 10"));
assert_eq!(pc.read_word_be(0x78AE), 10);       // BRK_LINE de la ROM
```

**Caracteres que sabe teclear:**
- **Sin SHIFT:** A-Z, 0-9, espacio y `. - + * / = ( )`.
- **Con SHIFT de prefijo:** `! " # $ % &` (F1-F6), `,` `<` `>` `?` `:` `;` `@` `^` `[` `]` y las minúsculas.
- **Imposibles**, porque no hay tecla: `' _ \` { | } ~ \`.

---

## 5. Los tests

**Cómo ejecutarlos:** `cargo test --workspace`, unos segundos en total. El test de rendimiento está marcado con `#[ignore]`; se lanza con `cargo test --release -p ceres-core --test speed -- --ignored --nocapture`.

### Tests de integración (`ceres-core/tests/`), todos contra la ROM real

| Fichero | Nº | Qué comprueba |
|---|---|---|
| `harness.rs` | 13 | El propio harness: arranque (`NEW0? :CHECK`, prompt `>` en PRO), `PRINT 1+2` → `3`, guardar y listar líneas, ejecutar en modo RUN, que se puedan teclear todos los caracteres, que no se pierdan teclas repetidas ni líneas largas, y que `:` solo separa sentencias dentro de un programa (comportamiento real de la ROM) |
| `break.rs` | 12 | BREAK durante `RUN`, con `CONT`, manteniendo ON, en `INPUT` y en `WAIT`; ON en el prompt borra la línea; OFF apaga y ON reinicia conservando el programa; ON con la máquina apagada no lanza además un BREAK; auto-apagado, reanudación y desactivación; cargar un `.lh5` con la máquina apagada |
| `display_symbols.rs` | 12 | Indicadores al arrancar (I, DEG, PRO), MODE RUN/PRO, RESERVE, SHIFT/DEF, SML, DEGREE/RADIAN/GRAD, niveles I/II/III, BUSY durante la ejecución, píxeles idénticos al harness, que la pantalla no parpadea en reposo, y apagado en blanco |
| `time.rs` | 8 | `TIME` lee el reloj en marcha; `TIME=` y leer 5 s después; 28 feb → 1 mar; fin de año; mes 13 → `ERROR 23`; Reset conserva la hora; `TIME_DELAY` dura 1 s por cada 64 unidades; `WAIT 64` |
| `speed.rs` | 9 | `run_for_ticks` gasta como mínimo el presupuesto; `step_frame` es lo mismo que un fotograma; ejecutar a trozos da el mismo resultado; el avance rápido en `HLT` es equivalente; el auto-apagado funciona con trozos pequeños; el LCD no parpadea a cámara lenta; benchmark (ignorado) |
| `keyboard_mapping.rs` | 19 | La `MATRIX` coincide con las tablas de la ROM (`$FE80`/`$FEC0`) y está completa; todo carácter se puede teclear o se sabe por qué no; mantener SHIFT no escribe nada (como en la real); alimentador: toques cortos, ráfagas, rollover, autorrepetición, SHIFT memorizado, pegado exacto con SML, pegar 20 líneas, liberar todo y prioridad de BREAK |
| `basic.rs` | 21 | Tabla de palabras clave leída de la ROM (85 entradas); reglas del tokenizador; el corpus `.bas` → `.bin` coincide byte a byte; idas y vueltas; cabecera CE-158; detección de formato; lo tecleado en la ROM coincide con el tokenizador del PC; fixture arreglado; cargar → `LIST` → editar → `RUN`; `CONT` tras cargar; "no está en el prompt"; demasiado grande |
| `host_files.rs` | 22 | La tabla virtual se tokeniza y se lista; `ERROR 27` sin carpeta; `CSAVE` exacto (`mole.bin`); formato por defecto CE-158; sufijo `a` y extensiones; `CLOAD` de cada formato, del corpus y de ficheros con cabecera; fichero inexistente (`ERROR 61`); solo modo directo; demasiado grande; línea mala; `CLOAD?`; `MERGE`; `CHAIN` conserva variables; nombres no válidos y cinta; Reset conserva la configuración |

### Tests unitarios (dentro de cada módulo)

| Núcleo | Nº | Interfaz | Nº |
|---|---|---|---|
| `pd1990ac.rs` | 12 | `host_keymap.rs` | 15 |
| `key_feeder.rs` | 9 | `pacing.rs` | 11 |
| `keyboard.rs` | 8 | `faceplate.rs` | 11 |
| `host_io.rs` | 8 | `lcd_view.rs` | 9 |
| `display.rs` | 7 | `pc1500_app.rs` | 5 |
| `lh5_loader.rs` (ya existían) | 6 | `basic_io.rs` | 4 |
| `basic.rs` | 3 | | |

---

## 6. Hallazgos por el camino

**Comportamiento real de la ROM (no son fallos):**
- `PRINT 1:PRINT 2` en **modo directo** da `ERROR 1`. La rutina `CHECK_AT_END` (`$DCC5`) solo acepta `:` cuando hay un número de línea en curso. Dentro de un programa funciona.
- `RUN` en modo **PRO** da `ERROR 26`. La ROM solo admite `RUN` en modo RUN.
- La ROM no detecta flancos de tecla. Si llega una tecla antes de que haya visto soltar la anterior (bit 0 de `$7B0E`), la descarta. Por eso el harness espera con `run_until_ready`, y el alimentador de teclas de la interfaz hace lo mismo.
- Los códigos de Shift+↑ y Shift+↓ (`$5B`/`$5D`, es decir `[` y `]` en ASCII) se **ven en el LCD como √ y π**, porque así los dibuja la fuente de la ROM. La carátula y la ayuda lo indican.

**Fallos del emulador corregidos:**
- Polaridad de los indicadores invertida, y RAD duplicado.
- BREAK imposible: PB7 no se activaba nunca.
- *Underflow* de temporización tras un segundo reset.
- Reloj parado, en UTC y con `TIME=` perdido.
- Velocidad real máxima de 0,69x.
- SHIFT del PC mal tratado, y mapeo que dependía de la distribución US.
- Fixture de arranque sin `0xFF` final ni `VAR_START`: editar corrompía el programa.

**Diferencias del corpus.** `mole.bas` y `jackpot.bas` no son listados exactos de sus `.bin`: usan `GOTO` donde el original tiene `THEN`, entre otras cosas. El tokenizador es fiel a la ROM. Para reproducir esos `.bin` exactos hay una normalización que **solo existe en los tests**.

---

## 7. Pendiente

- **Corregido tras la revisión final:**
  - El número del deslizador de velocidad se convertía en un campo de texto que robaba teclas a la PC-1500. Ahora el valor se muestra como etiqueta.
  - La etiqueta de velocidad se partía en dos líneas.
  - Aclarado lo de √/π en la ayuda.
  - Una errata en `harness.rs`.
- **Limitaciones conocidas y documentadas:**
  - En Windows, los caracteres con AltGr (`@ # [ ]`) pueden no llegar.
  - Si se cancela un pegado justo después del SHIFT de prefijo, SHIFT queda memorizado.
  - No hay test de `CSAVE` en modo RESERVE (`ERROR 26`).
  - `CSAVE` escribe por defecto en la carpeta desde la que se lanza el emulador.
- **Pruebas manuales de la interfaz:**
  - aspecto del LCD y de la carátula;
  - teclado español: `( ) * " ? ;`;
  - que Tab y Espacio no activan botones;
  - Ctrl+V;
  - arrastrar ficheros (en Wayland puede no funcionar);
  - deslizador de velocidad y modo Máx;
  - auto-apagado.
- Nada está commiteado. Cuando lo revises, se puede agrupar en commits por funcionalidad.
