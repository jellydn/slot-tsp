# Stock TSP test.12 — experimental Input Test

**Diagnostic prerelease, not a proven binding or performance fix.** This is
stock-firmware FAT32 card content for TrimUI Smart Pro TG5040, not an OS image,
firmware updater or support for another model. Stable 0.2.0 remains Latest.

## What changed

- **Menu → Controls → Input Test** separates observed raw evdev node/type/code/
  value from logical edges and effective PS1 actions. It shows all 14 buttons,
  both signed normalized sticks with center/availability, selected PS1 core
  and saved Digital/DualShock mode. It does not load or execute a core.
- The preview reuses production gestures/bindings, supports modifier bindings,
  and names shortcuts without executing them. Game buttons/axes are blocked
  while open. B release or plain Menu tap returns; exit releases are consumed.
  No auto-remapping, injected input, calibration or trace-file write is added.
- The frontend is rebuilt from committed source with the test.12 label. This
  is not a relabelled test.11 or test.11-input.1 binary. Source identity and
  build/hash receipts accompany the ZIP; bundled sources include the exact
  frontend snapshot, manifest, core sources, patches, licences and build recipes.
- Preserve test.11's clean-extraction fix: empty `Slot/Logs` is retained in the
  ZIP because the stock launcher needs it before native application startup.

## What did not change

The owner reports test.11 now works and PICO art is good. This is scoped owner
feedback, not complete PS1 hardware or speed acceptance. PICO art/fixes, all
pinned core binaries, core metadata/sources/licences, web and startup files
remain test.11 bytes. PCSX NEON stays default. Optional Swan Software 1x/PGXP
off keeps separate cards; states, quick states, automatic resume and rewind
remain disabled. Never move cards/states between cores.

**Unresolved:** the physical BOTH-core L1/L2/R1/R2/stick root cause, early stock
boot logo, startup/first-key volume synchronization, automatic PS1 art,
decorative LEDs/sleep-wake, sustained speed and choppy audio. No blind mapping,
gain, frameskip, clock, firmware or graphics-driver change is included.

Committed graphics diagnostics are supplied as source only. Uncommitted Swan
shared-context attachment work is excluded. This release does not attach a
core to production shared transport or establish GPU/window/driver-loss/speed
acceptance. No merge, SD write, device action or profiling was performed to
prepare the release.

## Owner Input Test

1. Select a PS1 cart on the shelf. Repeat with **PCSX and Swan**, each in
   **Digital and DualShock** mode. Open **Menu → Controls → Input Test**.
   The screen previews the selected settings; it is not a real-core test.
2. Press/release D-pad, face buttons, Select/Start and **L1/L2/R1/R2** one at
   a time. Compare **raw → logical → mapped** observations. `HELD` means
   active now; `seen` means earlier in this visit. L2/R2 are ABS trigger axes,
   not digital KEY events. Node numbers can change; unknown raw codes remain
   visible rather than being hidden behind configured names.
3. Move each stick separately, then together, and release to center. Values
   are normalized after the existing deadzone, not calibration. Digital mode
   ignores gameplay axes. TSP has **no physical L3/R3**, with no substitute.
4. Hold B to inspect it last, then release B to return, or tap Menu. Reopen
   to clear history. Report failed control, raw value, logical edge, mapped
   action, core and controller mode. Correct preview with failed gameplay
   needs separate core-routing investigation; do not auto-remap from it.

## Verification and known exclusions

The attached verification receipts record the fresh committed-source ARM64
build, scoped input/menu/evdev/PICO/core-option/emulation tests, authored C ABI
contract, clean-extraction regression, ZIP/member/source/hash checks and retained
licence/core/web/startup identities. Host render checks cover Controls and
idle/active/released/DualShock states at 720 and 640 widths. These are host
checks, not handheld acceptance or a second reproducible binary build.

Five optional real-core cases remain ignored unless their external libraries
are explicitly supplied. The known baseline fast-forward timing test is
explicitly skipped, not fixed or counted as passing. Linux Clippy is absent
from the pinned image; scoped Mac check/Clippy/format passed. Do not infer an
all-green workspace or hosted build-CI result from the scoped receipts.

Preserve all supplied GPL and dependency notices, including fake-08 borrowed
code/CC BY-SA 3.0 attribution. Pinned Swan retains its MIT OpenBIOS binary,
header and notice; the exact upstream firmware-source revision is unverified.
Unchanged corresponding materials are not a new legal/source-completeness
clearance. No Sony/user BIOS, commercial ROM, credentials or game art is added.

## Installation and rollback

Back up the intended FAT32 card, including games, BIOS, saves, states, settings
and custom splash. Keep test.11 available for rollback. Power off normally;
never hot-replace the running executable. Verify `SHA256SUMS`, extract into a
fresh host directory and confirm empty `Slot/Logs` exists. Merge the four root
folders **Slot, Apps, System, trimui** into the card root without deleting user
content, custom splash or settings. Stop on an unexplained baseline conflict.
Eject safely and confirm build label **stock-tsp-0.3.0-test.12**.

The owner installs/tests their own card; release permission is not agent
installation permission. Restore verified prior payload files for rollback.
For powered-off offline stock-menu fallback, see the existing stock notes;
do not guess or write an internal boot-logo target.
