# Build the inventory cap's sell screen movies (mod/menu) from the game's own:
# - 02_010_equipcap.gfx from 02_010_equiptop.gfx: extract (ac6_unpack.py) -> FFDec swf2xml -> build_equipcap.py (no 3D
#   AC view, no AC specs slot) -> build_capmovies.py (centred) -> FFDec xml2swf -> import mod/src/equipcap_MainTimeline.as
#   (dialog panel)
# - 02_eqcap.gfx (the list) and 02_gaitemstatus_capview_*.gfx (the part info and stats) from 02_equip.gfx and
#   02_gaitemstatus_general_*.gfx: swf2xml -> build_capmovies.py (centred) -> xml2swf
param([string]$FFDec = "D:\samee\Workspace\Modding\PC\Tools\ffdec\ffdec-cli.jar",
      [string]$Java = "C:\Program Files (x86)\Common Files\Oracle\Java\java8path\java.exe")
$ErrorActionPreference = "Continue"
$root = Split-Path $PSScriptRoot -Parent
$work = Join-Path $env:TEMP "ac6_equipcap_build"
$dx = 320  # the content's vanilla x 60..1220 centred on the 1920 stage
New-Item -ItemType Directory -Force $work | Out-Null
$copies = [ordered]@{ "02_010_equiptop" = "02_010_equipcap"; "02_equip" = "02_eqcap" }
foreach ($v in "normal", "detailcomparison", "modelviewer") { $copies["02_gaitemstatus_general_$v"] = "02_gaitemstatus_capview_$v" }
$missing = @($copies.Keys | Where-Object { -not (Test-Path (Join-Path $work "menu\$_.gfx")) } | ForEach-Object { "/menu/$_.gfx" })
if ($missing) { python "$PSScriptRoot\ac6_unpack.py" @missing -o $work | Out-Host }
foreach ($v in $copies.Keys) {
    $n = $copies[$v]
    & $Java -jar $FFDec -swf2xml (Join-Path $work "menu\$v.gfx") "$work\$v.xml" | Out-Null
    $xml = "$work\$v.xml"
    if ($v -eq "02_010_equiptop") {
        python "$PSScriptRoot\build_equipcap.py" $xml "$work\cap.xml" | Out-Host
        if ($LASTEXITCODE -ne 0) { exit 1 }
        $xml = "$work\cap.xml"
    }
    # kept at their vanilla place: the credits box (AdditionalProperties) top right, the key guide (StatusBar) bottom left
    $kept = @()
    if ($v -eq "02_010_equiptop") { $kept = @("AdditionalProperties", "StatusBar") }
    if ($v -eq "02_equip") { $kept = @("AdditionalProperties") }
    python "$PSScriptRoot\build_capmovies.py" $xml "$work\$n.xml" $dx @kept | Out-Host
    if ($LASTEXITCODE -ne 0) { exit 1 }
    $out = "$root\mod\menu\$n.gfx"
    if ($v -eq "02_010_equiptop") {
        & $Java -jar $FFDec -xml2swf "$work\$n.xml" "$work\cap_noscript.gfx" | Out-Null
        $scripts = Join-Path $work "scripts\scripts\_02_010_EquipTop_fla"
        New-Item -ItemType Directory -Force $scripts | Out-Null
        Copy-Item "$root\mod\src\equipcap_MainTimeline.as" "$scripts\MainTimeline.as" -Force
        $log = cmd /c "`"$Java`" -jar `"$FFDec`" -importScript `"$work\cap_noscript.gfx`" `"$out`" `"$work\scripts`" 2>&1" | Out-String
        if ($log -match 'SEVERE|Exception') { $log; exit 1 }
    } else {
        & $Java -jar $FFDec -xml2swf "$work\$n.xml" $out | Out-Null
    }
    "built $out ($((Get-Item $out).Length) bytes)"
}
