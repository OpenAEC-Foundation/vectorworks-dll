// Drives the menus of a running Vectorworks through MSAA (its menu bar is a BCG toolbar that
// UI Automation shows no items for). Used by start_server.ps1 to choose
// Extra > Plug-ins > Script uitvoeren without anyone at the keyboard.
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using Accessibility;

public static class Msaa
{
    [DllImport("oleacc.dll")]
    static extern int AccessibleObjectFromWindow(IntPtr hwnd, uint id, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out object acc);

    [DllImport("oleacc.dll")]
    static extern int AccessibleChildren(IAccessible acc, int start, int count, [Out] object[] children, out int obtained);

    delegate bool EnumProc(IntPtr hwnd, IntPtr lparam);

    [DllImport("user32.dll")]
    static extern bool EnumWindows(EnumProc proc, IntPtr lparam);

    [DllImport("user32.dll")]
    static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);

    [DllImport("user32.dll")]
    static extern bool IsWindowVisible(IntPtr hwnd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern int GetClassName(IntPtr hwnd, StringBuilder text, int max);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int max);

    static IAccessible FromWindow(IntPtr hwnd)
    {
        Guid iid = new Guid("618736E0-3C3D-11CF-810C-00AA00389B71");
        object o;
        AccessibleObjectFromWindow(hwnd, 0xFFFFFFFC, ref iid, out o); // OBJID_CLIENT
        return o as IAccessible;
    }

    /// The visible top-level windows of a process: "handle|class|title" each.
    public static string[] Windows(int pid)
    {
        var found = new List<string>();
        EnumWindows((h, l) =>
        {
            uint p;
            GetWindowThreadProcessId(h, out p);
            if (p == (uint)pid && IsWindowVisible(h))
            {
                var cls = new StringBuilder(256);
                var title = new StringBuilder(256);
                GetClassName(h, cls, 256);
                GetWindowText(h, title, 256);
                found.Add(h.ToInt64() + "|" + cls + "|" + title);
            }
            return true;
        }, IntPtr.Zero);
        return found.ToArray();
    }

    [DllImport("user32.dll")]
    static extern bool EnumChildWindows(IntPtr parent, EnumProc proc, IntPtr lparam);

    /// The child windows of a window (a popup menu holds its items in one): "handle|class|title".
    public static string[] ChildWindows(long hwnd)
    {
        var found = new List<string>();
        EnumChildWindows(new IntPtr(hwnd), (h, l) =>
        {
            var cls = new StringBuilder(256);
            var title = new StringBuilder(256);
            GetClassName(h, cls, 256);
            GetWindowText(h, title, 256);
            found.Add(h.ToInt64() + "|" + cls + "|" + title);
            return true;
        }, IntPtr.Zero);
        return found.ToArray();
    }

    static object[] Children(IAccessible acc)
    {
        int n = 0;
        try { n = acc.accChildCount; } catch { }
        var kids = new object[n];
        int got = 0;
        if (n > 0) AccessibleChildren(acc, 0, n, kids, out got);
        Array.Resize(ref kids, got);
        return kids;
    }

    static string NameOf(IAccessible parent, object child)
    {
        try { return child is IAccessible ? ((IAccessible)child).get_accName(0) : parent.get_accName(child); }
        catch { return null; }
    }

    /// The names of the items of a window (a menu bar or a popup menu), in order.
    public static string[] Items(long hwnd)
    {
        var acc = FromWindow(new IntPtr(hwnd));
        var names = new List<string>();
        if (acc == null) return names.ToArray();
        foreach (var kid in Children(acc)) names.Add(NameOf(acc, kid) ?? "");
        return names.ToArray();
    }

    [DllImport("user32.dll")]
    static extern IntPtr GetDlgItem(IntPtr dialog, int id);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern IntPtr SendMessage(IntPtr hwnd, uint msg, IntPtr wparam, string lparam);

    [DllImport("user32.dll")]
    static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);

    /// Fills in the file name of a standard Windows file dialog (control 0x47c) and presses its
    /// default button (control 1). False when the dialog has no such controls.
    public static bool ChooseFile(long dialog, string path)
    {
        IntPtr name = GetDlgItem(new IntPtr(dialog), 0x47c);
        IntPtr ok = GetDlgItem(new IntPtr(dialog), 1);
        if (name == IntPtr.Zero || ok == IntPtr.Zero) return false;
        SendMessage(name, 0x000C, IntPtr.Zero, path); // WM_SETTEXT
        PostMessage(ok, 0x00F5, IntPtr.Zero, IntPtr.Zero); // BM_CLICK
        return true;
    }

    /// Presses a button of a dialog by its control id (1 is OK, 2 Cancel).
    public static bool Press(long dialog, int id)
    {
        IntPtr button = GetDlgItem(new IntPtr(dialog), id);
        if (button == IntPtr.Zero) return false;
        PostMessage(button, 0x00F5, IntPtr.Zero, IntPtr.Zero); // BM_CLICK
        return true;
    }

    /// The text of a message box (its static control 0xffff or 3), for the log.
    public static string DialogText(long dialog)
    {
        var sb = new StringBuilder();
        foreach (var line in ChildWindows(dialog))
        {
            var parts = line.Split(new[] { '|' }, 3);
            if (parts.Length == 3 && parts[2].Length > 0) sb.Append(parts[2]).Append(" / ");
        }
        return sb.ToString();
    }

    /// As `Click`, on a background thread: for an item that opens a modal dialog, whose click
    /// returns only when the dialog closes.
    public static void ClickLater(long hwnd, string name)
    {
        var t = new System.Threading.Thread(() => { try { Click(hwnd, name); } catch { } });
        t.IsBackground = true;
        t.SetApartmentState(System.Threading.ApartmentState.STA);
        t.Start();
    }

    /// Does the default action (a click) of the first item whose name starts with `name`.
    public static bool Click(long hwnd, string name)
    {
        var acc = FromWindow(new IntPtr(hwnd));
        if (acc == null) return false;
        foreach (var kid in Children(acc))
        {
            var n = NameOf(acc, kid);
            if (n == null || !n.Replace("&", "").StartsWith(name, StringComparison.OrdinalIgnoreCase)) continue;
            if (kid is IAccessible) ((IAccessible)kid).accDoDefaultAction(0);
            else acc.accDoDefaultAction(kid);
            return true;
        }
        return false;
    }
}
