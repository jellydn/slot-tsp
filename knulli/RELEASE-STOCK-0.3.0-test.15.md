# Stock TSP test.15 — upstream integration

Experimental FAT32 copy-to-SD package for TrimUI Smart Pro TG5040.
Not an OS image or firmware update. Stable 0.2 remains unchanged.

## Changes

- Integrates the fork's current `knulli-tsp` base into PR #5 without rebasing
  or replacing its PS1/PICO-8 commits. Upstream is `slot-cfw/slot`, branch `main`.
- Combines upstream collections, overlays, settings and GPU frame blending
  with native PS1/PICO-8 frames, bezel pages, Input Test and Performance HUD.
- Keeps collection X-move and hold-X collection movement. On normal shelves,
  default X still opens the guarded named-file deletion prompt with Cancel
  selected. Referenced discs, saves, cards, states, configuration and art stay.
- Keeps native frame geometry and discontinuity markers together when frames
  are dropped or recycled. Native frame-size changes resize both GPU textures.
- Fits native PS1 and PICO-8 pictures in framebuffer pixels, so upstream's
  wider canvas does not stretch 4:3 or square frames. Nintendo wide-panel
  pictures retain their prior size with default, absent and custom bezels.
- Updates input-device discovery without losing TrimUI mappings, stick-to-D-pad
  reconciliation or the standalone raw/logical/mapped Input Test. Held-key
  discovery does not block when opening a FIFO test input.
- Uses upstream's explicit Turbo A/B bindings instead of the removed global
  Turbo switch. Existing upstream configuration migration remains in place.

## Preserved behavior and known limits

PCSX remains the default PS1 core. SwanStation remains Software, 1x, PGXP off,
with isolated cards and no states, quick states, automatic resume or rewind.
PICO-8 keeps local label extraction, exact-core-build states and cartdata;
rewind remains disabled. Pinned core binaries, metadata, source archives and
licences are carried forward unchanged from test.14. The frontend and web
assets are freshly built; see BUILD-INFO.json for the actual compiler and
source identity. This is not a reproducible-build claim.

**The reported TSP DualShock all-button loss remains unreproduced and unfixed.**
No handheld gameplay, sustained speed, audio, sleep/wake or power-button
acceptance follows from host tests. Early logo/recovery, startup volume,
automatic PS1 art and LEDs remain open. Stock owns volume and system power.
No device, SD card, user game, Sony BIOS or private configuration is accessed
or included in this release.

RomM remains bounded to flat raw BIN/CUE/M3U closure. Abrupt interruption can
leave hidden payloads that block retry; no automatic scavenging is added.
Swan's retained OpenBIOS materials do not establish its exact firmware-source
revision. Historical hardware observations and limits in test.14 still apply.

## Verification and installation

Read the attached VERIFICATION.json for actual completed checks, failures,
ignored external-core tests and scope. A successful cross-build or screenshot
is not hardware acceptance. No all-green CI claim is made without check results.

Verify SHA256SUMS and extract into a fresh host folder. Confirm `Slot/Logs`
exists. Back up the powered-off card and retain test.14 for rollback. Copy the
package's Slot, Apps, System and trimui trees to the FAT32 root while preserving
games, BIOS, saves/cards/states, settings, custom artwork and splash. Never
replace a running executable. Safely eject and confirm the test.15 build label.

Test collection move versus shelf deletion first, then both PS1 cores/modes,
physical sticks/buttons, PICO labels/states and Nintendo blending/overlays.
Hold Menu for one second to exit Input Test. Do not format valuable cards or
erase settings to troubleshoot. This release does not install itself or
promote the experimental branch to stable.
