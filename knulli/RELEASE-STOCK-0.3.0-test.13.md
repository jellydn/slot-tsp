# Stock TSP test.13 — Input Test hold-to-exit

Experimental stock-firmware FAT32 card content for **TrimUI Smart Pro TG5040**.
Not an OS image, firmware updater, binding repair or performance release.
Stable 0.2 remains Latest. The owner installs and tests their own card.

## Changes and scoped successes

- **Hold Menu continuously for one second to leave Input Test.** B release
  and short Menu taps stay in the screen. Menu modifiers can be previewed
  before the threshold; even a modifier hold can exit safely. No shortcut
  executes and no test input reaches a game. Reopen to clear history.
- The owner reports standalone test.12 detects all physical TSP controls.
  This does not establish in-game PS1 bindings, analog compatibility or speed.
- Test.11 startup packaging works according to the owner; retain empty
  `Slot/Logs` on fresh extraction. PICO art is explicitly good and unchanged.
- Rebuild the frontend with the test.13 label, committed source and receipts.
  Preserve pinned core bytes, web/startup files, licences and save safeguards.

## Optional free homebrew test pack

Download **slot-tsp-0.3.0-test.13-homebrew.zip** separately from this release.
It adds one unmodified upstream game per actual supported console, with
source archives, immutable provenance, licences, attributions and controls.
The main runtime ZIP stays ROM-free. These samples have static format/hash
checks, **not stock-core boot or gameplay acceptance** or independent rebuilds.

| Shelf | Sample | Terms |
| --- | --- | --- |
| GB | Game Boy Raw Snake — Albert Gonzalez | Unlicense |
| GBC | uCity v1.3 compatibility ROM — Antonio Nino Diaz | GPL-3.0+, BSD player, CC BY-SA 4.0 media |
| GBA | Xorlix — Erik Gillespie | MIT, public-domain startup code |
| PS1 | VoXide 0.4.1 — Bonnie Studios / EBonura | GPL-2.0+, CC0 assets, CC BY 4.0 music; dependency notices retained |
| PICO-8 | Sixlets — aquova | CC0 self-contained source cart |

GBAV remains empty: it is a GBA Video shelf, and no licensed actual homebrew
video was verified. Do not substitute a commercial video or mislabel a game.

After installing the main ZIP, extract the companion into a fresh host folder
and merge **Slot** into the powered-off card root. Preserve existing files;
stop if a destination game has different bytes. Keep source and licence files
under `Slot/Homebrew`. The shelf scans direct files: keep `voxide.cue` and
`voxide.bin` together directly in `Slot/Games/PS`, not in a subfolder.
No settings, BIOS, saves, labels, cores or startup scripts are added by the kit.

VoXide needs a mapped right stick for the camera. Test DualShock with PCSX
and Swan separately; do not share cards. Refuse any format request involving
valuable saves. Its upstream compatibility list is not a stock-TSP guarantee.
The official 0.4.1 download matches the pinned author release image; the pack
includes matching guest/SDK source and notices, but not a tested offline build
kit. Full provenance and music attribution are in `Slot/Homebrew`.

The next CUE/M3U/region/multidisc improvements are **not included in test.13**.

## Known issues / TODO

- [ ] **PS1 in-game input:** owner is testing PCSX/Swan shoulders and sticks.
  Record game, core, controller mode and failed action. Correct standalone
  preview is not proof of correct in-game routing. No guessed remapping.
- [ ] **PS1 performance/audio:** sustained speed and choppy sound remain open;
  Bloody Roar II is not claimed fixed. No blind frameskip, clocks or gain.
- [ ] **One early Slot logo:** internal candidate and BMP format are known
  historically, but current identity, consistent p1 backup, exact early loader
  and recovery validation remain gates. Keep the working later SD splash.
  Backup permission is deferred while the owner tests; no device access now.
- [ ] **Startup/first-key volume sync:** stock setter/status exists, but status
  reflects shared state and does not prove mixer success. Keep one stock
  volume owner and unity PCM gain; no competing attenuation/startup replay.
- [ ] **Automatic PS1 art:** existing Slot provider has no PS1 catalogue.
  Requires approved provider access/terms and safe region/revision/multidisc
  identity. Manual PNG upload remains fallback, not an automatic solution.
- [ ] **Swan GPU transport:** unfinished attachment experiments stay excluded.
  Production remains Software 1x/PGXP off; host probes are source-only, not
  handheld presentation or performance acceptance.
- [ ] **Platform limits:** no physical TSP L3/R3 or substitute. Digital mode
  ignores analog gameplay axes. Not every game supports DualShock.
- [ ] **Power/LEDs:** decorative LED control, complete sleep/wake reliability
  and remaining physical Power-button cases are not accepted.
- [ ] **Source/legal limit:** retained MIT OpenBIOS binary/header/notices have
  an unverified exact upstream firmware-source revision. Unchanged materials
  are not a new legal/source-completeness clearance.

## Owner test procedure

1. Back up the card and retain test.12 for rollback. Verify external checksums,
   extract to a fresh host folder and confirm empty `Slot/Logs` exists.
   With the handheld powered off, merge **Slot, Apps, System, trimui** into
   the intended FAT32 card root. Preserve user files/settings/custom splash;
   do not hot-replace a running executable. Eject safely. Confirm test.13.
2. Select a PS1 cart, then **Menu → Controls → Input Test**. Repeat PCSX and
   Swan settings in Digital/DualShock. Compare raw → logical → mapped events
   for D-pad, face, Start/Select and L1/L2/R1/R2; move both sticks separately,
   together and back to center. Triggers use ABS axes, not digital KEY events.
3. Test B down/up and short Menu taps: stay open. Preview Menu modifiers,
   then hold Menu for one second: return. Release all buttons before navigating.
4. Test actual gameplay separately. Preserve PCSX default and separate Swan
   cards; never move cards/states between cores. Swan states, quick states,
   automatic resume and rewind remain disabled.

## Verification scope

Attached build/package receipts report the actual fresh checks and exclusions.
Pinned Linux ARM64 build and host tests are not handheld acceptance. Optional
real-core tests require external libraries and may remain ignored; the known
baseline fast-forward timing case is explicitly skipped. No all-green workspace
or hosted build-CI claim. Reused dependency caches are not a second reproducible
binary build. No live device/SD access, profiling, internal write or merge is
part of release preparation. Release permission does not authorise installation.
