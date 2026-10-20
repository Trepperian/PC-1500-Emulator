# New features of the PC-1500 emulator

This document gathers **everything new and changed** in the work that answered the tutor's
comments on the emulator: what each feature does, how it is used, which files it touches, which
tests cover it and what the test *harness* is. The work was done by aburgaz on top of commit
`e1c45f1` (commit `fe9e955`) and merged into the branch `auditoria-fidelidad` (`cce31ca`), where
it was adapted to that branch's memory map (the CE-161's RAM from `&0000`, BASIC at `&00C5`) and
to the PC-1500A, and translated to English.

---

## 1. Summary

| # | Feature | Where it shows |
|---|---|---|
| 0 | Groundwork: a more reliable harness and quieter logs | Tests, console |
| 1 | **BREAK** (the ON/BRK key), and real OFF, automatic power-off and switching on | `Esc` stops a program; OFF switches off |
| 2 | **Grey STN LCD** and correct indicators (PRO, RUN, DEG...) | Display |
| 3 | **TIME** (the µPD1990AC clock rewritten) | `TIME`, `TIME=`, `WAIT`, `RANDOM` |
| 4 | **Speed** beyond 2x, as far as the host allows | Slider up to 1000x and a Max button |
| 5 | **Whole keyboard** from the PC to the PC-1500 | Every key and any layout |
| 6 | **Loading and saving BASIC** (ASCII, tokenized, CE-158) from the GUI | "BASIC program" panel |
| 7 | **CLOAD / CSAVE / MERGE / CHAIN** inside the emulated BASIC | BASIC commands with PC files |
| 8 | **Procedural faceplate** of the PC-1500 | Main view |

Figures: **226 tests pass** (117 integration and 109 unit tests) and 1 benchmark is ignored on
purpose. No compiler warnings, in dev or release.

---

## 2. Map of files

### New

