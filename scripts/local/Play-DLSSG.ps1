# DLSS Frame Generation test: the DLSS-G build (depth/motion-vector scout + Streamline) with MFG-Unlocker
# (E:\victory-build\target-dlssg\release\winmm.dll) loaded into victory.exe.
# Press Backspace in game to open MFG-Unlocker's menu. Logs: E:\victory-runs\<time>-dlssg\
param([int]$Frames = 4,   # frames generated per real frame (DLSS-G: 1-5)
      [ValidateSet('series-x','series-s')][string]$Console = 'series-x')   # series-s: the title renders at a lower resolution
Get-Process victory -ErrorAction SilentlyContinue | ForEach-Object { Write-Host "Closing running victory.exe ($($_.Id))"; $_.CloseMainWindow() | Out-Null; Start-Sleep 5; if (!$_.HasExited) { $_.Kill() } }
$run = "E:\victory-runs\$(Get-Date -Format yyyyMMdd-HHmmss)-dlssg"
New-Item -ItemType Directory -Force $run | Out-Null
foreach ($v in 'VICTORY_CACHE_DIR','VICTORY_DATA_DIR','VICTORY_HAIR','VICTORY_SKIP_PIPELINES','VICTORY_WALLS_TRACE') { Remove-Item "Env:$v" -ErrorAction SilentlyContinue }
$env:VICTORY_DISPLAY_MODE = '1280x720'; $env:VICTORY_LEVELS_ON_DEMAND = '1'; $env:VICTORY_RELAY = '2'
$env:VICTORY_PIPELINE_WAIT_MS = '24'; $env:VICTORY_INDIRECT_WINDOW = '1'; $env:VICTORY_DSTORAGE_ZERO_HOLES = '1'; $env:VICTORY_LOG = 'info'
$env:VICTORY_DLSSG = "$Frames"; $env:VICTORY_SL_DIR = 'E:\victory-build\sl-bin3'; $env:VICTORY_SL_LOG = "$run\streamline.log"
$p = Start-Process 'E:\victory-build\target-dlssg\release\victory.exe' -ArgumentList 'run','F:\GTA6-Xbox\game_scarlett_master.exe','--execute','--console',$Console `
  -WorkingDirectory 'F:\victory\merge-test' -RedirectStandardOutput "$run\out.log" -RedirectStandardError "$run\err.log" -PassThru
Write-Host "Started victory.exe pid $($p.Id) with MFG-Unlocker. Logs: $run"

