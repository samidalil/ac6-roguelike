# Give the game window keyboard/mouse focus by clicking inside it with a real (SendInput-level) click.
# Windows refuses SetForegroundWindow while another window (e.g. the text input host) holds the foreground;
# a click on the game window always works. Default spot: lower middle of a 1920x1080 window (empty in the garage).
param([int]$x = 960, [int]$y = 810)
Add-Type @"
using System; using System.Runtime.InteropServices;
public class FocusGame { [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow(); }
"@
$p = Get-Process armoredcore6 -ErrorAction SilentlyContinue
if (-not $p) { "game not running"; exit 1 }
[FocusGame]::SetCursorPos($x, $y) | Out-Null
[FocusGame]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 300
[FocusGame]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 300
"game focused: " + ([FocusGame]::GetForegroundWindow() -eq $p.MainWindowHandle)