| File | What for |
|---|---|
| `ceres-core/src/harness.rs` | Test harness: boot the ROM without a window, press keys and read the LCD as text (see §4) |
| `ceres-core/src/key_feeder.rs` | Key feeder: the only scheduler of key presses, which counts time in emulated ticks |
| `ceres-core/src/basic.rs` | Tokenizer and detokenizer (keyword table read from the ROM), the ASCII, tokenized and CE-158 formats, and loading and saving in RAM |
| `ceres-core/src/host_io.rs` | CLOAD/CSAVE/MERGE/CHAIN: a virtual keyword table at `$9800` and "traps" that do the file I/O in Rust |
| `ceres-core/tests/*.rs` | Integration test files (see §5) |
| `ceres-core/tests/data/` | `mole`, `jackpot` and `bathyscaph` as `.bas` (from the compiler's corpus) and `.bin` (tokenized from them) |
| `ceres-egui/src/lcd_view.rs` | Procedural painting of the grey STN LCD with the indicators inside the glass |
| `ceres-egui/src/pacing.rs` | `Pacer`: real-time pacing, accumulator, budget per frame, Max mode and measurement of the speed |
| `ceres-egui/src/host_keymap.rs` | Translation of egui's keyboard events to PC-1500 actions, the table of special keys and the help texts |
| `ceres-egui/src/basic_io.rs` | Logic of the BASIC panel: choice of format, load queue and command line argument |
| `ceres-egui/src/faceplate.rs` | The PC-1500's faceplate drawn by code, with clickable keys |

### Changed

| File | Main change |
|---|---|
| `ceres-core/src/lib.rs` | `run_for_ticks`, per-frame hooks, `reset()` with a list of what survives, PB7 (BREAK), clock (OPB5/OPB6), automatic power-off and a new public API |
| `ceres-core/src/lh5801.rs` | Off state (`bf`), `power_on`, `timer_state = 0` removed from the reset (it caused an *underflow*), fast forward in `HLT` and a trap hook for CLOAD/CSAVE |
| `ceres-core/src/keyboard.rs` | `MATRIX` table (the same logic as before, checked against the ROM), `Key::ALL`, labels, the ON pulse, `Key::Quote` removed (it did not exist), and `KeyStroke`/`key_for_char` now live here |
| `ceres-core/src/display.rs` | Indicators with the right polarity and by table (DE, G and RAD apart), columns, `is_on()` and a grey palette |
| `ceres-core/src/pd1990ac.rs` | Complete rewrite of the clock |
| `ceres-core/src/memory.rs` | `LOG_UNMAPPED` (silences the `println!`s), reading of the virtual table at `$9800` and `read_banked` |
| `ceres-egui/src/pc1500_app.rs` | New GUI: speed, keyboard, BASIC and CLOAD/CSAVE panels, keyboard help, LCD and checkboxes |
| `ceres-egui/src/main.rs` | New modules, a 1280×760 window and the command line argument routed by extension |

---

## 3. Each feature in detail

### 3.1 BREAK, and real OFF and ON (step 1)

**What was wrong.** On the real PC-1500, the ON/BRK key is not in the keyboard matrix: it goes to
the LH5810's **PB7** pin, which the ROM checks in `CHK_BRK` (`$E451`, `BII #$F00B,$02`). The
emulator never set PB7, so BREAK could not work.

**What has been done.**
- ON generates a **single pulse** on PB7 on the press edge, as `CheckKon` in PockEmul. If it were
  a level, the ROM would see a second BREAK while the key is held and would clear the
  `BREAK IN nn` message.
- **Real OFF.** The `OFF` instruction (FD 4C) really switches off: blank display, CPU stopped and
  RAM kept.
- **ON with the machine off** restarts the CPU, and the ROM restores the session with its
  signature at `$7A10`.
- Faithful **automatic power-off** (some 5 minutes without keys at 1x), **on by default**, with the
  **"Auto power-off"** checkbox to disable it. The checkbox keeps the counter `$7B0A`-`$7B0C` away
  from the overflow.
- A latent fault fixed: the reset set `timer_state = 0`, and at the next reset the timing
  subtractions *underflowed*.

**Use.** `Esc` (or F12, End, Ctrl+C) is BREAK. `Ctrl+F12` is OFF. With the machine off the notice
"Off — press ON (Esc)" shows.

A compiled program (`.lh5`) does not look at the key, as machine code on the real machine: BREAK
does not stop it (nor does it break it: an `INPUT` or a `PRINT` waiting go on working); Reset
does.

**API.** `Pc1500::is_powered_on()`, `set_auto_power_off(bool)`, `auto_power_off()`. Harness:
`power_on()`.

### 3.2 Grey LCD and indicators (step 2)

**What was wrong.** `display.rs` read the indicators **the other way round**: a bit at 1 means on.
Besides, it assigned RAD twice and always lit the battery.

**What has been done.**
- A table of bits taken from the ROM:
  - `$764E`: BUSY, SHIFT, KANA, SMALL, III, II, I, DEF.
  - `$764F`: DE, G, RAD, RESERVE, PRO, RUN.
- **DEG = DE+G** and **GRAD = G+RAD**, as on the real glass.
- Procedural painting of the LCD (`lcd_view.rs`): grey STN background, dark grey pixels and an
  optional "ghost" of the unlit pixels. The indicators go **inside the glass**, in the same color
  and in their real position, with a 3×5 type of our own.
- The ROM turns the display off for some 1.3 ms at each turn of the keyboard loop. There is now a
  persistence measured in emulated ticks, so the display no longer flickers.

**Use.** The "LCD ghosting" and "Large LCD" checkboxes.

### 3.3 TIME (step 3)

**What was wrong.** The clock did not implement mode 3 (*Time Read*), never moved the time on,
used UTC and shifted the bits in a different order from what the ROM's `WRITE_2_CLOCK` routine
expects. `TIME=` was lost.

**What has been done.**
- A rewrite of the µPD1990AC following its data sheet:
  - 40-bit shift register;
  - commands 0-3 and the TP frequencies (4-7) latched separately on the edge of STB;
  - shift on the edge of CLK;
  - OE output on PC3.
- The time comes from the **PC's local clock**, moves on in real time at any speed and **survives
  the Reset button**.
- The change of day and month is in BCD, without leap years, as on the real chip.
- TP (64/256/2048 Hz) goes in emulated time, so `WAIT`, `BEEP` and the pauses of `PRINT` follow
  the emulation speed.

**Use.** `TIME` returns `MMDDHH.MMSS`. `TIME=101912.3045` sets the time. An invalid month gives
`ERROR 23`. The GUI shows "Clock: dd/mm hh:mm:ss".

**API.** `RtcDateTime`, `RtcTimeBase::{HostLocal, Emulated}`, `Pc1500::set_rtc_time_base`,
`rtc_now`, `reset()`.

### 3.4 Speed (step 4)

**What was wrong.** The GUI ran at most one `step_frame()` (11.5 emulated ms) per repaint. With a
60 Hz monitor, the real maximum was **0.69x**, so even "1x" was slow.

**What has been done.**
- Core:
  - `run_for_ticks(budget)`, on which `step_frame()` now depends. All the per-frame hooks are
    measured in emulated ticks.
  - **Fast forward in `HLT`.** When the CPU sleeps, it jumps straight to the next boundary of 42
    timer states. A test shows the result is identical.
- GUI, with `Pacer`:
  - a time accumulator with a budget per frame, so that the GUI does not freeze;
  - if the PC cannot keep up, it drops the pending time instead of accumulating it;
  - a limit when coming back from a minimized window.
- Performance measured in release:
  - **some x396 idle** (some x83 before);
  - some **x159 in a busy loop**.

**Use.** A logarithmic slider **0.02x-1000x**, preset buttons, a **"⏩ Max"** button and a label
with the real speed measured and the emulated MHz. A "limited by the host" warning when the speed
asked for is not reached.

> Note: in Max mode with the automatic power-off on, the idle machine switches off in seconds (it
> is the real behaviour, sped up). Turn "Auto power-off" off if it is in the way.

### 3.5 Whole keyboard (step 5)

**What was wrong.** The PC's Shift held the PC-1500's SHIFT down, but on the real machine SHIFT
is a **prefix**: it is pressed and released before the next key. So every character with Shift
failed. Besides, the mapping depended on the US layout: on a Spanish keyboard, `(` arrived as `8`.

**What has been done.**
- Printable characters arrive as **text events**, so any layout works. The exact list of
  characters supported is in §4.
- The **key feeder** (`key_feeder.rs`) measures the key presses in emulated time and adds the
  SHIFT prefix only when needed.
- Key presses depending on the speed:
  - at ≤1x the key is held as long as you hold it, so it works for games with `INKEY$`;
  - at >1x each key press is a tap of fixed length.
- **Ctrl+V** types the clipboard, and "Cancel typing" stops it.
- The GUI does not steal keys: Tab and Space do not click buttons, and what you type in a path
  field does not reach the emulator.
- A help window **"⌨ Keyboard"**, generated from the same tables the mapping uses.

**Special keys:**

| PC | PC-1500 |
|---|---|
| Esc, F12, End, Ctrl+C | ON/BRK (BREAK) |
| Ctrl+F12 | OFF |
| Home / Shift+Home | CL / CA |
| Enter | ENTER |
| Backspace | delete to the left (← and then DEL) |
| Delete / Insert | DEL / INS |
| Arrows, Page Up, Page Down | cursors |
| Shift+↑ / Shift+↓ | `[` / `]` (the LCD shows them as √ and π) |
| Tab / Shift+Tab | MODE (RUN/PRO) / RESERVE |
| F1-F6, Shift+F1-F6 | reserved keys / `! " # $ % &` |
| F7 | DEF |
| F8 | SML (lowercase) |
| F9 | RSV |
| F10 | RCL |
| F11 | the PC-1500's SHIFT (prefix) |

Letters come out in **uppercase** by default. With the **"Match upper/lower case"** checkbox the
PC's case is kept. The **"Capture the PC keyboard"** checkbox turns the capture on and off.

### 3.6 Loading and saving BASIC from the GUI (step 6)

**What has been done.**
- **Tokenizer**, a faithful copy of the ROM's `TOK_INBUF` routine.
  - The keywords are **read from the ROM itself**, walking the tables in the same order as the
    ROM, so it includes the virtual table of CLOAD/CSAVE.
  - Checked by typing **3,596 lines** of the corpus on the real ROM: 0 differences of bytes.
- **Formats:**
  - ASCII (`10 PRINT` or `10:PRINT`, with CRLF, CR or LF line ends);
  - a tokenized dump without header (`.bin`);
  - tokenized with the **CE-158 header** of 27 bytes, as the real CE-158 or Jeff Birt's files
    write it (and as the dull compiler does).
  - The format is detected automatically.
- **Loading into RAM:** it updates the same system pointers as the ROM (`BASPRG_ST`/`END`/`EDT`,
  `VAR_START`, the DATA pointer...), so that `LIST`, `RUN`, editing and adding lines work
  afterwards. The program goes where the ROM's program is, at `&00C5` (where `NEW0` puts it with
  the CE-161), and gets the RAM up to its end (`&47FF`, or `&57FF` on a PC-1500A).
