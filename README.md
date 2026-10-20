# PC-1500 Emulator

An emulator of the Sharp PC-1500 pocket computer: the LH5801 CPU, the real A04 ROM, the LCD,
the keyboard and the clock, with the CE-161 RAM expansion fitted. It runs the PC-1500's own
BASIC, and the `.lh5` machine code programs made by the
[dull compiler](https://github.com/Trepperian/SHARP_PC-1500_Compiler), whose test suite uses it
to check the compiled code against the real ROM.

## Building

The ROM comes from a git submodule, so clone with `--recursive`:

```bash
git clone --recursive https://github.com/Trepperian/PC-1500-Emulator.git
cd PC-1500-Emulator
cargo build --release
```

In a clone made without `--recursive`, get the ROM with `git submodule update --init`; without
it the build stops with "couldn't read ... PC-1500_ROM-A04.bin".

## Running

```bash
target/release/ceres-egui                           # BASIC, as when the machine is switched on
target/release/ceres-egui program.lh5               # load a compiled program and run it
target/release/ceres-egui program.bin               # load a BASIC program, to run it with RUN
target/release/ceres-egui --pc1500a program.lh5     # the same on a PC-1500A (4 KB more RAM)
```

The file is taken by its extension: `.bas`, `.asc`, `.txt`, `.bin` (and `.sbs`, `.tok`, `.img`)
are BASIC programs, anything else is `.lh5` machine code. `--basic file` loads a file as BASIC
whatever its extension. The window's title says which kind it has, "(BASIC)" or "(compiled)",
to tell apart two windows run side by side.

A BASIC program can be a text listing, a tokenized dump or a tokenized file with the CE-158's
27-byte header (what `dull program.bas -o program.bin` writes); the format is recognized from
the contents. It is loaded as soon as the ROM reaches BASIC's prompt, where the PC-1500 puts a
program after `NEW0`, `&00C5`. To run it, press CL (Home) to clear the `NEW0? :CHECK` the machine
shows when it starts (until then it takes no other key), MODE (Tab) for RUN mode (RUN lit), then
type `RUN` and Enter, or DEF (F7) and the program's label if it starts at one. A program that
reads a variable `A`-`Z` before giving it a value needs `CLEAR` first: the emulator starts with
them unset, and the ROM stops with `ERROR 15`.

User RAM is `&0000-&47FF` (`&57FF` on a PC-1500A); compiled programs load at the address in
their `.lh5` header, `&0100` by default. A compiled program cannot be stopped with BREAK (it
does not look at the key, as machine code on the real machine): **Reset** stops it.

In the window:
- The PC-1500's faceplate, with its grey LCD and indicators; its keys can be clicked.
- **Pause**, **Step**, the **Speed** slider (0.02x to 1000x; 1x is the real PC-1500's speed) and
  **Max** (as fast as the computer can).
- **.lh5 file** with **Load**, **Reset** (restarts the machine and goes back to BASIC) and the
  clock that `TIME` reads (the computer's local time).
- **BASIC program**: a path, its format, **Load BASIC** and **Save BASIC**. A file can also be
  dropped on the window.
- **CLOAD/CSAVE**: the folder where `CLOAD`, `CSAVE`, `MERGE` and `CHAIN` typed on the PC-1500
  read and write their files (the current folder by default), and a "tape" file for those
  without a name.
- Checkboxes: capture the PC keyboard, match upper/lower case, LCD ghosting, large LCD and
  automatic power-off (on by default, as on the real machine: about 5 minutes without a key).

### Keyboard

Printable characters are typed as they are, with any keyboard layout (the PC-1500's SHIFT is
added when needed). The other keys (the **⌨ Keyboard** window lists them all):

| Computer | PC-1500 |
|---|---|
| Esc, F12, End, Ctrl+C | ON/BRK (BREAK; switches the machine on when it is off) |
| Ctrl+F12 | OFF |
| Home / Shift+Home | CL / CA |
| Enter | ENTER |
| Backspace | delete to the left (← and then DEL) |
| Delete / Insert | DEL / INS |
| Arrows, Page Up, Page Down | the arrows |
| Shift+↑ / Shift+↓ | `[` / `]` (the LCD shows √ and π) |
| Tab / Shift+Tab | MODE (RUN/PRO) / RESERVE |
| F1-F6, Shift+F1-F6 | the reserved keys / `! " # $ % &` |
| F7, F8, F9, F10 | DEF, SML, RSV, RCL |
| F11 | SHIFT (a prefix: it affects the next key) |
| Ctrl+V | types the clipboard |

Letters are typed in uppercase, as BASIC's keywords want, unless "Match upper/lower case" is on.

## Tests

`cargo test --workspace` runs the emulator's tests, most of them against the real ROM through
a headless harness (`ceres_core::harness`): BREAK, the indicators, the clock, the speed, the
keyboard, loading and saving BASIC and the PC-1500A. `NEW_FEATURES.md` describes each feature
and its tests.

## Thanks

- [PockEmul](https://github.com/matsumo/PockEmul)
- [lh5801_Emu](https://github.com/Jeff-Birt/lh5801_Emu)
- [Sharp_PC-1500_ROM_Disassembly](https://github.com/Jeff-Birt/Sharp_PC-1500_ROM_Disassembly)
