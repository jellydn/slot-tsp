# Stock TSP test.14 — Digital controls, RomM bundles and Screen fixes

Experimental **FAT32 copy-to-SD content for TrimUI Smart Pro TG5040**.
Not an OS image or firmware updater. Stable 0.2 stays Latest, unchanged.

**The owner's TSP DualShock all-button loss remains unreproduced and unfixed.**
This build includes completed, host-verified changes and diagnostics, not a
complete PS1 input fix or handheld acceptance. No device/SD action was used
to prepare it. Installation and device testing belong to the owner.

## Implemented changes

- PS1 **Digital** now uses the existing saved **Left Stick as D-Pad** setting
  on both cores. The row is available in Digital mode. Held directions release
  on mode/setting changes without another axis event; physical D-pad priority
  and native axes remain. No default, forced analog mode or chord was added.
- RomM accepts complete flat raw 2352-byte BIN/BINARY-CUE bundles and
  M3U-ordered CUE sets. It fetches each file by ID, validates bounded confined
  references, then publishes one top-level playlist last without replacement.
  Existing games are not overwritten. Directory sync precedes success.
- Screen includes PlayStation/PICO-8 bezel pages and **Developer → Performance
  HUD**. The HUD remains session-only and starts off.
- GBA has opt-in Default/custom wide-panel side art. Bundled GB/GBC bezels
  extend their edge texture to the 16:9 panel edges without stretching or
  cropping the game. None's intentional aspect padding, custom art and 4:3
  behavior remain distinct.
- Default shelf **X** opens a named-file confirmation with Cancel selected.
  Only the selected ROM/descriptor is removed; referenced tracks/discs,
  saves/cards/states/config/art remain. Shared/unsafe references fail closed.
  Custom X bindings retain their action; Remove Recent can use another key.
  The existing SD write boundary also guards deletion.

## Host findings — not device acceptance

- Actual cached PCSX/Swan host libraries received FIFO input through the full
  frontend. An original authored guest negotiated real **0x73** replies and
  independently expected four-axis values, buttons/releases, live switches
  and resets. PCSX cross-mode state loads preserve input. Swan refuses state
  actions under its unchanged memory-card safeguard and retains input.
- Both missing-negotiation and zero-axis negative controls failed as required.
  These are not tests of the owner's game, configuration or handheld binary.
- Original Road Rash USA is documented as **digital-only**, not native
  DualShock analog: [manual SLUS-00035](https://www.videogamemanual.com/ps1/Road%20Rash%20%28LB%29%20%28USA%29.pdf),
  [compatibility entry](https://psxdatacenter.com/games/U/R/SLUS-00035.html).
  Library metadata names Road Rash (USA), separate from Jailbreak. The owner's
  actual disc serial/revision was not extracted. Do not substitute Road Rash
  3D or infer native analog support from physical stick compatibility.
- Default PS bindings keep plain L2/R2 out of FF/rewind. Persisted unbound or
  remapped triggers can expose those shortcuts. No owner bindings export was
  available, so this is a configuration-sensitive finding, not its diagnosis.

## Owner observations — retained, scoped

The owner reports test.13 at 50–60 fps, improved audio aligned with stock,
and a working standalone Input Test; PICO art was previously reported good.
The same reports identify both-core joystick problems, DualShock D-pad/ABXY
loss, L2/R2 FF/rewind behavior and a RomM failure. Positive standalone/performance
observations do not resolve those gameplay or transfer failures.

## Known issues and exclusions

- **DualShock on TSP remains unverified/unfixed.** Menu recovery, exact saved
  bindings, cold start versus resume and installed executable identity still
  need a failing-case report. Do not erase settings or saves to troubleshoot.
- RomM's initial bundle scope excludes nested files, cooked 2048-byte images,
  separate audio-only BINs, arbitrary ZIPs and incomplete descriptor closure.
  Handled cancellation/errors clean owned staging. Abrupt process/power loss
  can leave an invisible `.romm-<id>` payload that blocks retry; **no automatic
  cleanup, reuse or recovery** is provided. Do not delete retained content
  blindly. A post-commit sync error preserves the visible game and payload;
  inspect the library before retrying. The reported live game was not downloaded.
- Early internal logo/recovery, startup volume synchronization, transfer
  keep-awake/stock suspend, automatic PS1 art and full sleep/wake/LED behavior
  remain open. The working SD splash/manual art/PICO labels are preserved.
- PCSX stays default. Pinned core/source/licence bytes and save/card identity
  stay unchanged. Swan remains Software 1x/PGXP off, isolated cards, no states,
  quick states, automatic resume or rewind. No physical L3/R3 or rumble claim.
- All 12 uncommitted Swan attachment WIP paths are excluded. Committed graphics
  probes remain source-only; no hardware-renderer integration is installed.
  No games, Sony BIOS, credentials, personal config/logs or homebrew companion
  are added to this runtime ZIP. Existing test.13 homebrew assets are unchanged.
- Exact OpenBIOS upstream firmware-source revision remains unverified; retained
  notices/materials are not a new firmware reproducibility or legal clearance.
- Baseline exceptions are not repaired here: two binding-navigation tests,
  two menu renders, link-screen width and header-index persistence. The known
  FF timing case is skipped in scoped emulation runs; optional external-core
  tests may be ignored. Attached VERIFICATION.json gives actual runs/counts.
  No all-green workspace or hosted build-CI claim; untouched render Clippy and
  newer-toolchain UI lints remain outside the passing scoped checks.

## Owner checklist

1. Verify external SHA256SUMS. Extract to a fresh host folder and confirm
   `Slot/Logs` exists, even empty. Back up the powered-off card and retain
   test.13 for rollback. Copy **Slot, Apps, System, trimui** to its FAT32 root;
   preserve user games/BIOS/saves/cards/states/config/art/splash. Never replace
   a running executable. Safely eject and confirm the **test.14** build label.
2. In **Input Test**, compare raw → logical → mapped D-pad/ABXY, Start/Select,
   L1/L2/R1/R2 and each stick/center for both cores/modes. B and short Menu taps
   stay; hold Menu one second to exit. Release all buttons before navigation.
3. For original Road Rash, choose **Digital** and check the existing Left Stick
   as D-Pad setting. Test physical D-pad, stick steering, ABXY and L2/R2 combat
   on both cores. Record cold start versus resume; do not assume native analog.
4. For a known analog-capable game, test DualShock separately on both cores.
   If buttons fail, record whether Menu still works and whether switching back
   to Digital or normal Reset restores input. Attach redacted `Config/controls.txt`,
   `Config/shortcuts.txt`, `Config/slot.state`, build/core identity and the exact
   sequence. Do not share pairing/Wi-Fi credentials or move cards between cores.
5. Use a disposable, legally owned complete RomM bundle. Confirm one shelf entry
   and gameplay; check cancellation and collision refusal without deleting an
   existing game. Avoid sleep until transfer keep-awake behavior is established.
6. Check Screen → Developer and PS/PICO bezel independence. Compare None,
   Default/custom GB/GBC/GBA at the same game scale; no lost picture edges.
   Test X Cancel first, then delete only a disposable ROM/descriptor and verify
   its saves, cards and referenced payloads remain. Preserve valuable data.

Exact committed-source identity, runtime/source hashes, notices, fresh checks
and exclusions accompany the ZIP. Cached dependencies are reused; the recorded
build is not a second independent reproducible binary build. No merge or device
installation is implied by this experimental publication.