- **The boot program fixed.** The program in RAM at power-on (bathyscaph) was loaded without
  `VAR_START`, and without the final `0xFF` if its file lacks it, so **editing a line after
  booting corrupted the program**. The fix is in `Pc1500::new()`.

**Use.** The "BASIC program" panel:
- a path field;
- format: Auto, ASCII CRLF, ASCII CR (`.asc`), Tokenized (`.bin`) or CE-158 (`.bas`);
- the "Load BASIC" and "Save BASIC" buttons;
- a file can also be **dropped on the window** or passed on the **command line**:
  `cargo run -- program.bas`.
- If the machine is not at the prompt, the load waits in a queue.

When saving, the extension decides the format:

| Extension | Format |
|---|---|
| `.bas` | CE-158 (tokenized with header) |
| `.asc` | ASCII with CR |
| `.txt` | ASCII with CRLF |
| `.bin` | tokenized without header |

**API.** `basic::{KeywordTable, Keywords, Program, detect_format, …}` and
`Pc1500::{is_at_basic_prompt, load_basic_program, load_basic_bytes, load_basic_file,
save_basic_file, basic_program, basic_program_bytes}`.

### 3.7 CLOAD, CSAVE, MERGE and CHAIN in the emulated BASIC (step 7)

**How it works.** There is a small virtual keyword table at `$9800`, which the ROM discovers by
itself at boot. It uses the **real tokens**: CLOAD=`F089`, CSAVE=`F095`, MERGE=`F08F`,
CHAIN=`F0B2`. That is why programs that already contain these commands list and run fine. When
the CPU reaches the handler, the emulator does the file I/O in Rust (reusing `basic.rs`) and
gives control back to the ROM, or shows `ERROR n`.

