# Compares how Vectorworks and the open-bim-suite viewer show the same files: for each file a
# copy is opened in a running Vectorworks (never the original, never saved), its window is
# captured without taking the foreground, and the viewer renders the same copy headlessly.
#
#   powershell -File compare.ps1 -Viewer <open-3d-viewer.exe> -Out <folder> <file> [<file> ...]
#
# Writes <Out>\NN_<name>_vw.png and <Out>\NN_<name>_app.png, and index.txt with one line per
# file (what opened, load time, the window title). Vectorworks is restarted after -Batch files,
# since documents opened this way stay open. The oracle server must not be running (its script
# keeps Vectorworks from drawing).
[CmdletBinding(PositionalBinding = $false)]
param(
    [Parameter(Mandatory)] [string]$Viewer,
    [Parameter(Mandatory)] [string]$Out,
    [int]$Batch = 6,
    [string]$Vectorworks = 'C:\Program Files\Vectorworks 2027 NL\Vectorworks2027.exe',
    [Parameter(Position = 0, ValueFromRemainingArguments)] [string[]]$Files
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Cap {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr h, int c);
}
'@
Add-Type -Path (Join-Path $PSScriptRoot '..\oracle\Msaa.cs') -ReferencedAssemblies Accessibility
New-Item -ItemType Directory -Force $Out | Out-Null
$copies = Join-Path $Out 'copies'
New-Item -ItemType Directory -Force $copies | Out-Null
$index = Join-Path $Out 'index.txt'

function Main-Window {
    Get-Process | Where-Object { $_.ProcessName -like 'Vectorworks20*' -and $_.MainWindowHandle -ne 0 } | Select-Object -First 1
}

# The documents open in Vectorworks: the frames of its views (a document is not always titled
# by the name of the file it was opened from, and a template opens as a new untitled one).
function Documents {
    $w = Main-Window
    if (-not $w) { return @() }
    @([Msaa]::ChildWindows([long]$w.MainWindowHandle) | Where-Object { $_ -match 'AfxFrameOrView[^|]*\|\S' })
}

# Presses OK on the message boxes Vectorworks shows (that it is a student version, that a file
# comes from another version): no document is closed here, so none asks to be saved.
function Dismiss {
    $w = Main-Window
    if (-not $w) { return }
    foreach ($d in [Msaa]::Windows($w.Id)) {
        $p = $d.Split('|')
        if ($p[1] -eq '#32770' -and ([Msaa]::Items([long]$p[0]) -join ' ') -match '\S') { [void][Msaa]::Press([long]$p[0], 1) }
    }
}

function Start-Vectorworks {
    if (Main-Window) { return }
    Start-Process $Vectorworks
    for ($t = 0; $t -lt 240; $t += 5) {
        Start-Sleep 5
        $w = Main-Window
        if ($w -and $w.MainWindowTitle -like 'Vectorworks*') {
            for ($k = 0; $k -lt 6; $k++) { Start-Sleep 3; Dismiss }
            return
        }
    }
    throw 'Vectorworks did not start'
}

function Stop-Vectorworks {
    Get-Process | Where-Object { $_.ProcessName -like 'Vectorworks20*' } | Stop-Process -Force
    Start-Sleep 5
}

function Capture([string]$path) {
    $vw = Main-Window
    $h = $vw.MainWindowHandle
    if ([Cap]::IsIconic($h)) { [Cap]::ShowWindowAsync($h, 4) | Out-Null; Start-Sleep 2 }
    $r = New-Object Cap+RECT
    [Cap]::GetWindowRect($h, [ref]$r) | Out-Null
    $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $dc = $g.GetHdc()
    [Cap]::PrintWindow($h, $dc, 2) | Out-Null
    $g.ReleaseHdc($dc)
    $bmp.Save($path)
}

$n = 0
foreach ($file in $Files) {
    $n++
    if ($n -gt 1 -and ($n - 1) % $Batch -eq 0) { Stop-Vectorworks }
    Start-Vectorworks
    $name = [IO.Path]::GetFileNameWithoutExtension($file)
    $tag = '{0:D2}_{1}' -f $n, ($name -replace '[^\w\-]', '_')
    $copy = Join-Path $copies ($tag + [IO.Path]::GetExtension($file))
    Copy-Item -LiteralPath $file $copy -Force
    $before = (Documents).Count
    Start-Process $Vectorworks -ArgumentList "`"$copy`""
    $opened = $false
    for ($t = 0; $t -lt 900; $t += 3) {
        Start-Sleep 3
        Dismiss
        if ((Documents).Count -gt $before) { $opened = $true; break }
    }
    $title = (Main-Window).MainWindowTitle
    if ($opened) { Start-Sleep 10; Capture (Join-Path $Out "$($tag)_vw.png") }
    $shot = Join-Path $Out "$($tag)_app.png"
    # (the viewer writes what it loaded to stderr, which is no error)
    $ErrorActionPreference = 'Continue'
    $log = & $Viewer $copy --screenshot $shot --view iso --size 1600x1000 --no-remote 2>&1 | Out-String
    $ErrorActionPreference = 'Stop'
    $loaded = [string]($log -split "`n" | Where-Object { $_ -like "loaded*" } | Select-Object -First 1)
    Add-Content -LiteralPath $index -Value ("{0}`t{1}`tvw={2}`t{3}`t{4}" -f $tag, $file, $opened, $title, ($loaded -replace '\s+$', '')) -Encoding utf8
}
