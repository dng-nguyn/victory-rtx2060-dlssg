# Launches GTA VI (Xbox Series dump) through VIctory, tuned for: RTX 2060 6 GB, i3-12100F, 32 GB RAM.
# Build: branch rtx2060-full in F:\victory\merge-test (fxceless main + picked fixes), output in F:\victory\target-full.
# Logs go to F:\victory-data\runs\<timestamp>\. Saves and the game's settings live in F:\victory-data\data.
param(
  [string]$Display = '1280x720',   # resolution the game is told the TV has (default 3840x2160 is too heavy for 6 GB)
  [switch]$Default,                # VIctory's defaults only (for comparison)
  [switch]$NoHoles                 # do NOT pass all-zero blocks of an incomplete dump (the game then stops at them)
)
$repo = 'F:\victory\merge-test'
$exe  = 'F:\victory\target-full2\release\victory.exe'
$game = 'F:\GTA6-Xbox\game_scarlett_master.exe'
if (-not (Test-Path "$repo\victory.toml")) { Copy-Item 'F:\victory\fxceless\victory.toml' "$repo\victory.toml" }
$run  = "F:\victory-data\runs\$(Get-Date -Format yyyyMMdd-HHmmss)"
New-Item -ItemType Directory -Force $run | Out-Null

if (-not $Default) {
  $env:VICTORY_DISPLAY_MODE        = $Display
  $env:VICTORY_LEVELS_ON_DEMAND    = '1'     # only the texture mips the game holds take VRAM (~2-3 GB saved)
  $env:VICTORY_RELAY               = '2'     # submit ahead of the GPU wait, relay thread lands results
  $env:VICTORY_PIPELINE_WAIT_MS    = '24'    # don't stall a frame up to 500 ms for background pipeline builds
  $env:VICTORY_INDIRECT_WINDOW     = '1'     # cheaper indirect draws (fxceless: 6.0 -> 8.9 fps); the driver watch turns them off before a hang
}
if (-not $NoHoles) { $env:VICTORY_DSTORAGE_ZERO_HOLES = '1' }  # anim.rpf in this dump is ~20% zero-filled
$env:VICTORY_LOG = 'info'

Write-Host "Logs: $run"
Push-Location $repo
& $exe run $game --execute 2> "$run\err.log" > "$run\out.log"
Pop-Location
Write-Host "Exited with $LASTEXITCODE. The game's own crash logs: F:\victory-data\data\7952987A\temp"