| Command | Effect |
|---|---|
| `CSAVE "X"` | `X.BAS`, tokenized with the CE-158 header, as the real CE-158 |
| `CSAVEa "X"` | `X.ASC`, in ASCII text |
| `CSAVE "X.BIN"` / `"X.TXT"` | tokenized without header / CRLF text |
| `CLOAD "X"` | detects the format by itself (ASCII, tokenized or CE-158). **Direct mode only** |
| `CLOAD? "X"` | compares with what is in memory (`ERROR 43` if it does not match) |
| `MERGE "X"` | adds to the end of the current program (direct mode only) |
| `CHAIN "X"[,line]` | loads and runs keeping the variables (inside programs) |
| `CLOAD` / `CSAVE` without a name | uses the GUI's "Tape" file; without it, `ERROR 40` |

Errors, as on the real hardware:

| Error | Cause |
|---|---|
| 27 | no folder set |
| 61 | the file does not exist |
| 40 | invalid name |
| 42 | the program does not fit |
| 44 | read error |
| 67 | ASCII line without a number |

For safety, names with `/`, `..` or starting with `.` are rejected.

**Use.** In the GUI: "CLOAD/CSAVE folder" (by default, the current folder) and "Tape".

**API.** `Pc1500::{set_host_dir, host_dir, set_tape, tape, take_host_events}`, `HostEvent` and
`HostCommand`.

### 3.8 Faceplate (step 8)

A faceplate of the original PC-1500 (bronze top band) **drawn by code**, with no photos or new
dependencies:
- a brushed aluminium panel;
- the 65 keys in their real position and color, with the SHIFT labels;
- the LCD of §3.2 built in;
- clickable keys that can be held, with their PC key in the *tooltip*.

It replaces the old generic button keyboard.

---

## 4. The test harness: what it is and what it does

**What it is.** A public module, `ceres_core::harness` (`ceres-core/src/harness.rs`), to use the
emulated PC-1500 **without a window**, from a test, as a person would: it boots the real ROM,
presses keys and reads what shows on the display.

**What it is for.** It is what lets every feature be checked automatically against the **real
A04 ROM**, instead of trusting what "should" happen. For example: typing `RUN`, pressing ON and
checking that the LCD says `BREAK IN 10`.

**How it works inside.**
- **Deterministic time.** Everything is measured in *emulated frames* (15,000 cycles, some
  11.5 ms). `Pc1500::new_deterministic()` also fixes the clock at a known time, so that `TIME` and
  `RANDOM` always give the same.
- **Pressing keys.** Each key is held for 4 frames and released for another 4. Measured by
  sweeping values: below "release ≥2 and hold+release ≥6", the ROM loses keys. Characters with
  SHIFT are typed as on the real machine: first a tap of SHIFT and then the key.
