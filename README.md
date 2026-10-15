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
target/release/ceres-egui --pc1500a program.lh5     # the same on a PC-1500A (4 KB more RAM)
```

User RAM is `&0000-&47FF` (`&57FF` on a PC-1500A); compiled programs load at the address in
their `.lh5` header, `&0100` by default.

In the window:
- **.lh5 file / Load** loads another program; **Reset** restarts the machine and goes back to
  BASIC.
- **Pause**, **Step** and **Speed** control the emulation: 1x is the real PC-1500's speed.

### Keyboard

The keys of the PC-1500 drawn in the window can be clicked. From the computer's keyboard:

| Computer | PC-1500 |
|---|---|
| Letters, digits, Space, Enter, arrows, `-` `/` `=` `.` | The same |
| `[` and `]` | `(` and `)` |
| Shift | SHIFT |
| Backspace | CL |
| Tab | MODE |
| Esc / End | OFF / ON |
| `!` `"` F3-F6 | F1 F2 F3-F6 |

DEF and the other keys are only on the window's keyboard.

## Thanks

- [PockEmul](https://github.com/matsumo/PockEmul)
- [lh5801_Emu](https://github.com/Jeff-Birt/lh5801_Emu)
- [Sharp_PC-1500_ROM_Disassembly](https://github.com/Jeff-Birt/Sharp_PC-1500_ROM_Disassembly)
