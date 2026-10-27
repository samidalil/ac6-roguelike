# Live view of the roguelike DLL's money/mission events for play sessions.
# Follows %USERPROFILE%\ac6_roguelike_dll.log (recreated at every game launch) and prints only
# balance changes, taxes, game over, deaths/respawns, rewards and rerolls, with wall-clock time.
$log = Join-Path $env:USERPROFILE 'ac6_roguelike_dll.log'
$pattern = 'COAM:|TAX|GAME OVER|respawn|reroll|mission start|re-queue|cleared|defer 1->0'
$pos = 0
Write-Host "Watching $log  (Ctrl+C to stop)" -ForegroundColor DarkGray
while ($true) {
    if (Test-Path $log) {
        $len = (Get-Item $log).Length
        if ($len -lt $pos) { $pos = 0; Write-Host '--- game relaunched ---' -ForegroundColor DarkGray }
        if ($len -gt $pos) {
            $fs = [System.IO.File]::Open($log, 'Open', 'Read', 'ReadWrite,Delete')
            $fs.Seek($pos, 'Begin') | Out-Null
            $sr = New-Object System.IO.StreamReader($fs)
            $text = $sr.ReadToEnd(); $pos = $fs.Position
            $sr.Close(); $fs.Close()
            foreach ($line in $text -split "`r?`n") {
                if ($line -match $pattern) {
                    $msg = $line -replace '^\[\s*[\d.]+\]\s*', ''
                    $color = 'Gray'
                    if ($msg -match 'TAX|GAME OVER') { $color = 'Red' }
                    elseif ($msg -match 'COAM:') { $color = 'Yellow' }
                    elseif ($msg -match 'reroll|reward|re-queue') { $color = 'Cyan' }
                    Write-Host ((Get-Date -Format 'HH:mm:ss') + '  ' + $msg) -ForegroundColor $color
                }
            }
        }
    }
    Start-Sleep -Milliseconds 300
}