- **Knowing when the ROM is ready.** `is_waiting_for_key()` checks that the CPU is asleep in the
  keyboard loop `WAIT_4_KB` (`$E243`-`$E33F`). `is_ready_for_key()` also checks that the ROM has
  already seen the previous key released (bit 0 of `$7B0E`). This detail was discovered in step
  0: without it, the first key of the next line was lost (`200` ended up as `00`).
- **Reading the LCD as text.** It decodes the display RAM (`$7600`/`$7700`) in 156 columns and
  compares each cell of 6 columns with the **ROM's own 5×7 font** (`$FCA0`). That gives the 26
  characters of the display.

**Main functions:**

| Function | What it does |
|---|---|
| `Pc1500::new_deterministic()` | A new machine with a fixed clock (for tests) |
| `boot()` | Cold boot, presses CL and leaves the `>` prompt in PRO mode. It keeps the boot program |
| `run_frames(n)` / `run_until(n, cond)` | Emulates n frames, or until a condition holds |
| `run_until_idle(n)` / `run_until_ready(n)` | Waits for the ROM to wait for a key / to be ready for the next one |
| `tap(KeyStroke)`, `tap_key_for(key, hold, release)` | Presses a key, with the SHIFT prefix if needed |
| `type_text("...")` | Types text (`\n` = ENTER) |
| `enter_line("10 PRINT 5")` / `enter_lines(...)` | Types whole lines reliably, waiting for the ROM before and after |
| `power_on()` | Presses ON and waits for the ROM to be ready for the next key |
| `lcd_text()` / `lcd_cells()` / `lcd_columns()` | Display as text / by cells / by columns of pixels |
| `lcd_symbol_bytes()` | Raw bytes of the indicators (`$764E`, `$764F`) |
| `read_bytes(addr, n)`, `read_word_be(addr)` | Read memory |
| `key_for_char(c)`, `text_to_keystrokes(s)` | Which key, with or without SHIFT, produces each character |

**A real (simplified) example of a test:**

```rust
let mut pc = Pc1500::new_deterministic();
pc.boot();                                     // prompt ">"
pc.type_text("NEW\n").unwrap();
pc.enter_line("10 GOTO 10").unwrap();          // a program in an endless loop
pc.tap(KeyStroke::plain(Key::Mode));           // to RUN mode
pc.type_text("RUN\n").unwrap();
pc.run_frames(30);
pc.press(Key::On); pc.release(Key::On);        // BREAK
pc.run_until_idle(500);
assert!(pc.lcd_text().contains("BREAK IN 10"));
assert_eq!(pc.read_word_be(0x78AE), 10);       // the ROM's BRK_LINE
```

**Characters it can type:**
- **Without SHIFT:** A-Z, 0-9, space and `. - + * / = ( )`.
- **With the SHIFT prefix:** `! " # $ % &` (F1-F6), `,` `<` `>` `?` `:` `;` `@` `^` `[` `]` and
  the lowercase letters.
- **Impossible**, because there is no key: `' _ \` { | } ~ \`.

---

## 5. The tests

**How to run them:** `cargo test --workspace`, a few seconds in all. The performance test is
marked `#[ignore]`; it is run with
`cargo test --release -p ceres-core --test speed -- --ignored --nocapture`.

### Integration tests (`ceres-core/tests/`), all against the real ROM

