# Real Windows mouse input for the native smoke tests: move, glide, click, right click and drag,
# in screen pixels, after bringing the Nocta window of process -Window to the front.
param(
  [string]$Action,
  [int]$X1, [int]$Y1,
  [int]$X2 = 0, [int]$Y2 = 0,
  [string]$Window = ""
)
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class M {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  public delegate bool P(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(P f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  public static IntPtr FindNocta(uint target) { IntPtr found = IntPtr.Zero; EnumWindows((h,l)=>{ uint p; GetWindowThreadProcessId(h,out p); var s=new System.Text.StringBuilder(64); GetWindowText(h,s,64); if(p==target && s.ToString()=="Nocta"){ found=h; return false; } return true; }, IntPtr.Zero); return found; }
}
'@
[M]::SetProcessDPIAware() | Out-Null
if ($Window) {
  $h = [M]::FindNocta([uint32]$Window)
  if ([M]::IsIconic($h)) { [M]::ShowWindow($h, 9) | Out-Null; Start-Sleep -Milliseconds 400 }
  [M]::SetForegroundWindow($h) | Out-Null
  Start-Sleep -Milliseconds 150
}
$LEFTDOWN = 0x2; $LEFTUP = 0x4; $RIGHTDOWN = 0x8; $RIGHTUP = 0x10
switch ($Action) {
  "move" { [M]::SetCursorPos($X1, $Y1) | Out-Null }
  "glide" {
    for ($i = 1; $i -le 12; $i++) {
      [M]::SetCursorPos([int]($X1 + ($X2 - $X1) * $i / 12), [int]($Y1 + ($Y2 - $Y1) * $i / 12)) | Out-Null
      Start-Sleep -Milliseconds 25
    }
  }
  "right" {
    [M]::SetCursorPos($X1, $Y1) | Out-Null; Start-Sleep -Milliseconds 80
    [M]::mouse_event($RIGHTDOWN, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 40
    [M]::mouse_event($RIGHTUP, 0, 0, 0, [IntPtr]::Zero)
  }
  "click" {
    [M]::SetCursorPos($X1, $Y1) | Out-Null; Start-Sleep -Milliseconds 80
    [M]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 40
    [M]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
  }
  "drag" {
    [M]::SetCursorPos($X1, $Y1) | Out-Null; Start-Sleep -Milliseconds 120
    [M]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 150
    for ($i = 1; $i -le 20; $i++) {
      [M]::SetCursorPos([int]($X1 + ($X2 - $X1) * $i / 20), [int]($Y1 + ($Y2 - $Y1) * $i / 20)) | Out-Null
      Start-Sleep -Milliseconds 30
    }
    Start-Sleep -Milliseconds 250
    [M]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
  }
}
