param(
    [string]$ProcessName = "window",
    [int]$X = 24,
    [int]$Y = 24,
    [int]$Width = 960,
    [int]$Height = 700
)

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class JasperWindowControl
{
    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr window, int command);

    [DllImport("user32.dll")]
    public static extern bool SetWindowPos(
        IntPtr window,
        IntPtr insertAfter,
        int x,
        int y,
        int width,
        int height,
        uint flags);

    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr window);

    [DllImport("user32.dll")]
    public static extern bool SetFocus(IntPtr window);

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(
        IntPtr window,
        out uint processId);

    [DllImport("kernel32.dll")]
    public static extern uint GetCurrentThreadId();

    [DllImport("user32.dll")]
    public static extern bool AttachThreadInput(
        uint attach,
        uint attachTo,
        bool attachFlag);

    [DllImport("user32.dll")]
    public static extern bool BringWindowToTop(IntPtr window);

    public static bool ForceForeground(IntPtr window)
    {
        uint targetProcess;
        var foreground = GetForegroundWindow();
        uint foregroundProcess;
        var currentThread = GetCurrentThreadId();
        var foregroundThread = GetWindowThreadProcessId(
            foreground,
            out foregroundProcess);
        var targetThread = GetWindowThreadProcessId(
            window,
            out targetProcess);

        var attachedForeground = foregroundThread != 0 &&
            AttachThreadInput(currentThread, foregroundThread, true);
        var attachedTarget = targetThread != 0 &&
            AttachThreadInput(currentThread, targetThread, true);
        BringWindowToTop(window);
        SetForegroundWindow(window);
        SetFocus(window);
        if (attachedTarget)
            AttachThreadInput(currentThread, targetThread, false);
        if (attachedForeground)
            AttachThreadInput(currentThread, foregroundThread, false);
        return GetForegroundWindow() == window;
    }
}
"@

$process = Get-Process $ProcessName | Select-Object -First 1
if ($null -eq $process) {
    throw "process '$ProcessName' is not running"
}

$window = $process.MainWindowHandle
if ($window -eq [IntPtr]::Zero) {
    throw "process '$ProcessName' has no main window"
}

$shown = [JasperWindowControl]::ShowWindow($window, 9)
$positioned = [JasperWindowControl]::SetWindowPos(
    $window,
    [IntPtr](-1),
    $X,
    $Y,
    $Width,
    $Height,
    0x0040)
$focused = [JasperWindowControl]::ForceForeground($window)

Write-Output (
    "window=$($process.Id) title=$($process.MainWindowTitle) " +
    "shown=$shown positioned=$positioned focused=$focused")
