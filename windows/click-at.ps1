param(
    [Parameter(Mandatory = $true)]
    [int]$X,
    [Parameter(Mandatory = $true)]
    [int]$Y
)

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class JasperMouseControl
{
    [StructLayout(LayoutKind.Sequential)]
    public struct Point
    {
        public int X;
        public int Y;
    }

    [DllImport("user32.dll")]
    public static extern bool SetCursorPos(int x, int y);

    [DllImport("user32.dll")]
    public static extern bool SetProcessDpiAwarenessContext(IntPtr context);

    [DllImport("user32.dll")]
    public static extern bool GetCursorPos(out Point point);

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    public static extern IntPtr WindowFromPoint(Point point);

    [DllImport("user32.dll")]
    public static extern void mouse_event(
        uint flags,
        uint dx,
        uint dy,
        uint data,
        System.UIntPtr extraInfo);

    [StructLayout(LayoutKind.Sequential)]
    public struct MouseInput
    {
        public int Dx;
        public int Dy;
        public uint MouseData;
        public uint Flags;
        public uint Time;
        public UIntPtr ExtraInfo;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct Input
    {
        public uint Type;
        public MouseInput Mouse;
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint SendInput(
        uint count,
        Input[] inputs,
        int size);

    public static uint ClickAt(int x, int y)
    {
        var down = new Input {
            Type = 0,
            Mouse = new MouseInput { Flags = 0x0002 },
        };
        var up = new Input {
            Type = 0,
            Mouse = new MouseInput { Flags = 0x0004 },
        };
        return SendInput(2, new[] { down, up }, Marshal.SizeOf(typeof(Input)));
    }
}
"@

[JasperMouseControl]::SetProcessDpiAwarenessContext(
    [IntPtr](-4)) | Out-Null
[JasperMouseControl]::SetCursorPos($X, $Y) | Out-Null
$point = New-Object JasperMouseControl+Point
[JasperMouseControl]::GetCursorPos([ref]$point) | Out-Null
$foreground = [JasperMouseControl]::GetForegroundWindow()
$underCursor = [JasperMouseControl]::WindowFromPoint($point)
$sent = [JasperMouseControl]::ClickAt($X, $Y)
Write-Output (
    "clicked=$X,$Y cursor=$($point.X),$($point.Y) " +
    "foreground=$foreground target=$underCursor sent=$sent")
