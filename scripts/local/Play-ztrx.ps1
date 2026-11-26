# Fallback launcher: ztrx exactly as published (fxceless at 2119b0e, tested on an RTX 3050 through the prologue).
# Default VIctory settings, like ztrx's own scripts\play.ps1, with its own cache so it never shares files with the other builds.
# Main launcher with all fixes and the RTX 2060 tuning: F:\victory-data\Play-GTA6.ps1
param([string]$Display = '')   # e.g. -Display 1280x720 (ztrx has the fxceless display-mode switch)
$repo = 'F:\victory\ztrx'
$exe  = 'F:\victory\target-ztrx\release\victory.exe'
$game = 'F:\GTA6-Xbox\game_scarlett_master.exe'
foreach ($v in 'VICTORY_KEYS','VICTORY_CAPTURE','VICTORY_CAPTURE_FRAMES','VICTORY_DUMP_TARGETS','VICTORY_HAIR','VICTORY_SKIP_PIPELINES',
               'VICTORY_LEVELS_ON_DEMAND','VICTORY_RELAY','VICTORY_PIPELINE_WAIT_MS','VICTORY_INDIRECT_WINDOW','VICTORY_DSTORAGE_ZERO_HOLES','VICTORY_DISPLAY_MODE') {
  Remove-Item "Env:$v" -ErrorAction SilentlyContinue
}
if ($Display) { $env:VICTORY_DISPLAY_MODE = $Display }
$env:VICTORY_CACHE_DIR = 'F:\victory-data\cache-ztrx'
$env:VICTORY_DATA_DIR  = 'F:\victory-data\data'
$env:VICTORY_LOG = 'info'
$run = "F:\victory-data\runs\ztrx-$(Get-Date -Format yyyyMMdd-HHmmss)"
New-Item -ItemType Directory -Force $run, $env:VICTORY_CACHE_DIR | Out-Null
Write-Host "Logs: $run"
Push-Location $repo
& $exe run $game --execute 2> "$run\err.log" > "$run\out.log"
Pop-Location
Write-Host "Exited with $LASTEXITCODE. The game's own crash logs: F:\victory-data\data\7952987A\temp"
