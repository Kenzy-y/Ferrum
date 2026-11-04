# Manual regression check for the custom title bar on Windows.
#
# Asks the running IDE window what the platform hit test answers at the
# window controls and at empty title bar spots, without moving the real
# mouse: it posts WM_MOUSEMOVE to the window, then sends WM_NCHITTEST.
# Expected: close 20 (HTCLOSE), maximize 9 (HTMAXBUTTON), minimize 8
# (HTMINBUTTON), empty spots 2 (HTCAPTION), search field 1 (HTCLIENT).
# Coordinates assume the default 1280x800 window at 100% scale.
#
# Usage: powershell -File scripts/windows/title-bar-hit-test.ps1 -ProcessId <pid>
param([int]$ProcessId)
Add-Type @"
using System; using System.Runtime.InteropServices;
public class H {
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct R { public int L, T, Rr, B; }
  public struct P { public int X, Y; }
}
"@
[H]::SetProcessDPIAware() | Out-Null
$h = (Get-Process -Id $ProcessId).MainWindowHandle
$r = New-Object H+R; [H]::GetClientRect($h, [ref]$r) | Out-Null
$w = $r.Rr
function Probe([string]$name, [int]$x, [int]$y) {
  $lp = [IntPtr](($y -shl 16) -bor ($x -band 0xFFFF))
  [H]::PostMessage($h, 0x0200, [IntPtr]::Zero, $lp) | Out-Null   # WM_MOUSEMOVE (client coords)
  Start-Sleep -Milliseconds 400
  $p = New-Object H+P; $p.X = $x; $p.Y = $y
  [H]::ClientToScreen($h, [ref]$p) | Out-Null
  $slp = [IntPtr](($p.Y -shl 16) -bor ($p.X -band 0xFFFF))
  $res = [H]::SendMessage($h, 0x0084, [IntPtr]::Zero, $slp)      # WM_NCHITTEST (screen coords)
  Write-Output ("{0,-12} ({1},{2}) -> {3}" -f $name, $x, $y, $res)
}
Probe "close" ($w - 23) 32
Probe "maximize" ($w - 69) 32
Probe "minimize" ($w - 115) 32
Probe "gap-left" 435 32
Probe "gap-right" 925 32
Probe "wordmark" 100 32
Probe "search" ([int]($w / 2)) 32
