# Runs AnyXboxX's VIctory (Neptune "VIctory Edit." 2026-11-25c base + AnyXboxX changes; ~14 fps median on an
# RTX 4070 SUPER per their README) with its tuned switch set, scaled down for an RTX 2060 6 GB.
# Source: F:\victory\anyxboxx\third_party\victory   Build: E:\victory-build\target-anyx
param(
  [string]$Display = '1280x720',   # resolution the game is told the TV has
  [switch]$Precompile,             # translate shaders/pipelines into the cache first (~5-10 min, once)
  [switch]$Quality,                # AnyXboxX's picture-first set (fresh window copies, smaller pools; slower)
  [int]$Frames = 4                 # DLSS-G frames generated per real frame, 0 = off
)
$env:RUSTUP_HOME = 'F:\rust\rustup'; $env:CARGO_HOME = 'F:\rust\cargo'; $env:Path = "F:\rust\cargo\bin;$env:Path"
$env:CARGO_TARGET_DIR = 'E:\victory-build\target-anyx'
$run = "E:\victory-runs\$(Get-Date -Format yyyyMMdd-HHmmss)-anyx"
New-Item -ItemType Directory -Force $run, 'E:\victory-data\cache-anyx' | Out-Null
$override = @{
  VICTORY_DATA_DIR          = 'F:\victory-data\data'       # same saves as the other builds
  VICTORY_CACHE_DIR         = 'E:\victory-data\cache-anyx' # own cache, on the SSD
  VICTORY_DISPLAY_MODE      = $Display
  # Sized for 12-16 GB cards in the set; halved for 6 GB (the set's own note: lower CARD_BUFFERS_MB first).
  VICTORY_CARD_WINDOWS_MB   = '512'
  VICTORY_CARD_BUFFERS_MB   = '512'
  VICTORY_TEXTURE_STAGE_MB  = '256'
  VICTORY_WIDE_THREADS      = '8'                          # i3-12100F: 4 cores / 8 threads
  VICTORY_DSTORAGE_ZERO_HOLES = '1'                        # harmless with intact files (ours, if present)
  VICTORY_LOG               = 'info'
  VICTORY_DLSSG             = "$Frames"                       # DLSS Frame Generation (0 = off); MFG-Unlocker is winmm.dll beside victory.exe
  VICTORY_SL_DIR            = 'E:\victory-build\sl-bin3'
  VICTORY_SL_LOG            = "$run\streamline.log"
}
$args2 = @{ GameDir = 'F:\GTA6-Xbox'; Override = $override }
if ($Precompile) { $args2.Precompile = $true }
if ($Quality) { $args2.Quality = $true }
Write-Host "Logs: $run"
# No PowerShell redirection here: a 2> on the game holds its log until exit. Start this script with
# Start-Process -RedirectStandardError to get the log as it is written.
& 'F:\victory\anyxboxx\tools\lancer_victory.ps1' @args2


