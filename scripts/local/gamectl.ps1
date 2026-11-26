param([string]$Keys = "", [string]$Shot = "", [int]$HoldMs = 150, [int]$WaitMs = 1500)
# Focus the VIctory window, optionally press keys (comma list of virtual-key names), optionally save a screenshot.
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public class VicCtl {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte sc, uint f, UIntPtr e);
  [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint c, uint t);
  public struct RECT { public int L, T, R, B; }
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  // The game's window: the largest visible window of process `pid` whose title is not an overlay's
  // (Neptune-based builds also open a "NeptuneOverlay" window).
  public static IntPtr GameWindow(uint pid) {
    IntPtr best = IntPtr.Zero; long area = 0;
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid || !IsWindowVisible(h)) return true;
      var t = new System.Text.StringBuilder(256); GetWindowText(h, t, 256);
      if (t.ToString().Contains("Overlay")) return true;
      RECT r; GetClientRect(h, out r); long a = (long)r.R * r.B;
      if (a > area) { area = a; best = h; }
      return true;
    }, IntPtr.Zero);
    return best;
  }
  public static string Title(IntPtr h) { var t = new System.Text.StringBuilder(256); GetWindowText(h, t, 256); return t.ToString(); }
}
'@
$p = Get-Process victory -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $p) { "NO GAME WINDOW"; exit 1 }
$h = [VicCtl]::GameWindow([uint32]$p.Id)
if ($h -eq [IntPtr]::Zero) { "NO GAME WINDOW"; exit 1 }
[VicCtl]::ShowWindow($h, 9) | Out-Null
# An Alt tap first: without it Windows' foreground lock keeps the window in the background
# when this runs from a background shell, and the keys go elsewhere (2026-11-26).
[VicCtl]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [VicCtl]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
[VicCtl]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 400
$vk = @{ Enter=0x0D; Backspace=0x08; Esc=0x1B; Tab=0x09; Up=0x26; Down=0x28; Left=0x25; Right=0x27; X=0x58; Y=0x59; W=0x57; A=0x41; S=0x53; D=0x44; Q=0x51; E=0x45; Z=0x5A; C=0x43; F=0x46; H=0x48 }
foreach ($k in ($Keys -split ',' | Where-Object { $_ })) {
  if ($k -like 'wait*') { Start-Sleep -Milliseconds ([int]$k.Substring(4)); continue }
  $code = [byte]$vk[$k]; $sc = [byte][VicCtl]::MapVirtualKey($code, 0)
  [VicCtl]::keybd_event($code, $sc, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds $HoldMs
  [VicCtl]::keybd_event($code, $sc, 2, [UIntPtr]::Zero); Start-Sleep -Milliseconds 300
}
if ($Shot) {
  Start-Sleep -Milliseconds $WaitMs
  $r = New-Object VicCtl+RECT; [VicCtl]::GetClientRect($h, [ref]$r) | Out-Null
  $bmp = New-Object Drawing.Bitmap ([Math]::Max(1,$r.R)), ([Math]::Max(1,$r.B))
  $g = [Drawing.Graphics]::FromImage($bmp); $dc = $g.GetHdc()
  [VicCtl]::PrintWindow($h, $dc, 3) | Out-Null   # PW_CLIENTONLY | PW_RENDERFULLCONTENT
  $g.ReleaseHdc($dc); $bmp.Save($Shot)
}
[VicCtl]::Title($h)
