# ztrx launcher: runs the game for normal play (no scripted presses, no capture).
param(
    [Parameter(Mandatory = $true)][string]$Game,
    [string]$Cache = "$env:LOCALAPPDATA\ztrx\cache",
    [string]$Data = "$env:LOCALAPPDATA\ztrx\data",
    [string]$Exe
)
# Windows PowerShell 5.1 leaves $PSScriptRoot empty in parameter defaults.
if (-not $Exe) {
    $Exe = Join-Path $PSScriptRoot "..\target\release\victory.exe"
}

# Check what is needed before starting, so a wrong path is named plainly.
function Stop-Launch([string]$Message) {
    Write-Host "ztrx: $Message" -ForegroundColor Red
    exit 1
}
if (-not (Test-Path $Exe -PathType Leaf)) {
    Stop-Launch "victory.exe not found at $Exe. Build it first: cargo build --release -p victory"
}
if (-not (Test-Path $Game -PathType Leaf)) {
    Stop-Launch "the game's exe was not found at $Game. Pass -Game with the full path to the game's exe."
}
foreach ($dir in $Cache, $Data) {
    try {
        New-Item -ItemType Directory -Force $dir -ErrorAction Stop | Out-Null
    } catch {
        Stop-Launch "cannot create the folder $dir ($($_.Exception.Message))"
    }
}

foreach ($v in "VICTORY_KEYS", "VICTORY_CAPTURE", "VICTORY_CAPTURE_FRAMES", "VICTORY_DUMP_TARGETS") {
    Remove-Item "Env:$v" -ErrorAction SilentlyContinue
}
$env:VICTORY_CACHE_DIR = $Cache
$env:VICTORY_DATA_DIR = $Data
# victory.toml next to the repository (key map and paths), unless one was chosen.
$config = Join-Path $PSScriptRoot "..\victory.toml"
if (-not $env:VICTORY_CONFIG -and (Test-Path $config)) {
    $env:VICTORY_CONFIG = (Resolve-Path $config).Path
}
& $Exe run $Game --execute 2>&1 | Out-File "$env:TEMP\ztrx.log" -Encoding utf8
$code = $LASTEXITCODE
Write-Host "Exited with $code; log: $env:TEMP\ztrx.log"
exit $code
