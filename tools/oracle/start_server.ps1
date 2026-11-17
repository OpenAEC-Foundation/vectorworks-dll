# Starts tools/oracle/server.py inside a running Vectorworks, with nobody at the keyboard:
# Extra > Insteekmodules > Voer script uit... (Tools > Plug-ins > Run Script...), then the file
# dialog. Vectorworks must be running with an empty document. Takes the foreground for a moment.
#
#   powershell -File start_server.ps1 [-Script C:\Users\...\vwx-samples\server.py]
param([string]$Script = "$env:USERPROFILE\vwx-samples\server.py")

$ErrorActionPreference = 'Stop'
Add-Type -Path (Join-Path $PSScriptRoot 'Msaa.cs') -ReferencedAssemblies Accessibility
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Fg { [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); }
'@

$vw = Get-Process | Where-Object { $_.ProcessName -like 'Vectorworks20*' -and $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $vw) { throw 'Vectorworks is not running' }
# the frame is the window with a title (an open menu is a window of the process too)
$frame = [Msaa]::Windows($vw.Id) | Where-Object { $_.Split('|')[2] -like 'Vectorworks*' } | Select-Object -First 1
if (-not $frame) { throw 'the Vectorworks window was not found' }
$main = [long]($frame.Split('|')[0])
# the menu bar: the toolbar whose items include the Extra/Tools menu
$bar = $null
foreach ($c in [Msaa]::ChildWindows($main)) {
    $h = [long]($c.Split('|')[0])
    if ($c -like '*BCGPToolBar*' -and (([Msaa]::Items($h) -join '|') -match '(^|\|)(Extra|Tools)(\||$)')) { $bar = $h; break }
}
if (-not $bar) { throw 'menu bar not found' }

function Windows { [Msaa]::Windows($vw.Id) | ForEach-Object { [long]($_.Split('|')[0]) } | Where-Object { $_ -ne $main } }
function Bars { foreach ($h in @(Windows)) { foreach ($c in [Msaa]::ChildWindows($h)) { [long]($c.Split('|')[0]) } } }
# the open menu that holds an item starting with one of $names, and that name
function Find-Item([string[]]$names) {
    foreach ($try in 1..20) {
        foreach ($b in @(Bars)) {
            foreach ($item in [Msaa]::Items($b)) {
                foreach ($n in $names) {
                    if ($item.Replace('&', '').StartsWith($n)) {
                        # a menu that has just opened does not take a click at once
                        Start-Sleep -Milliseconds 500
                        return @($b, $n)
                    }
                }
            }
        }
        Start-Sleep -Milliseconds 150
    }
    throw "menu item not found: $names"
}

[void][Fg]::SetForegroundWindow([IntPtr]$main)
Start-Sleep -Milliseconds 300
# close menus left open
if (@(Windows).Count -gt 0) { [System.Windows.Forms.SendKeys]::SendWait('{ESC}{ESC}{ESC}{ESC}'); Start-Sleep -Milliseconds 400 }
$menu = @('Extra', 'Tools') | Where-Object { ([Msaa]::Items($bar) -join '|') -match "(^|\|)$_(\||$)" } | Select-Object -First 1
[void][Msaa]::Click($bar, $menu)
$m = Find-Item 'Insteekmodules', 'Plug-ins'
[void][Msaa]::Click($m[0], $m[1])
$m = Find-Item 'Voer script uit', 'Run Script'
[Msaa]::ClickLater($m[0], $m[1])   # opens a modal file dialog

# the file dialog (a top-level window of the process of class #32770): file name, then Open
$dialog = 0
foreach ($try in 1..40) {
    Start-Sleep -Milliseconds 250
    $found = [Msaa]::Windows($vw.Id) | Where-Object { $_.Split('|')[1] -eq '#32770' } | Select-Object -First 1
    if ($found) { $dialog = [long]($found.Split('|')[0]); "dialog: $($found.Split('|')[2])"; break }
}
if (-not $dialog) { throw 'the file dialog did not open' }
Start-Sleep -Milliseconds 500
if (-not [Msaa]::ChooseFile($dialog, $Script)) { throw 'the dialog is not a file dialog' }

# the server writes jobs\server.alive when it runs
$alive = Join-Path (Split-Path $Script) 'jobs\server.alive'
foreach ($try in 1..60) {
    Start-Sleep -Milliseconds 500
    if (Test-Path $alive) { 'server is running'; exit 0 }
    # another dialog in between (a warning, an error of the script): name it
    if ($try % 10 -eq 0) { [Msaa]::Windows($vw.Id) | Where-Object { $_.Split('|')[1] -eq '#32770' } | ForEach-Object { "dialog open: $_" } }
}
'server did not start within 30 s'
exit 1
