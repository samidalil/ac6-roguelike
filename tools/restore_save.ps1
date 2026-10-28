# Put a test save from saves/ in place of the dev save (AC6_roguelike_fresh3.sl2). Refuses while the game runs; the
# current save is copied to %APPDATA%\ArmoredCore6\backups\keep first.
# Usage: powershell -File tools\restore_save.ps1 [-Name ac-mission-available]
#   ac-mission-available = garage, chapter 1, "Destruction de l'AC d'essai" available (instant clear: RE_NOTES
#   "Test routine: instant mission clear")
param([string]$Name = "ac-mission-available", [string]$Save = "AC6_roguelike_fresh3.sl2")
$root = Split-Path $PSScriptRoot -Parent
$src = Join-Path $root "saves\$Name.sl2"
if (-not (Test-Path $src)) { "no save $src"; exit 1 }
if (Get-Process armoredcore6 -ErrorAction SilentlyContinue) { "the game is running: close it first (never mid-mission)"; exit 1 }
$dir = "$env:APPDATA\ArmoredCore6\76561198128476385"
$keep = "$env:APPDATA\ArmoredCore6\backups\keep"
New-Item -ItemType Directory -Force $keep | Out-Null
$dest = Join-Path $dir $Save
if (Test-Path $dest) { Copy-Item $dest (Join-Path $keep ("before-restore_" + (Get-Date -Format "yyyy-MM-dd_HH-mm-ss") + ".sl2")) }
Copy-Item $src $dest -Force
"restored $Name -> $dest"
