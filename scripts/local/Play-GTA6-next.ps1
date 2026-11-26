# Like Play-GTA6.ps1, but runs the newest build (E:\victory-build\target-next: rtx2060-full rebased on
# fxceless 085ee7c + 8 triippywhale fixes, 2026-11-26) and can use fxceless's measured "run 253" switch set
# (8-10 fps in the first rooms on their machine; never measured on this RTX 2060 yet).
# Logs go to E:\victory-runs\<timestamp>-next\ (NVMe). Saves and settings live in F:\victory-data\data.
param(
  [string]$Display = '1280x720',
  [switch]$Run253,                 # fxceless's run-253 switch set (see their README "Frame rate in the world")
  [switch]$Run279,                 # add-on to -Run253: fxceless run 279 extras (use with -Run253)
  [switch]$FG,                     # frame generation (DLSS-G via Streamline, E:\victory-build\sl-bin2)
  [switch]$Default,
  [switch]$NoHoles,
  [string]$Exe = 'E:\victory-build\target-next2\release\victory.exe'   # rtx2060-full c4ed5ad on fxceless 32b31f7 (2026-11-26 08:50)
)
$repo = 'F:\victory\merge-test'
$exe  = $Exe
$game = 'F:\GTA6-Xbox\game_scarlett_master.exe'
if (-not (Test-Path "$repo\victory.toml")) { Copy-Item 'F:\victory\fxceless\victory.toml' "$repo\victory.toml" }
$run  = "E:\victory-runs\$(Get-Date -Format yyyyMMdd-HHmmss)-next"
New-Item -ItemType Directory -Force $run | Out-Null

if (-not $Default) {
  $env:VICTORY_DISPLAY_MODE        = $Display
  $env:VICTORY_LEVELS_ON_DEMAND    = '1'
  $env:VICTORY_PIPELINE_WAIT_MS    = '24'
  $env:VICTORY_INDIRECT_WINDOW     = '1'
  if ($Run253) {
    $env:VICTORY_ASYNC_SUBMIT      = '1'
    $env:VICTORY_QUEUE2            = '1'
    $env:VICTORY_TEXTURE_NO_WAIT   = '1'
    $env:VICTORY_MERGE_GAP         = '64'
    $env:VICTORY_MERGE_RUNS        = '4096'
    $env:VICTORY_STAGING_POOL      = '0'
    $env:VICTORY_COPY_PLAIN        = '1'
    $env:VICTORY_JOIN_PARTS        = '1'
    $env:VICTORY_BACK_OWN_VIEW     = '1'
    $env:VICTORY_INDIRECT_GATHER   = '1'
    $env:VICTORY_CARD_REHASH_RUNS  = '1'
  }
  if ($Run279) {
    # fxceless run 279 (their best set, 2026-11-25, RTX 4070 12 GB): run 253 + card buffers/windows,
    # texture copy-back (water right, boats float: needed for the first boat mission), hash length.
    # RTX 2060 6 GB: theirs also sets CARD_BUFFERS_MIN_KB=64 + CARD_BUFFERS_MB=1280. With MIN_KB=64 and
    # BUFFERS_MB=640 here (run 20261126-072158) the small buffers filled the card budget and 340 MB/frame
    # of >=1 MiB buffers fell back to CPU compares (default run: 1 MB): 3-4 fps. So: buffer defaults
    # (from 1 MiB, 1024 MB), windows halved (theirs 2048).
    $env:VICTORY_CARD_WINDOWS_MB     = '1024'
    $env:VICTORY_TEXTURE_BACK        = '1'
    $env:VICTORY_CARD_HASH_LEN       = '1'
  }
  $env:VICTORY_RELAY               = '2'
}
if ($FG) {
  # DLSS-G through Streamline; bridge with engine version + project GUID (commit 376be95).
  # RTX 20: Streamline still refuses the adapter until the user adds MFG-Unlocker (their step).
  $env:VICTORY_DLSSG  = '4'
  $env:VICTORY_SL_DIR = 'E:\victory-build\sl-bin2'
  $env:VICTORY_SL_LOG = "$run\streamline.log"
}
if (-not $NoHoles) { $env:VICTORY_DSTORAGE_ZERO_HOLES = '1' }
$env:VICTORY_LOG = 'info'

Write-Host "Logs: $run"
Push-Location $repo
# Start-Process writes stderr/stdout to the files as they come (pwsh '2>' on a native exe left err.log empty, 2026-11-26)
$p = Start-Process $exe -ArgumentList 'run',$game,'--execute' -WorkingDirectory $repo -NoNewWindow -PassThru -RedirectStandardError "$run\err.log" -RedirectStandardOutput "$run\out.log"
$p.WaitForExit(); $LASTEXITCODE = $p.ExitCode
Pop-Location
Write-Host "Exited with $LASTEXITCODE. The game's own crash logs: F:\victory-data\data\7952987A\temp"
