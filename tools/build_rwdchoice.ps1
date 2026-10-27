# Build mod/menu/02_rwdchoice.gfx (reward choice grid) from the game's own 02_gotreward.gfx:
# extract (ac6_unpack.py) -> FFDec swf2xml -> build_rwdchoice.py -> FFDec xml2swf -> import mod/src/rwdchoice_MainTimeline.as
param([string]$FFDec = "D:\samee\Workspace\Modding\PC\Tools\ffdec\ffdec-cli.jar",
      [string]$Java = "C:\Program Files (x86)\Common Files\Oracle\Java\java8path\java.exe")
$ErrorActionPreference = "Continue"
$root = Split-Path $PSScriptRoot -Parent
$work = Join-Path $env:TEMP "ac6_rwdchoice_build"
New-Item -ItemType Directory -Force $work | Out-Null
$vanilla = Join-Path $work "menu\02_gotreward.gfx"
if (-not (Test-Path $vanilla)) { python "$PSScriptRoot\ac6_unpack.py" /menu/02_gotreward.gfx -o $work | Out-Host }
& $Java -jar $FFDec -swf2xml $vanilla "$work\vanilla.xml" | Out-Null
python "$PSScriptRoot\build_rwdchoice.py" "$work\vanilla.xml" "$work\choice.xml" | Out-Host
& $Java -jar $FFDec -xml2swf "$work\choice.xml" "$work\choice_noscript.gfx" | Out-Null
$scripts = Join-Path $work "scripts\scripts\_02_GotReward_fla"
New-Item -ItemType Directory -Force $scripts | Out-Null
Copy-Item "$root\mod\src\rwdchoice_MainTimeline.as" "$scripts\MainTimeline.as" -Force
$out = "$root\mod\menu\02_rwdchoice.gfx"
$log = cmd /c "`"$Java`" -jar `"$FFDec`" -importScript `"$work\choice_noscript.gfx`" `"$out`" `"$work\scripts`" 2>&1" | Out-String
if ($log -match 'SEVERE|Exception') { $log; exit 1 }
"built $out ($((Get-Item $out).Length) bytes)"