| File | No. | What it checks |
|---|---|---|
| `harness.rs` | 13 | The harness itself: boot (`NEW0? :CHECK`, `>` prompt in PRO), `PRINT 1+2` → `3`, storing and listing lines, running in RUN mode, that every character can be typed, that repeated keys and long lines are not lost, and that `:` only separates statements inside a program (the ROM's real behaviour) |
| `break.rs` | 12 | BREAK during `RUN`, with `CONT`, holding ON, in `INPUT` and in `WAIT`; ON at the prompt clears the line; OFF switches off and ON restarts keeping the program; ON with the machine off does not also send a BREAK; automatic power-off, resuming and disabling it; loading a `.lh5` with the machine off |
| `display_symbols.rs` | 12 | Indicators at boot (I, DEG, PRO), MODE RUN/PRO, RESERVE, SHIFT/DEF, SML, DEGREE/RADIAN/GRAD, levels I/II/III, BUSY while running, pixels identical to the harness's, that the display does not flicker when idle, and blank when off |
| `time.rs` | 8 | `TIME` reads the running clock; `TIME=` and reading 5 s later; Feb 28 → Mar 1; end of the year; month 13 → `ERROR 23`; Reset keeps the time; `TIME_DELAY` lasts 1 s per 64 units; `WAIT 64` |
| `speed.rs` | 8 | `run_for_ticks` spends at least the budget; `step_frame` is the same as a frame; running in pieces gives the same result; the fast forward in `HLT` is equivalent; the automatic power-off works with small pieces; the LCD does not flicker in slow motion; benchmark (ignored) |
| `keyboard_mapping.rs` | 19 | The `MATRIX` matches the ROM's tables (`$FE80`/`$FEC0`) and is complete; every character can be typed or it is known why not; holding SHIFT types nothing (as on the real one); the feeder: short taps, bursts, rollover, key repeat, latched SHIFT, exact pasting with SML, pasting 20 lines, releasing everything and the priority of BREAK |
| `basic.rs` | 21 | Keyword table read from the ROM (85 entries); the tokenizer's rules; the corpus's `.bas` → `.bin` byte for byte; round trips; the CE-158 header; format detection; what is typed on the ROM matches the PC's tokenizer; the boot program fixed; load → `LIST` → edit → `RUN`; `CONT` after loading; "not at the prompt"; too big |
| `host_files.rs` | 22 | The virtual table is tokenized and listed; `ERROR 27` without a folder; exact `CSAVE` (`mole.bin`); CE-158 as the default format; the `a` suffix and extensions; `CLOAD` of each format, of the corpus and of files with a header; a file that does not exist (`ERROR 61`); direct mode only; too big; bad line; `CLOAD?`; `MERGE`; `CHAIN` keeps the variables; invalid names and tape; Reset keeps the configuration |
| `pc1500a.rs` | 2 | The PC-1500A: `reset()` keeps the model, and a BASIC program gets the RAM up to `&57FF` |

### Unit tests (inside each module)

| Core | No. | GUI | No. |
|---|---|---|---|
| `pd1990ac.rs` | 12 | `host_keymap.rs` | 15 |
| `key_feeder.rs` | 9 | `pacing.rs` | 11 |
| `keyboard.rs` | 8 | `faceplate.rs` | 11 |
| `host_io.rs` | 8 | `lcd_view.rs` | 9 |
| `display.rs` | 7 | `pc1500_app.rs` | 5 |
| `lh5_loader.rs` | 7 | `basic_io.rs` | 4 |
| `basic.rs` | 3 | | |

---

## 6. Findings along the way

**The ROM's real behaviour (not faults):**
- `PRINT 1:PRINT 2` in **direct mode** gives `ERROR 1`. The `CHECK_AT_END` routine (`$DCC5`) only
  takes `:` when there is a line number in progress. Inside a program it works.
- `RUN` in **PRO** mode gives `ERROR 26`. The ROM only takes `RUN` in RUN mode.
- After a cold start the machine shows `NEW0? :CHECK` and takes no key but CL until it is
  cleared: MODE does nothing.
- The ROM does not detect key edges. If a key comes before it has seen the previous one released
  (bit 0 of `$7B0E`), it drops it. That is why the harness waits with `run_until_ready`, and the
  GUI's key feeder does the same.
- The codes of Shift+↑ and Shift+↓ (`$5B`/`$5D`, that is `[` and `]` in ASCII) **show on the LCD
  as √ and π**, because that is how the ROM's font draws them. The faceplate and the help say so.

**Emulator faults fixed:**
- Indicator polarity inverted, and RAD duplicated.
- BREAK impossible: PB7 was never set.
- Timing *underflow* after a second reset.
- Clock stopped, in UTC and with `TIME=` lost.
- Real maximum speed of 0.69x.
- The PC's SHIFT handled wrongly, and a mapping that depended on the US layout.
- The boot program without `VAR_START`: editing corrupted the program.

---

## 7. Pending

- **Known and documented limitations:**
  - On Windows, the characters with AltGr (`@ # [ ]`) may not arrive.
  - If a paste is cancelled right after the SHIFT prefix, SHIFT stays latched.
  - There is no test of `CSAVE` in RESERVE mode (`ERROR 26`).
  - `CSAVE` writes by default to the folder the emulator was started from.
- **Manual tests of the GUI:**
  - look of the LCD and of the faceplate;
  - Spanish keyboard: `( ) * " ? ;`;
  - that Tab and Space do not click buttons;
  - Ctrl+V;
  - dropping files (it may not work on Wayland);
  - the speed slider and Max mode;
  - automatic power-off.
