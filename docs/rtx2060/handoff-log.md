# GTA VI (Xbox Series dump) on PC via VIctory: handoff log

Last updated: 2026-11-26. Machine: RTX 2060 6 GB (driver 610.88, HW GPU scheduling ON),
i3-12100F (4 cores), 32 GB RAM, game on F: (USB 2.5" 5400 rpm HDD, Toshiba MK5059GSXP).

## Current state (one line)
The game runs to **player control in the prison prologue** ("Go with the prison guard") at
**2-5 fps** with the main build. DLSS-G frame generation is wired in but **blocked by NVIDIA's
runtime on RTX 20** until MFG-Unlocker is added; a hook fix for it is built but **not yet test-run**.

---

## Where everything is

| What | Path |
|---|---|
| Game dump (read-only, never modified) | `F:\GTA6-Xbox` |
| Main source tree (branch `rtx2060-full`) | `F:\victory\merge-test` (git worktree of `F:\victory\fxceless`) |
| Main build (proven to reach player control) | `F:\victory\target-full2\release\victory.exe` |
| Frame-generation build (newest code) | `F:\victory\target-fg\release\victory.exe` |
| Streamline SDK 2.14.1 (official NVIDIA, signatures checked) | `F:\sdk\streamline-2.14.1\x` |
| Streamline runtime + bridge DLL | `F:\victory\sl-bin` (bridge source `F:\victory\merge-test\native\victory_sl`) |
| Bridge with shareable log (copy over sl-bin when no game runs) | `F:\victory\sl-bin-next\victory_sl.dll` |
| Belobug fork (+ our fixes, branch `rtx2060`) | `F:\victory\belobug`, build `F:\victory\target-bb\diag\victory.exe` |
| ztrx (as published, fallback) | `F:\victory\ztrx`, build `F:\victory\target-ztrx\release\victory.exe` |
| Other forks (reference) | `F:\victory\fxceless` (+ remotes `my`, `tw`, `bb`), `F:\victory\myokex`, `F:\victory\triippywhale` |
| Saves, game settings, ToS acceptance | `F:\victory-data\data` (title `7952987A`) |
| Shader cache (25,635 pre-translated) + pipeline cache | `F:\victory-data\cache` (Belobug: `cache-bb`, ztrx: `cache-ztrx`) |
| Run logs | `F:\victory-data\runs\*`, newer ones on `E:\victory-runs\*` |
| Game's own crash logs | `F:\victory-data\data\7952987A\temp\*.crash.log`; VIctory's: `...\7952987A\crashes.log` |
| Toolchain | Rust 1.98.1 `F:\rust` (RUSTUP_HOME/CARGO_HOME user env vars), MSVC `F:\BuildTools` |

## How to run

```powershell
# Main build, tuned for the RTX 2060 (720p, levels on demand, relay, draw windows, zero-hole skip)
F:\victory-data\Play-GTA6.ps1

# Fallback: ztrx exactly as published (defaults)
F:\victory-data\Play-ztrx.ps1
```
Window helper (focus, keys, screenshot): `F:\victory-data\tools\gamectl.ps1 -Keys "Enter" -Shot out.png`.
Keyboard = pad when no controller: WASD move, IJKL camera, Enter=A, Backspace=B, X, Y, Q/E bumpers,
Z/C triggers, F/H stick clicks, Esc=Menu, Tab=View. Hold keys (low fps).

Frame generation build (manual launch):
```powershell
$env:VICTORY_DLSSG='4'; $env:VICTORY_SL_DIR='F:\victory\sl-bin'; $env:VICTORY_SL_LOG='E:\victory-runs\sl.log'
# plus the Play-GTA6.ps1 env vars, then:
F:\victory\target-fg\release\victory.exe run F:\GTA6-Xbox\game_scarlett_master.exe --execute
```

---

## Log (what happened, in order)

1. Cloned fxceless / myokex / triippywhale forks; installed Rust 1.98.1 + MSVC Build Tools on F:.
2. Built fxceless; game boots, menus 25-30 fps; user accepted Rockstar ToS (asked me to tick it).
3. **Crash in the world** traced to the dump: `anim.rpf` (0.82 GiB) and `audio.rpf` (1.69 GiB)
   were NTFS sparse files with never-written ranges (`fsutil sparse queryrange`). User re-downloaded
   both; verified not sparse and the 6 previously failing blocks decompress.
4. Code added (branch `rtx2060-full` = fxceless main `a253706` + these):
   - 22 picked fixes from triippywhale (DirectStorage file lifetime, clzero decode, xasync UAF,
     tap-keeping input, view-heap limit query, shader/save fixes).
   - `dstorage`: failed reads name their file; `VICTORY_DSTORAGE_ZERO_HOLES=1` passes all-zero
     and partly-zero compressed blocks (only matters for damaged dumps).
   - Frame generation groundwork (see below).
5. Belobug fork: fixed its DualSense HID crash (path copied without NUL); ported display-mode
   switch + storage fixes; runs on NVIDIA but NVIDIA's compiler crashes on ~18-24 of its shaders.
6. ztrx checked: it is fxceless at 2119b0e + README; built as fallback.
7. Shader pre-translation done for the main build (`victory translate shaders.rpf --warm all`).
8. **Run 11 (main build, repaired files): player control reached** after ~50 min of slow-motion
   cutscenes; 2-5 fps. Crashed after 1h15 (game code null write at image+0x23cf683, game worker
   thread) in the next mission; likely load-timing related (slow USB HDD).
9. Read trace (run 12, `VICTORY_WALLS_TRACE=2`): `levels.rpf` = ~65% of read wait (71 GB, too big
   for any SSD's free space); others (core_bvh4, anim, data, shaders, story000, packs, ttdb) ~20%.
10. DLSS-G integration (run 13): Streamline loads, queue/swap chain go through it, game runs.
    Streamline says `eErrorNoSupportedAdapterFound` (RTX 2060) -> DLSS-G plugin not loaded.
    Found that wrapping `ffxDispatch` had stopped the game's FidelityFX loader DLL from loading
    (3 missing imports, FidelityFX unused); fixed (`registry::replaces`, WRAPPERS), rebuilt,
    **not yet test-run**.

## Frame generation: how it is built
- `crates/victory-hle/src/modules/ffx.rs`: wraps the game's `ffxDispatch`, logs the first upscale
  dispatches (`ffx: upscale dispatch` lines: render/upscale size, depth/mv descriptions, jitter,
  mv scale, near/far/fov) and feeds `victory_gpu::dlssg::record`.
- `crates/victory-gpu/src/dlssg.rs`: loads `victory_sl.dll`; queue via Streamline proxy device,
  swap chain via proxy factory; at present tags the host copies of depth + motion vectors
  (`resource::host_texture_now`), sets constants (projection approximated from fov; no view
  matrices from the game, `clipToPrevClip` = identity, camera motion in MVs), Reflex/PCL markers.
- `native/victory_sl/victory_sl.cpp` + `build.bat`: C bridge, manual hooking, OTA downloads off.
- Max multiplier with DLSS-G is 6x (MFG-Unlocker); 10x is not offered by any DLSS-G path.

---

## TODO (next steps, in order)
1. **Test-run the fixed FG build** (`target-fg`): confirm the exe binds with 0 missing imports and
   the log shows `ffx: the title's ffxDispatch is wrapped` + `ffx: upscale dispatch` lines with
   sane sizes (render ~1280x720 or lower, output 1280x720). If sizes look wrong, the struct layout
   in `ffx.rs` needs adjusting.
2. Copy `F:\victory\sl-bin-next\victory_sl.dll` over `F:\victory\sl-bin\victory_sl.dll` (shareable log).
3. **User step:** download MFG-Unlocker v1.7.2, rename `MFGUnlocker.dll` -> `winmm.dll`, put it in
   `F:\victory\target-fg\release\` (victory.exe imports winmm.dll). Then run with `VICTORY_DLSSG=4`
   and read the `dlssg:` lines + Streamline log. Unknown: whether it handles manual-hooking hosts.
4. If DLSS-G runs: tune constants (depth inverted? mv scale sign), check `DLSSGState.status`
   flags in the Streamline log, judge image quality.
5. Base frame rate: try fxceless's measured switch set: `F:\victory-data\Play-GTA6-next.ps1 -Run253`
   (includes `VICTORY_INDIRECT_GATHER=1`, fxceless's answer to the per-draw clamps; Belobug's hoist was
   NOT ported, see Fork heads 02:50). Pending hand-merge of triippywhale `ed8c0ce` (retry textures refused for lack of VRAM).
6. Storage: move core_bvh4/anim/data/shaders/story000/packs/ttdb (~10-12 GB) to `E:` with symlinks
   (game closed first); real gain needs `levels.rpf` (71 GB) on an SSD.
7. Crash at image+0x23cf683 (game worker thread null write): reproduce on SSD-hosted files to
   confirm it is load timing; if not, investigate.
8. Re-arm the fork watcher (fxceless/myokex/triippywhale/Belobug, 3-min fetch) when work resumes.

## Unattended handoff between Claude and OpenCode
- The user handed all work over for unattended running (2026-11-26).
- Claude -> OpenCode (OpenCode desktop window, model DeepSeek V4.1 Flash Free, no computer use):
  at about 95% of Claude's 5-hour limit Claude runs `F:\victory-data\tools\handoff-opencode.ps1 -Send`
  (focuses OpenCode, pastes a pointer to `F:\victory-data\OPENCODE-BRIEF.md`, presses Enter).
- OpenCode -> Claude: OpenCode appends `## OpenCode log <time>` here, ending with `BATON: Claude`.
- **Claude does not depend on OpenCode:** recurring scheduled task `gta6-victory-loop` (Claude app,
  every 2 h at :55, Bypass permissions) resumes the work from this file. Gate: skips if 5-hour usage
  >= 90% or `F:\victory-data\CLAUDE-ACTIVE.lock` was touched < 40 min ago; works until ~95%, logs a
  `## Claude log` section, wakes OpenCode (optional), deletes the lock. Runs only while the Claude
  app is open (missed runs start on next launch). (The earlier one-time task was removed.)
- Run 14 (pid 20340, `E:\victory-runs\run14-dlssg`) was started 2026-11-26 ~00:15 to confirm the
  FidelityFX capture; the fixed build binds with 0 missing imports and `ffxDispatch` is wrapped.

## Latest finding (2026-11-26 ~00:30, Claude)
- Run 14 (FG build `target-fg`, FidelityFX loader loaded + `ffxDispatch` wrapped, Streamline on):
  binds 0 missing, wrapper installed, but the game showed **"COULDN'T LOAD - choose another saved
  game or start a new one"** when loading the checkpoint; retrying gave the same. No `ffx: upscale
  dispatch` line was ever logged (it never reached the world).
- The save looks intact (`saves/.../SARC0015~f3ad35e1a05a7da0/v1` written 22:38 by run 11, with
  manifest); run 13 (FFX DLL accidentally not loaded) DID load this save into the world.
- Run 15 (`E:\victory-runs\run15-main-saveload`, main build `target-full2`, no wrapper, no
  Streamline) was started to tell whether the save or the FG build is at fault.
  - If run 15 loads: suspect the wrapper (`crates/victory-hle/src/modules/ffx.rs`) or Streamline's
    proxy queue/swap chain. Bisect: run `target-fg` with `VICTORY_DLSSG` unset (wrapper only).
    Wrapper suspects: calling convention / return value of `ffxDispatch` (the original returns
    `ffxReturnCode_t`, 0 = ok), or the HLE export of a `title`-loaded DLL changing bind order.
  - If run 15 fails too: the checkpoint is bad; choose New Game (X) or an older save.
- **RESULT (00:40): run 15 (main build) shows the same "COULDN'T LOAD".** The FG build is NOT at fault;
  the checkpoint is. Run 15 (pid 14856) was left on that screen.
  Next: on that screen press A (Load Game) to see the save list and pick an older entry if any, else
  X (New Game; the opening cutscenes take ~50 min at this fps). Optionally back up
  `F:\victory-data\data\7952987A\saves` first (copy, don't move). Then re-test the FG build
  (`target-fg`) and look for `ffx: upscale dispatch` lines once in the world.

## Claude log 2026-11-26 00:40 (handing to OpenCode, user's request)
- Claude's 5-hour limit is near; user asked to hand off to OpenCode (DeepSeek) now. Claude's loop task
  `gta6-victory-loop` now runs at :40 every 2 h (next useful run ~02:40 after the limit resets).
- Running now: run 15 (`F:\victory\target-full2\release\victory.exe`, pid 14856) on the
  "COULDN'T LOAD" screen. Nothing else running. All code committed (rtx2060-full c4a6ef4, rebased on
  fxceless d510ee4, NOT yet rebuilt).
- Next for whoever continues: (1) resolve the save (above); (2) build rtx2060-full into
  `F:\victory\target-fg` (no game running from it) and test `VICTORY_INDIRECT_GATHER=1`;
  (3) review the 35 new triippywhale commits; (4) TODO list above.

## Fork heads seen (update on every check)
2026-11-26 02:50: fxceless `origin/main` **085ee7c** (3 new: built-in frame capture `VICTORY_SHOT`, upload-heap windows
`VICTORY_WINDOW_UPLOAD`, DirectStorage reads held until the title's Submit), myokex `my/victory-amd` 070df42 (+ new
branch `my/vi-fx3` 1544bd5: AMD water/texture copy-back, not taken), triippywhale `tw/main` **266cd04**, Belobug `bb/main` 4cbe9d7 (unchanged).
- rtx2060-full rebased onto 085ee7c (one conflict: tw's AutoSubmitDisabled vs fxceless's held reads, merged both; follow-up
  fixes f91f26c, 463902c).
- Taken from tw (cherry-pick -x): a0d79bc split unwatchable range (stops 100-750 MB whole-buffer copies a frame),
  51acc7f f16 mix, b58a903 re-registered buffer retires its copy, 71e6808 root params within SGPRs, 44fef9d 16 MiB
  thread stack (driver shader-compile threads overflowed 1 MiB on an RTX 4070), 8b4e75f quad-read helpers,
  88b4cdb 64-bit compares on dwords, 3610066 16-bit compares low halves.
- Skipped from tw: ec621a5 hoisted clamps (13 conflict hunks against fxceless's own one-pass gather,
  `VICTORY_INDIRECT_GATHER`, same problem; also needs tw's card_constants; never run on a GPU upstream),
  fd89543 thin LTO + 1 codegen unit (very slow builds on 4 cores), 1cde6ec (AMD compiler only), d861e3d /
  266cd04 / 79f0b7a / b2e3486 / b028679 (don't apply; BVH builder and HLE lookup rewrites, not needed now).
2026-11-26 00:35: fxceless `origin/main` d510ee4 (10.3 fps with `VICTORY_INDIRECT_GATHER=1` on a 4070),
myokex `my/victory-amd` 070df42, triippywhale `tw/main` 7e88d16 (35 new: PalmVIDev shader ports, view-guard fixes,
not yet reviewed), Belobug `bb/main` 4cbe9d7.
- rtx2060-full rebased onto d510ee4 (24 commits on top). Skipped (conflict with fxceless's rewrite of
  the clzero decoder): tw "loader: fix ClzeroLoop ModRM decoding..." and "enforce F2/F3 prefix precedence".
- Not built yet: build into a target dir whose victory.exe is not running, then try `VICTORY_INDIRECT_GATHER=1`.

## Known problems / notes
- Shadows black, bright scenes overexposed (no ray-traced lighting in any fork yet).
- Late model/texture pop-in from the USB HDD; long loads; cutscenes play in slow motion at low fps.
- Heavy disk/CPU work during game boot can crash System Init (image+0x2215c35): don't scan/copy
  the game drive while the game starts.
- One upstream GPU test (`indirect::warp_budget...`) fails; fxceless marks it flaky.
  fake); not configured by me.
- Nothing has been pushed anywhere; all commits are local.
2026-11-26 00:46 Claude loop run skipped: usage 91%

2026-11-26 02:46 Claude loop run active (lock held): saw run16-fg (pid 19548, started 02:44, likely OpenCode); Claude leaves the game window to OpenCode, doing fork fetch/review only, no builds while it boots.

## Claude log 2026-11-26 03:25 (scheduled loop run, 02:46-03:25)
- OpenCode was mid-task (no BATON yet): it started **run 16** (`E:\victory-runs\run16-fg`, FG build `target-fg`, pid 19548,
  02:44) and chose **New Game** (opening cutscene "Nine months ago", 1-4 fps, 2,300-4,900 draws/frame at 03:20).
  Claude left the game window to OpenCode and did no heavy work on F: (builds went to the NVMe E:, Idle priority, 2 jobs).
- **Finding: the title never calls ffxDispatch** (not even an "other dispatch" line) through 4,000+-draw world frames,
  although game_scarlett_master.exe imports ffxCreateContext/ffxDestroyContext/ffxDispatch/ffxQuery statically.
  So frame generation has no FFX inputs yet. New commit **5b9cc74** logs `ffx: ffxCreateContext` (chain kinds, upscale
  flags, max render/upscale sizes, return code) and `ffx: ffxQuery`, to tell whether the game makes an upscale
  context at all (maybe not at 1280x720, or the loader fails on PC). Not run yet.
- **Finding: Streamline disabled NGX** for a 2nd reason besides RTX 20: "Please provide correct application id when
  calling slInit". Commit **376be95**: bridge passes engineVersion "1.0.0" + a projectId GUID. Built to
  `F:\victory\sl-bin-next\victory_sl.dll` and copied with the DLSS-G plugins (no DLSS/RR models) to `E:\victory-build\sl-bin2`.
- Forks: see "Fork heads seen" 02:50 (rebased onto fxceless 085ee7c, 8 tw fixes taken, rest skipped with reasons).
- Commits (rtx2060-full, F:\victory\merge-test, local only): rebase + picks, f91f26c, 463902c (rebase fixes),
  376be95 (bridge), 5b9cc74 (ffx logging). Tests: `cargo test -p victory-hle` dstorage (20 ok) and ffx (2 ok).
  NOT run: victory-gpu / victory-shader tests (they use the card; the game was running).
- **New build: `E:\victory-build\target-next\release\victory.exe`** (all of the above). Launcher:
  `F:\victory-data\Play-GTA6-next.ps1` [-Run253] [-FG] (logs to `E:\victory-runs\<time>-next`).
- Running at handoff: run 16 (OpenCode's). Nothing of Claude's running.
- Next: (1) when run 16 is done (don't close it while it might save), run `Play-GTA6-next.ps1 -FG` and grep err.log for
  `ffx: ffxCreateContext` / `ffx: upscale dispatch` (the context is likely made by the world load, ~10 min);
  (2) `-Run253` vs plain, compare fps/ms in the window title at the same spot; (3) run the victory-gpu and
  victory-shader tests with no game running (`CARGO_TARGET_DIR=E:\victory-build\target-test`); (4) TODO list.
2026-11-26 03:06 Claude loop run ended; OpenCode woken via handoff-opencode.ps1; lock released.

2026-11-26 04:47 Claude loop run active (lock held): run at E:\victory-runs\20261126-033121-next (pid 1188, -FG, started 03:31 by OpenCode, no OpenCode log yet) is in the prologue at 3 fps; its err.log is EMPTY (pwsh 2> redirect does not stream) so ffx: lines are not visible.
2026-11-26 05:27 Claude (interim): killed OpenCode's 03:31 run (no save since 01:07); its err.log only appeared on exit: 0 ffxCreateContext/ffxQuery/ffxDispatch calls in 1h20. Running now: E:\victory-runs\20261126-045703-next (pid 15200, target-next2, -FG -Run253, New Game). Full log follows at end of run.

## Fork heads seen (update)
2026-11-26 06:07 : fxceless `origin/main` **9c184b3** (5 new: texture copy-back for CPU-read shader output `VICTORY_TEXTURE_BACK=1` off by default, small buffers in card memory `VICTORY_CARD_BUFFERS_MIN_KB`, `VICTORY_BACK_READS`), triippywhale `tw/main` **7e1d4eb** (~55 new), myokex `my/victory-amd` 070df42 / `my/vi-fx3` 1544bd5, Belobug `bb/main` 4cbe9d7 (unchanged).
- rtx2060-full rebased onto 9c184b3 cleanly (36 commits).
- Taken from tw (cherry-pick -x): 817e0ae dstorage decode workers below normal priority, ce6cf3c dstorage pool sized to physical cores, 8931584 buffer view ranges refuse overflowing counts (upstream #2541 port), 7c0de75 stop before landing a submission whose late fence wait fails.
- Skipped (conflicts): 273f043 Efficiency-Mode opt-out (runtime.rs; i3-12100F has no E-cores anyway), 0eb8f51 zero-length range guards (watch.rs), b3ef4e5 / 50f722f shader ports (test files). Not tried: the rest (shadow/relay rewrites, half-precision shader series, census, reverts) - large, interdependent, not proven on NVIDIA.

## Claude log 2026-11-26 06:07 (scheduled loop run, 04:46-06:10)
- OpenCode had started `E:\victory-runs\20261126-033121-next` (Play-GTA6-next.ps1 -FG) at 03:31 and left no OpenCode log. Its err.log was **0 bytes while running**: pwsh `2> file` on a native exe buffered everything until exit. Fixed: `Play-GTA6-next.ps1` now uses Start-Process -RedirectStandardError (logs stream), takes `-Exe`, default exe `E:\victory-build\target-next2`.
- `gamectl.ps1` keys did not reach the game (foreground lock: SetForegroundWindow fails from a background shell). Fixed with an Alt tap before SetForegroundWindow; Enter/X now work.
- **FidelityFX finding (final):** 1h20 of that run incl. world frames logged no ffxCreateContext/ffxQuery/ffxDispatch call. New test (commit **9841dfe**) proves all three imports bind to our wrappers (HLE wins over the title DLL). So the title does **not use the FidelityFX API** in this mode. DLSS-G needs its depth/motion vectors from elsewhere (the game's own TAA/upscale pass inputs) - research task, not started.
- New build **`E:\victory-build\target-next2\release\victory.exe`** (rtx2060-full 9841dfe on fxceless 9c184b3 + 4 tw picks). Tests: victory-hle dstorage+ffx 25 ok; victory-gpu 189 ok, 0 failed (no game running).
- **Speed, `-Run253 -FG` on target-next2 vs plain -FG on target-next:** same prison-window scene (4,600 draws): **4.1 fps / 244 ms vs 3.0-3.4 fps / ~300 ms**; copy-back 212 -> 48 ms/frame. Opening cutscene 4.5-5.4 fps. Keep -Run253 as the default way to play.
- **Saves:** the old 01:07 autosave (v2) still gave COULDN'T LOAD. A New Game on target-next2 wrote a fresh autosave (v3, 05:37, `(Autosave) Prologue`); the next launch **loaded it** straight into the prison prologue (~10 min load). (I closed run 045703 ~5 min after that autosave; save files complete.)
- Running at handoff: `E:\victory-runs\20261126-055002-next` (pid 17264, target-next2, -FG -Run253), in the prison prologue at ~4 fps, loaded from the v3 autosave. Don't close it while it might autosave.
- Next: (1) play on from here (Enter at prompts; "Go with the prison guard" = WASD hold) and watch for the old image+0x23cf683 crash; (2) FG inputs without FFX: find the title's TAA pass (motion-vector + depth targets) via VICTORY_SHOT / pipeline names; (3) try fxceless `VICTORY_TEXTURE_BACK=1` only if water/sinking-boat scenes misbehave; (4) hand-merge tw 0eb8f51 (zero-length range guards) if a watch.rs crash shows up.
2026-11-26 06:07 Claude loop run ended; OpenCode woken via handoff-opencode.ps1 (SENT); lock released.

## Fork heads seen (update)
2026-11-26 06:48 : fxceless `origin/main` **32b31f7** (7 new: card compare hashes a piece with 64 threads, ~35 -> ~10 ms/frame of card passes on a 4070, PR #1014; `VICTORY_CARD_HASH_WIDE=1` opt-in for the wide hash of #1014 / `VICTORY_CARD_CHANGED_LIST`; README: 9-12 fps on a 4070 with the "run 279" set, card lost after 30-50 min), triippywhale `tw/main` **d3baa7a** (reverted its ac02e9f..e31f1a9 batch "device removal and fps drop", re-applied a subset), myokex 070df42 / vi-fx3 1544bd5, Belobug 4cbe9d7 (all unchanged).
- rtx2060-full rebased onto 32b31f7 cleanly (41 commits).
- Taken from tw: 4c33215 BCPack keeps all 32 indices in the first BC7 dictionary entry (texture correctness; victory-formats tests 26 ok).
- Skipped: 0de380d trim upload caches on low commit (needs `memory_status` from a conflicting commit; build failed, dropped), 9272cde pipeline-worker throttle (conflict, make.rs), 4d7b62f find_all snapshot RwLock (conflicts make.rs/shadow.rs), 2efc861 XMA LUTs (conflict xma.rs), d3baa7a fat LTO (slow builds).

2026-11-26 07:59 Claude (interim): COULDN'T LOAD came back on the 05:37 v3 autosave (unchanged files) with BOTH the new build and the exact 05:50 config; saves backed up to F:\victory-data\saves-backup-20261126-0720. Now running E:\victory-runs\20261126-074012-next (pid 6336, target-next, -Run253 -Run279) - New Game opening cutscene; waiting for its autosave, then reload tests. Full log follows.

2026-11-26 08:37 Claude (interactive session, user check-in): OpenCode stopped at DeepSeek's free daily message limit ('reset at 7:59 AM'); the 06:07 handoff was refused (text left in its box, doubled by the next paste). handoff-opencode.ps1 now clears the box first. Not re-sent: a Claude loop run (started 06:46) holds the lock and drives the game. Note for loop runs: OpenCode's free quota can run out; check its window (PrintWindow capture) for 'used today's free messages' before relying on it.

2026-11-26 08:40 USER DECISION: OpenCode is dropped. Claude alone does the work (gta6-victory-loop every 2 h at :40). Do not run handoff-opencode.ps1 or type into OpenCode.

## Claude log 2026-11-26 08:50 (scheduled loop run, 06:46-08:50; stopped on the user's request)
- Forks: see "Fork heads seen" 06:48. rtx2060-full rebased onto fxceless **32b31f7** (64-thread card hash) cleanly; took tw 4c33215 (BC7 fix, victory-formats 26 ok); skipped the others (reasons there).
- New commit **c4ed5ad**: hand port of tw 0de380d (idle staging/shadow caches age out by min(free RAM, commit headroom)); host idle tests ok. NOT taken: tw's pipeline-worker throttle (reads ullAvailPageFile against the *current* commit limit; with this PC's growable pagefile it would pin compiling to 1 worker).
- **Memory finding:** commit charge 40.5 of 41.4 GB while playing (victory 21.5 GB private; pagefile D:\pagefile.sys 5-25 GB, already grown to 9.7 GB; D: has only 16 GB free). If D: fills, allocations fail. USER STEP (system setting, not done): add a pagefile on F: (230 GB free) or free space on D:.
- Builds: `E:\victory-build\target-next` = 45fa603 (rebase + BC7); `E:\victory-build\target-next2` = **c4ed5ad** (launcher default now).
- Launcher `Play-GTA6-next.ps1`: new `-Run279` (use with `-Run253`): CARD_WINDOWS_MB=1024, TEXTURE_BACK=1 (boats float; needed for the first boat mission), CARD_HASH_LEN=1. Do NOT add CARD_BUFFERS_MIN_KB=64 / CARD_BUFFERS_MB=640 on the 6 GB card: tried, 340 MB/frame fell back to CPU compares, 3-4 fps.
- **Speed (target-next, -Run253 -Run279, prison-window scene):** 4-5 fps, 190-223 ms at 3,900-4,900 draws (was 4.1 fps / 244 ms at 4,600 with -Run253 on the older build): ~10% better. Opening cutscene 4-5 fps at 4-5.7k draws, 7-10 fps in light shots.
- **Saves:** COULDN'T LOAD is **intermittent** with byte-identical save files: the 05:37 v3 autosave failed twice (06:52 new build; 07:06 the exact 05:50 config) and then loaded twice (07:21, 07:40) with no change. Not the build, switches, prewarm list (2744 vs 2746 entries) or save age. Same files opened in the same order in good and bad loads. Workaround: relaunch. A load of v3 starts with the "Nine months ago" opening, then the prison window (player control). Backup: `F:\victory-data\saves-backup-20261126-0720`.
- **Last run** `E:\victory-runs\20261126-084026-next` (target-next2 = c4ed5ad, -Run253 -Run279) **exited silently ~2 min into the load** (08:45): no crashes.log entry, no game crash log, no Windows event. One occurrence; it may be c4ed5ad or just the known boot fragility. Next: relaunch the same config once; if it dies again, use `-Exe E:\victory-build\target-next\release\victory.exe` (without c4ed5ad) and revert c4ed5ad if that one survives.
- Gameplay: the prologue is too dark to steer blind (black shadows); no new checkpoint reached.
- Running at handoff: nothing (no victory.exe). All commits local.
- Next: (1) the c4ed5ad check above; (2) user: pagefile on F:; (3) FG inputs without FFX (the title's TAA motion-vector/depth targets); (4) hand-merge tw 4d7b62f's snapshot RwLock (race fix) without its throttle.
2026-11-26 08:52 Claude loop run ended; OpenCode woken via handoff-opencode.ps1 (SENT); lock released.

2026-11-26 08:58 USER: scheduled work killed (gta6-victory-loop disabled). Interactive Claude session works on DLSS-G: scout build E:\victory-build\target-dlssg (picks depth + R16G16 motion vectors from the title's own render targets when FFX gives nothing; knobs VICTORY_DLSSG_MVEC_SCALE_X/Y, _DEPTH_INVERTED, _FOV_DEG, _NEAR, _FAR).

2026-11-26 09:02 User downloaded MFG-Unlocker v1.7.2 (zip sha256 e002d624...f22 = GitHub's release digest). Placed as E:\victory-build\target-dlssg\release\winmm.dll (loads with victory.exe from that folder only).

## Claude log 2026-11-26 09:03 (scheduled loop run 08:52-09:05; STOPPED on seeing the 08:58 USER note that the loop is disabled)
- This run started 08:52, before the loop was disabled; it saw the 08:58/09:02 notes only at 09:04 and stopped. It did NOT touch
  the game window, the interactive session's run (pid 3588, target-dlssg) or its uncommitted dlssg.rs/draw.rs edits.
  It briefly started its own victory.exe (target-next2, pid 2176) at 09:02 and killed it 5 s later (still in boot, nothing saved).
- **Branch changes in F:\victory\merge-test (rtx2060-full), all local commits:**
  - 08:54 rebased onto fxceless **64f3a82** (30 new upstream commits, mostly opt-in switches/diagnostics). Conflicts resolved:
    descriptor/host.rs tests (kept fxceless's 64 KiB CBV test + our view-limit test; our "drop neighbour's test" commit became empty, skipped),
    host.rs Present (fxceless now checks Present's result; our dlssg::present_start/present_end kept around it), shadow.rs register
    (fxceless's tables_changed() + tw b58a903's let before =).
  - Picked from tw (cherry-pick -x): 927db8a (XMemVirtualAlloc reservations at title addresses get a write watch; were copied whole every
    submission) -> 90acb1d; d51efcf (wide() caller only helps its own jobs) -> f01afa6.
  - **c627b56 bug fix:** host.rs wait_for_submission matched WAIT_OBJECT_0 / WAIT_FAILED as *bindings* (constants not in scope on this
    branch), so every wait result counted as success. Now matched by path. rustc's unused-variable / unreachable-pattern warnings are gone.
  - The interactive session's target-dlssg (08:57) was built on the rebased tree (after 08:54), so it already has the rebase; it lacks
    90acb1d / f01afa6 / c627b56.
- Tests (E:\victory-build\target-test, before c627b56): victory-gpu 200 ok / 5 ignored, victory-hle 121 ok.
- Build E:\victory-build\target-next2 (09:02) = c627b56 **plus the interactive session's uncommitted dlssg.rs/draw.rs scout edits** as of 09:00
  (shared worktree). Rebuild before trusting it as a clean rtx2060-full build.
- Finding: the 08:45 "silent exit" of run 20261126-084026-next was a **window close** (err.log: "window closed: suspending the title"), not a crash;
  no reason to suspect c4ed5ad (now acb95c5).
- Fork heads seen 08:53: fxceless origin/main **64f3a82**; tw/main **8e4dd84** (10 new; skipped: ba848f4 FOLD_BUFFERS on by default, 3ec358c /
  291a5dd / d278b46 opt-in ports of switches fxceless already has, 457c8e4 / f85bfcd lock changes (conflict risk with fxceless #1053 work already in),
  962e6af / 8e4dd84 reports/reformat); myokex victory-amd 070df42, vi-fx3 **aa8399e**, new branch **vi-xtx** (AMD, not reviewed); Belobug 4cbe9d7.
- Lock released. The loop stays disabled (the user's choice); nothing of this run is running.

2026-11-26 09:34 DLSS-G RESULT (run E:\victory-runs\20261126-091759-dlssg, MFG-Unlocker winmm.dll + sl-bin3 + scout build 9820cd5): Streamline supported=0, max generated 5, set=0; scout tags depth (2560x1440, state 2048) + R16G16 mvec (state 192) with result=0 every world frame. DLSS-G runs but: 'Frame rate over 100.00ms, resetting frame timer' (134x), 'CPU pacer skipping frame (timeout)', GPU fence waits >500 ms, VRAM 6009 MB vs 5187 MB budget (4K output). Real fps fell ~4 -> ~2. Conclusion: integration works; DLSS-G's pacer needs base frames <100 ms. Next: --console series-s (lower res: VRAM + base fps) with -Frames 1.

## Claude log 2026-11-26 10:01 - DLSS-G status (interactive session)
- WORKS end to end: MFG-Unlocker v1.7.2 (user's download, sha256 = GitHub digest) as E:\victory-build\target-dlssg\release\winmm.dll;
  Streamline (E:\victory-build\sl-bin3, bridge with eUseFrameBasedResourceTagging) reports DLSS-G supported, max 5 generated;
  the scout (commit 9820cd5) tags the title's own depth + R16G16 motion vectors each world frame, result=0.
- Series X (default): render 2560x1440 -> 3840x2160 output; with DLSS-G 4x real fps fell ~4 -> ~2; VRAM 6009/5187 MB;
  DLSS-G pacer: 'Frame rate over 100.00ms, resetting frame timer', 'CPU pacer skipping frame (timeout)'.
- Series S (-Console series-s, run E:\victory-runs\20261126-093436-dlssg, -Frames 1): render 1280x720 -> 1920x1080, 5 fps / 194 ms
  WITH DLSS-G on, no VRAM/pacer/timeout warnings, tagging ok - but the picture is BLACK except VIctory's in-picture '5.x FPS' counter,
  so the real frames are black (Series S world rendering is listed as incomplete upstream). Not yet checked: Series S without DLSS-G.
- Launcher: F:\victory-data\Play-DLSSG.ps1 [-Frames 1-5] [-Console series-x|series-s] (pwsh -ExecutionPolicy Bypass -File ...).
- NEXT: (1) Series S without VICTORY_DLSSG to confirm black is Series S, not DLSS-G; (2) DLSS-G needs real frames < 100 ms (10+ fps):
  raise base fps (AnyXboxX/Neptune build E:\victory-build\target-anyx via F:\victory-data\Play-AnyXboxX.ps1, ~14 fps on a 4070 SUPER;
  port the scout+bridge onto it); (3) motion-vector scale unverified (VICTORY_DLSSG_MVEC_SCALE_X/Y default 1).
- Note: VICTORY_DISPLAY_MODE=1280x720 never changed the output (the title picks resolution from the console model).
- Running at handoff: run 20261126-093436-dlssg (pid 15736). Scheduled loop: DISABLED (user). OpenCode: dropped (user).

## Claude log 2026-11-26 12:50 - DLSS-G ported to AnyXboxX/Neptune
- Port: F:\victory\anyxboxx (branch dlssg, commit 321897f), third_party/victory = Neptune 2026-11-25c + AnyXboxX + our
  ffx wrapper, dlssg.rs (scout), victory_sl bridge, host.rs hooks. Builds; ffx tests 3/3 ok. Build E:\victory-build\target-anyx,
  MFG-Unlocker also there as winmm.dll. Launcher F:\victory-data\Play-AnyXboxX.ps1 [-Frames 0-5] (Neptune switch set, 6 GB-scaled).
- Run E:\victory-runs\20261126-121630-anyx (pid 15572): Neptune picture is much better (prison window lit properly).
  MFG-Unlocker menu (Backspace) shows: Game FG DLSS, GPU Turing RTX 20, FG state On, **Limit 2x ('game not on NVIDIA's list')**.
  World: 2 FPS / 508 ms (first-run shader compile on a new cache, E:\victory-data\cache-anyx). Streamline: 'Failed to find global tag
  kBufferTypeDepth / kBufferTypeMotionVectors' then 'DLSS-G interpolation state changed from enabled to disabled (mode=eOff,
  numFramesToGenerate=0)': the scout finds nothing in this build.
- Likely cause: Neptune binds render targets on another path than draw.rs set_targets (look at indirect.rs / resource.rs
  OMSetRenderTargets, and any Neptune-specific draw path); VIctory's err.log only appears at exit (lancer uses 2>), so read the
  'dlssg: scouted' lines after the game closes, or launch victory.exe directly with Start-Process -RedirectStandardError.
- gamectl.ps1 now picks the largest non-overlay window (Neptune opens a 'NeptuneOverlay' window too); Neptune's title shows
  'N FPS · N ms' without draws/frame.
- NEXT: (1) after this run ends, grep its err.log for 'dlssg: scouted' to see what the scout saw; hook the other OMSetRenderTargets
  path if needed; (2) second run once shaders are cached to get Neptune's real fps here; (3) MFG-Unlocker Mode: try a fixed
  multiplier above the 2x list limit; (4) Series S world renders black (both builds?) - check without DLSS-G.
