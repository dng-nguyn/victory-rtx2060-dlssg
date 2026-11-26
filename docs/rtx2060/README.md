# VIctory on an RTX 2060 6 GB, with DLSS Frame Generation (work log)

Branch `rtx2060-full`: fxceless/victory-fork `main` plus picked fixes from other forks and this work.
No game files are in this repository. Tested on one PC: RTX 2060 6 GB (driver 610.88), i3-12100F, 32 GB RAM,
game on a USB 5400 rpm hard drive.

## State (2026-11-26)

| | |
|---|---|
| Boot, menus, start screen | works, 30 fps |
| Opening cutscenes, prison prologue | play; player control reached ("Go with the prison guard"), 2-5 fps (4-5 fps with fxceless's newest switches) |
| Saves | load, but "COULDN'T LOAD" comes and goes on unchanged files; relaunching works |
| Series S profile (`--console series-s`) | renders 1280x720 -> 1920x1080, world picture black |
| `VICTORY_DISPLAY_MODE` | does not change the output: the title picks its resolution from the console model |
| DLSS Frame Generation | runs end to end (see below), but DLSS-G's pacer needs real frames under 100 ms; here they take 190-500 ms |

## DLSS Frame Generation

Off unless `VICTORY_DLSSG=<frames generated per real frame>` (1-5).

- `crates/victory-hle/src/modules/ffx.rs`: wraps the title's FidelityFX loader (`ffxDispatch`, `ffxCreateContext`, `ffxQuery`);
  the title's own DLL is still loaded (`registry::replaces`, `WRAPPERS`). In this title's mode the FidelityFX API is never called.
- `crates/victory-gpu/src/dlssg.rs`: loads `native/victory_sl` (C++ bridge to NVIDIA Streamline 2.14.1: manual hooking,
  frame-based resource tagging, OTA off); the host queue and swap chain are made through Streamline; at present it tags the
  title's depth and motion vectors and sets common constants and Reflex/PCL markers.
- The **scout** (`dlssg.rs`, hook in `draw.rs` `set_targets`) counts the render targets and depth buffers the title binds per
  frame and picks a two-channel 16-bit target (motion vectors) with a depth buffer of the same size, the most bound pair
  (2560x1440 in the Series X prologue; the 2048x8192 shadow atlas is the largest depth buffer and has no partner).
  Knobs: `VICTORY_DLSSG_MVEC_SCALE_X/Y` (default 1), `_DEPTH_INVERTED` (1), `_FOV_DEG` (60), `_NEAR`, `_FAR` (not verified).
- NVIDIA's runtime refuses DLSS-G on RTX 20 ("not supported on current hardware"). With the user's own copy of
  MFG-Unlocker v1.7.2 loaded as `winmm.dll` beside `victory.exe`, Streamline reports it supported (max 5 generated
  frames; MFG-Unlocker limits titles not on NVIDIA's list to 2x). MFG-Unlocker is not part of this repository.

Measured (Series X profile, prologue): tags accepted (`result=0`) every world frame; DLSS-G logs
`Frame rate over 100.00ms, resetting frame timer`, CPU pacer timeouts, and VRAM 6009 MB of a 5187 MB budget; real fps fell
about 4 -> 2 with 4x. Series S: no VRAM or pacer warnings, 5 fps, but a black world picture.

## Building

Rust 1.98.1 (`rust-toolchain.toml`), MSVC build tools. Bridge: `native/victory_sl/build.bat <streamline-sdk-2.14.1> <out dir>`;
put the bridge next to Streamline's `bin/x64` DLLs and point `VICTORY_SL_DIR` at that folder.

## Also here

- `docs/rtx2060/handoff-log.md`: the full chronological log (runs, findings, fork commits taken and skipped).
- `docs/rtx2060/patches-anyxboxx/`: the same DLSS-G work applied to LooperSalty/AnyXboxX's `third_party/victory`
  (Neptune "VIctory Edit." base); apply with `git am --directory=third_party/victory`.
- `scripts/local/`: the launchers and the window helper used on the test PC (paths are that PC's).

## Next

1. Re-test the size-matched scout on the AnyXboxX/Neptune build (its first run picked the shadow atlas in many frames).
2. Raise the base frame rate under 100 ms a frame (Neptune base, Belobug's hoisted indirect clamps, game files on an SSD).
3. Find why the Series S world renders black.
4. Verify the motion-vector scale and depth convention against the title's data.

Credits: VIctory (Marvisak) and the forks named in `handoff-log.md`; GPL-3.0-or-later.
