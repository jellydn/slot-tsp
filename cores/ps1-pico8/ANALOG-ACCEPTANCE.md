# Offline PS1 analog acceptance

`analog-acceptance.S` is an original, separate diagnostic disc program. It uses
no proprietary game, BIOS or SDK asset. The existing acceptance and transport
fixtures, builder and production controller options are unchanged.

## Contract

The guest stops firmware pad polling and owns SIO. It sends enter-config `0x43`,
set-analog `0x44`, exit-config `0x43`, then poll `0x42`. It retries negotiation
after a digital reply or controller replacement. Selecting a Digital controller
must still produce `0x41` and five received bytes. Selecting DualShock must
produce a negotiated `0x73` and nine received bytes, not merely a subclass ID.

Byte exchange waits for RX-ready and the latched ACK interrupt, with bounded
timeouts. The transient ACKINPUT bit is unsuitable here: pinned PCSX does not
implement it, and reading Swan's status register consumes it. The guest enables
ACK interrupts but disables CPU interrupt handling, so no BIOS service competes
for the serial port. It makes no memory-card writes.

The guest draws 15 byte rows, least-significant bit first, using black/white
tiles at `x = 16 + 32 * bit`, `y = 48 + 8 * row`, size 16×8. Every tile fits
inside both PCSX's 320×240 output and Swan's default 320×224 overscan crop.
No core crop setting is changed.

| Rows | Received/report value |
| --- | --- |
| 0–1 | Marker `A6 59` |
| 2–10 | Raw serial reply: `FF`, ID, `5A`, buttons low/high, RX, RY, LX, LY |
| 11 | Actual response length; unreceived byte slots remain zero |
| 12–13 | Completed-poll counter, little endian |
| 14 | XOR of rows 0–13 |

`crates/slot/src/frontend_ps1_analog_tests.rs` is a macOS-only, opt-in child unit
test module. Temporary FIFO/sysfs-shaped input goes through production
`DeviceInput`, the steering-policy setter, `Frontend`, and its driven Session
worker into the real core. Assertions decode only guest-rendered frames. There
is no test memory API, direct analog injection or echo-core report file. Two
matching reports with forward-moving completed-poll counters are required per
check; the 16-bit comparison permits wrap and rejects stale/backward reports.

Each core is tested from cold Digital and DualShock selections, through live
menu-driven switches and reset. Checks include every physical game button and
release, KEY and ABS triggers, each axis in both directions and release, and a
mixed four-axis vector while buttons are pressed. Reset uses the normal App
reset action and must restart the guest counter before acceptance continues.
PCSX also saves in Digital and loads after selecting DualShock, then saves in
DualShock and loads after selecting Digital. The guest counter must rewind,
while the currently selected controller, live axes and fresh button presses
remain usable. Swan deliberately disables states to protect memory cards;
its refused load actions must leave input working instead. The test does not
enable Swan states or alter that safety policy.

Input events and mode switches use the FIFO; reset and state actions enter at
the normal App action interface.
The test does not access real input nodes; it refuses hosts with `/sys`.

Expected mixed RX/RY/LX/LY bytes are derived from the pinned core conversions,
not copied from test output. For LX=-24576, LY=8192, RX=16384, RY=-12288:

- PCSX: `[210, 67, 6, 169]`, using its default square conversion
  `round((axis >> 8) / 0.785) + 128`.
- Swan: `[212, 64, 0, 170]`, using its fallback AxisScale=1.33 and clamped,
  rounded unsigned conversion.

Assertions allow one byte of quantisation difference. In particular, PCSX
intentionally changes neutral LX from `0x80` to `0x81` in `adjust_analog()`.
Endpoint, sign, swapped-axis and zeroed-axis errors remain distinguishable.

## Build and run

Build into a new output directory; the builder refuses an existing directory:

```sh
guest=$(mktemp -d)
cp cores/ps1-pico8/build-acceptance.py "$guest/"
cp cores/ps1-pico8/analog-acceptance.S "$guest/acceptance.S"
python3 "$guest/build-acceptance.py" .amp/in/ps1-analog \
  --llvm-bin /opt/homebrew/opt/llvm/bin
rm -r "$guest"
```

The temporary copy uses the committed builder's default source name; no
uncommitted alternate-source builder option is required.

Use locally built, pinned real-core libraries and run serially, without host
input/audio backends:

```sh
SLOT_BOARD=tg5040 \
SLOT_PCSX_CORE=/absolute/path/pcsx_bounded_libretro.dylib \
SLOT_SWAN_CORE=/absolute/path/swanstation_libretro.dylib \
SLOT_PS1_ANALOG_DIR="$PWD/.amp/in/ps1-analog" \
cargo test --offline --locked -p slot --no-default-features --features romm \
  --lib real_core_analog_acceptance -- --ignored --nocapture --test-threads=1
```

Optional `SLOT_PS1_ANALOG_CAPTURE` names an existing directory for decoded
Digital and non-neutral analog frame PNGs. Captures are evidence, not the
assertion oracle. Each core has its own test name for focused execution.

## Evidence boundary

This verifies the authored guest's serial-pad contract on the supplied host core
binaries. It does not establish handheld acceptance, performance, proprietary
BIOS compatibility, or original Road Rash accepting native analog input. It
does not reproduce or fix the owner's global DualShock button-loss report.
Digital stick compatibility and release approval remain separate gates.

For a red control, build a temporary copy with the set-mode byte changed from
`1` to `0` in the `0x44` command. The same acceptance tests must reject the
resulting `0x41` reply when DualShock analog is expected; Digital assertions
must pass first. Do not change production options or the permanent fixture to
make that control pass.

A second red control zeroes the four reported axis bytes after receiving the
packet, leaving ID `0x73`, length and buttons intact. Both tests must reject
this report. This checks the observation oracle, not a simulated device fault;
`0x73` alone cannot pass analog acceptance.
