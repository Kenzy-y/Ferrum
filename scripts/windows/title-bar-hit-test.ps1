# Regression check for the custom title bar on Windows.
#
# Asks the IDE window what the platform hit test answers at the window
# controls, at empty title bar spots and at the search field, without
# moving the real mouse: it posts WM_MOUSEMOVE to the window, then sends
# WM_NCHITTEST. Expected: close 20 (HTCLOSE), maximize 9 (HTMAXBUTTON),
# minimize 8 (HTMINBUTTON), empty spots 2 (HTCAPTION), search field 1
# (HTCLIENT). Coordinates assume the default 1280x800 window at 100% scale.
#
# Usage:
#   -ProcessId <pid>   probe a running IDE and print the answers;
#   -Exe <path>        start that binary, probe, stop it (by its own PID) and
#                      exit 1 if any answer differs (used by CI).
param([int]$ProcessId, [string]$Exe, [int]$StartupSeconds = 20)
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

$started = $null
if ($Exe) {
  $started = Start-Process -FilePath $Exe -PassThru
  $ProcessId = $started.Id
}

# Wait until the window exists and has been drawn once.
$h = [IntPtr]::Zero
$deadline = (Get-Date).AddSeconds($StartupSeconds)
while ((Get-Date) -lt $deadline) {
  $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
  if ($null -eq $process) { break }
  $h = $process.MainWindowHandle
  if ($h -ne [IntPtr]::Zero) { break }
  Start-Sleep -Milliseconds 250
}
if ($h -eq [IntPtr]::Zero) {
  Write-Output "no window appeared within $StartupSeconds s"
  if ($started) { Stop-Process -Id $started.Id -Force -ErrorAction SilentlyContinue }
  exit 1
}
Start-Sleep -Seconds 2

$r = New-Object H+R; [H]::GetClientRect($h, [ref]$r) | Out-Null
$w = $r.Rr
$failures = 0
function Probe([string]$name, [int]$x, [int]$y, [int]$expected) {
  $lp = [IntPtr](($y -shl 16) -bor ($x -band 0xFFFF))
  [H]::PostMessage($h, 0x0200, [IntPtr]::Zero, $lp) | Out-Null   # WM_MOUSEMOVE (client coords)
  Start-Sleep -Milliseconds 400
  $p = New-Object H+P; $p.X = $x; $p.Y = $y
  [H]::ClientToScreen($h, [ref]$p) | Out-Null
  $slp = [IntPtr](($p.Y -shl 16) -bor ($p.X -band 0xFFFF))
  $res = [int][H]::SendMessage($h, 0x0084, [IntPtr]::Zero, $slp)  # WM_NCHITTEST (screen coords)
  $mark = if ($res -eq $expected) { "ok" } else { $script:failures++; "EXPECTED $expected" }
  Write-Output ("{0,-12} ({1},{2}) -> {3} {4}" -f $name, $x, $y, $res, $mark)
}
Probe "close" ($w - 23) 32 20
Probe "maximize" ($w - 69) 32 9
Probe "minimize" ($w - 115) 32 8
Probe "gap-left" 435 32 2
Probe "gap-right" 925 32 2
Probe "wordmark" 100 32 2
Probe "search" ([int]($w / 2)) 32 1

if ($started) { Stop-Process -Id $started.Id -Force -ErrorAction SilentlyContinue }
if ($failures -gt 0) {
  Write-Output "$failures title bar spots answer wrong"
  if ($Exe) { exit 1 }
}
