# Build mod/regulation.bin from data/regulation/*.toml (tools/regulation, Smithbox's SoulsFormats): the game's own
# regulation with our declared param edits and new rows, served by me3 from the mod package. Without declarations no
# file is written and an old mod/regulation.bin is removed (the game then uses its own).
# Usage: powershell -File tools\build_regulation.ps1 [-SmithboxDir <Smithbox folder>]
param([string]$SmithboxDir = $(if ($env:SMITHBOX_DIR) { $env:SMITHBOX_DIR } else { "D:\samee\Workspace\Modding\PC\From Software\Smithbox_2_2_5_2026_08_29_a" }))
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$game = "D:\Program Files\Steam\Library\steamapps\common\ARMORED CORE VI FIRES OF RUBICON\Game\regulation.bin"
$out = Join-Path $root "mod\regulation.bin"
$data = Join-Path $root "data\regulation"
$tool = Join-Path $root "dll\target\regbuild"
$env:SMITHBOX_DIR = $SmithboxDir
dotnet build (Join-Path $PSScriptRoot "regulation\RegBuild.csproj") -nologo -v q -o $tool | Out-Null
if ($LASTEXITCODE -ne 0) { throw "RegBuild build failed" }
if (Test-Path $out) { Remove-Item $out }
dotnet (Join-Path $tool "RegBuild.dll") $game $data $out (Join-Path $SmithboxDir "Assets\PARAM\AC6\Defs")
if ($LASTEXITCODE -ne 0) { throw "regulation build failed" }
