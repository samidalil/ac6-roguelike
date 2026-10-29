# Build mod/menu/02_010_equipcap.gfx (the inventory cap's sell list) from the game's own 02_010_equiptop.gfx:
# extract (ac6_unpack.py) -> FFDec swf2xml -> build_equipcap.py (no 3D AC view, no AC specs slot) -> FFDec xml2swf
# -> import mod/src/equipcap_MainTimeline.as (opaque backdrop)
param([string]$FFDec = "D:\samee\Workspace\Modding\PC\Tools\ffdec\ffdec-cli.jar",
      [string]$Java = "C:\Program Files (x86)\Common Files\Oracle\Java\java8path\java.exe")
$ErrorActionPreference = "Continue"
$root = Split-Path $PSScriptRoot -Parent
$work = Join-Path $env:TEMP "ac6_equipcap_build"
New-Item -ItemType Directory -Force $work | Out-Null
$vanilla = Join-Path $work "menu\02_010_equiptop.gfx"
if (-not (Test-Path $vanilla)) { python "$PSScriptRoot\ac6_unpack.py" /menu/02_010_equiptop.gfx -o $work | Out-Host }
& $Java -jar $FFDec -swf2xml $vanilla "$work\vanilla.xml" | Out-Null
python "$PSScriptRoot\build_equipcap.py" "$work\vanilla.xml" "$work\cap.xml" | Out-Host
if ($LASTEXITCODE -ne 0) { exit 1 }
& $Java -jar $FFDec -xml2swf "$work\cap.xml" "$work\cap_noscript.gfx" | Out-Null
$scripts = Join-Path $work "scripts\scripts\_02_010_EquipTop_fla"
New-Item -ItemType Directory -Force $scripts | Out-Null
Copy-Item "$root\mod\src\equipcap_MainTimeline.as" "$scripts\MainTimeline.as" -Force
$out = "$root\mod\menu\02_010_equipcap.gfx"
$log = cmd /c "`"$Java`" -jar `"$FFDec`" -importScript `"$work\cap_noscript.gfx`" `"$out`" `"$work\scripts`" 2>&1" | Out-String
if ($log -match 'SEVERE|Exception') { $log; exit 1 }
"built $out ($((Get-Item $out).Length) bytes)"
