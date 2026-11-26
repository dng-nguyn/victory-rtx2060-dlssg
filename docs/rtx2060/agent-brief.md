# Brief for OpenCode: you are taking over from Claude (unattended)

The user handed this project to Claude to run unattended; Claude is near its usage limit and hands it
to you. Read `F:\victory-data\HANDOFF.md` first: state, every path, how to run, history, TODO.

## Goal
GTA VI (the user's own Xbox Series dump at `F:\GTA6-Xbox`) runs on this PC through VIctory (a Rust
compatibility layer). It already reaches player control at 2-5 fps. Current work: DLSS frame
generation through NVIDIA Streamline (code in `F:\victory\merge-test`, branch `rtx2060-full`).

## Rules (important)
- Never modify, move or delete anything in `F:\GTA6-Xbox` (the game files).
- Do not download or install third-party binaries (MFG-Unlocker is the USER's step; do not fetch it).
- Do not push to GitHub or post anywhere. Commit locally only; end commit messages with a line
  naming yourself (e.g. `Co-Authored-By: OpenCode`).
- Do not run disk- or CPU-heavy work (scans, copies, builds) while the game is booting: slow I/O
  crashes it in System Init.
- C: has ~16 GB free: keep builds, logs and caches on F: or E:.
- Never close or overwrite files a running game holds (`victory.exe` in use, `F:\victory\sl-bin\*.dll`).

## You have no computer use: drive the game with PowerShell
- Screenshot / keys for the game window: `pwsh -NoProfile -File F:\victory-data\tools\gamectl.ps1 -Shot <png> -WaitMs 100`
  and `-Keys "Enter" -HoldMs 600` (keys: Enter Backspace Esc Tab Up Down Left Right W A S D X Y Q E Z C F H, `wait500`).
  It prints the window title, which shows fps, ms/frame and draws/frame.
- Start screen: when the title shows ~13 draws/frame, press Enter (A). Loading then takes 5-10 min.
- Game state: `F:\victory-data\data\7952987A\temp\*.crash.log` (game's crash report), `...\7952987A\crashes.log` (VIctory's).

## Building (PowerShell)
```powershell
$env:RUSTUP_HOME='F:\rust\rustup'; $env:CARGO_HOME='F:\rust\cargo'; $env:Path="F:\rust\cargo\bin;$env:Path"
$env:CARGO_TARGET_DIR='F:\victory\target-fg'   # use another dir if that exe is running
Set-Location F:\victory\merge-test; cargo build --release -p victory
```
C++ bridge: `F:\victory\merge-test\native\victory_sl\build.bat <F:\sdk\streamline-2.14.1\x> <out dir>`.

## Running the frame-generation build
Set the env vars from `F:\victory-data\Play-GTA6.ps1` plus:
`VICTORY_DLSSG=4`, `VICTORY_SL_DIR=F:\victory\sl-bin`, `VICTORY_SL_LOG=E:\victory-runs\<run>\streamline.log`,
then `F:\victory\target-fg\release\victory.exe run F:\GTA6-Xbox\game_scarlett_master.exe --execute`
with working dir `F:\victory\merge-test`, stderr to `E:\victory-runs\<run>\err.log`.

## Where Claude stopped (2026-11-26 06:10)
Read the newest `## Claude log` in HANDOFF.md. In short: the game runs from `E:\victory-runs\20261126-055002-next`
(build `E:\victory-build\target-next2`, -FG -Run253, ~4 fps), loaded from a fresh autosave into the prison prologue.
The title never calls FidelityFX (proven), so DLSS-G has no inputs yet. `Play-GTA6-next.ps1` now streams logs;
`gamectl.ps1` keys now reach the window. Don't close the game while it may be autosaving.
## Handing back to Claude
Claude can't receive messages from you. When you stop (done, stuck, or out of budget):
1. Append a section `## OpenCode log <date time>` to `F:\victory-data\HANDOFF.md`: what you did,
   what you changed (commit ids), what is running, what failed, what's next.
2. End it with the line `BATON: Claude`.
A Claude session is scheduled to start after Claude's limit resets; it reads HANDOFF.md and continues.
