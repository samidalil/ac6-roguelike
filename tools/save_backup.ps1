# Back up the dev save every time the game writes it (FileSystemWatcher, no polling).
# Copies land in %APPDATA%\ArmoredCore6\backups\<save name>\<timestamp>.sl2; the newest $Keep are kept.
# Usage: powershell -File tools\save_backup.ps1 [-Save AC6_roguelike_fresh3.sl2] [-Keep 30]
param([string]$Save = "AC6_roguelike_fresh3.sl2", [int]$Keep = 30)
$dir = "$env:APPDATA\ArmoredCore6\76561198128476385"
$dest = "$env:APPDATA\ArmoredCore6\backups\$([IO.Path]::GetFileNameWithoutExtension($Save))"
New-Item -ItemType Directory -Force $dest | Out-Null
$w = New-Object IO.FileSystemWatcher $dir, $Save
$w.NotifyFilter = [IO.NotifyFilters]'LastWrite, FileName, Size'
"watching $dir\$Save -> $dest (keep $Keep)"
while ($true) {
    $r = $w.WaitForChanged([IO.WatcherChangeTypes]'Changed, Created, Renamed')
    if ($r.TimedOut) { continue }
    Start-Sleep -Seconds 3   # the game writes in several chunks: copy once it is done
    $src = Join-Path $dir $Save
    if (-not (Test-Path $src)) { continue }
    $name = (Get-Date -Format "yyyy-MM-dd_HH-mm-ss") + ".sl2"
    try { Copy-Item $src (Join-Path $dest $name) -ErrorAction Stop; "backup $name" } catch { "copy failed: $_" }
    Get-ChildItem $dest -Filter *.sl2 | Sort-Object LastWriteTime -Descending | Select-Object -Skip $Keep | Remove-Item -Force
}
