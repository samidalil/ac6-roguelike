# Dev cycle: kill the game, rebuild the DLL, launch the dev profile, wait for the title screen, focus the window.
# Usage: powershell -File tools\relaunch.ps1 [-NoBuild] [-Profile ac6-roguelike-fresh3.me3]
param([switch]$NoBuild, [string]$Profile = "ac6-roguelike-fresh3.me3")
$root = Split-Path $PSScriptRoot -Parent
Stop-Process -Name armoredcore6,me3,me3-launcher -Force -ErrorAction SilentlyContinue
Start-Sleep 3
if (-not $NoBuild) {
    Push-Location "$root\dll"
    $out = cargo build --release 2>&1 | Out-String
    Pop-Location
    if ($out -match '(?m)^error') { $out; exit 1 }
    ($out -split "`n" | Select-String 'Finished').Line
}
Start-Process -WindowStyle Hidden me3 -ArgumentList 'launch','--auto-detect','-p',"$root\$Profile"
$t = 0; while (-not (Get-Process armoredcore6 -ErrorAction SilentlyContinue) -and $t -lt 60) { Start-Sleep 2; $t += 2 }
Start-Sleep 30
& "$PSScriptRoot\focus_game.ps1" -y 1000
