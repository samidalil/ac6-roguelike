# Build the release DLL and assemble dist\AC6-Roguelike (DLL, me3 profile with its own save file, README) + a zip.
# Usage: powershell -File tools\package.ps1   (close the game first: it locks the DLL)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location (Join-Path $root "dll")
cargo build --release
if ($LASTEXITCODE -ne 0) { Pop-Location; throw "cargo build failed" }
Pop-Location

$out = Join-Path $root "dist\AC6-Roguelike"
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item (Join-Path $root "dll\target\release\ac6_roguelike.dll") $out
Copy-Item (Join-Path $root "dist_template\README.txt") $out
@'
profileVersion = "v1"
disable_arxan = true
savefile = "AC6_roguelike.sl2"   # separate save: the normal one is untouched

[[supports]]
game = "armoredcore6"

[[natives]]
path = 'ac6_roguelike.dll'
'@ | ForEach-Object { [IO.File]::WriteAllText((Join-Path $out "ac6-roguelike.me3"), $_, (New-Object Text.UTF8Encoding $false)) }

$zip = Join-Path $root "dist\AC6-Roguelike.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $out -DestinationPath $zip
Get-ChildItem $out | Select-Object Name, Length
"zip: $zip"
