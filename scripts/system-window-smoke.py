#!/usr/bin/env python3
"""Exercise the desktop window through system X11/XTest and IBus/XIM events.

This is a Linux integration harness, not a Rust unit test. It requires Xvfb,
xwininfo, dbus-run-session, IBus with libpinyin, and inspectable X11 libraries.
"""

from __future__ import annotations

import ctypes
import os
import re
import shlex
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path


WINDOW_RE = re.compile(
    r"^\s*(0x[0-9a-fA-F]+) \"Jasper UI Engine\"", re.MULTILINE
)


class Data(ctypes.Union):
    _fields_ = [
        ("b", ctypes.c_char * 20),
        ("s", ctypes.c_short * 10),
        ("l", ctypes.c_long * 5),
    ]


class ClientMessage(ctypes.Structure):
    _fields_ = [
        ("type", ctypes.c_int),
        ("serial", ctypes.c_ulong),
        ("send_event", ctypes.c_int),
        ("display", ctypes.c_void_p),
        ("window", ctypes.c_ulong),
        ("message_type", ctypes.c_ulong),
        ("format", ctypes.c_int),
        ("data", Data),
    ]


class X11:
    def __init__(self, display: str) -> None:
        self.x11 = ctypes.CDLL("libX11.so.6")
        self.xtst = ctypes.CDLL("libXtst.so.6")
        self._declare()
        self.display = self.x11.XOpenDisplay(display.encode())
        if not self.display:
            raise RuntimeError(f"cannot open X display {display}")

    def _declare(self) -> None:
        x11, xtst = self.x11, self.xtst
        x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
        x11.XOpenDisplay.restype = ctypes.c_void_p
        x11.XSetInputFocus.argtypes = [
            ctypes.c_void_p,
            ctypes.c_ulong,
            ctypes.c_int,
            ctypes.c_ulong,
        ]
        x11.XStringToKeysym.argtypes = [ctypes.c_char_p]
        x11.XStringToKeysym.restype = ctypes.c_ulong
        x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        x11.XKeysymToKeycode.restype = ctypes.c_uint
        x11.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
        x11.XInternAtom.restype = ctypes.c_ulong
        x11.XSendEvent.argtypes = [
            ctypes.c_void_p,
            ctypes.c_ulong,
            ctypes.c_int,
            ctypes.c_long,
            ctypes.c_void_p,
        ]
        x11.XFlush.argtypes = [ctypes.c_void_p]
        x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
        x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
        xtst.XTestFakeMotionEvent.argtypes = [
            ctypes.c_void_p,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_ulong,
        ]
        xtst.XTestFakeButtonEvent.argtypes = [
            ctypes.c_void_p,
            ctypes.c_uint,
            ctypes.c_int,
            ctypes.c_ulong,
        ]
        xtst.XTestFakeKeyEvent.argtypes = [
            ctypes.c_void_p,
            ctypes.c_uint,
            ctypes.c_int,
            ctypes.c_ulong,
        ]

    def focus(self, window: int) -> None:
        self.x11.XSetInputFocus(self.display, window, 2, 0)
        self.x11.XSync(self.display, 0)

    def click(self, window: int, x: int, y: int) -> None:
        self.focus(window)
        if not self.xtst.XTestFakeMotionEvent(self.display, 0, x, y, 0):
            raise RuntimeError("XTest motion injection failed")
        self.x11.XFlush(self.display)
        time.sleep(0.25)
        if not self.xtst.XTestFakeButtonEvent(self.display, 1, 1, 0):
            raise RuntimeError("XTest button-down injection failed")
        self.x11.XFlush(self.display)
        time.sleep(0.12)
        if not self.xtst.XTestFakeButtonEvent(self.display, 1, 0, 0):
            raise RuntimeError("XTest button-up injection failed")
        self.x11.XSync(self.display, 0)

    def key(self, name: str) -> None:
        sym = self.x11.XStringToKeysym(name.encode())
        code = self.x11.XKeysymToKeycode(self.display, sym)
        if not sym or not code:
            raise RuntimeError(f"cannot map key {name!r}")
        if not self.xtst.XTestFakeKeyEvent(self.display, code, 1, 0):
            raise RuntimeError(f"XTest key-down failed for {name!r}")
        self.x11.XFlush(self.display)
        time.sleep(0.10)
        if not self.xtst.XTestFakeKeyEvent(self.display, code, 0, 0):
            raise RuntimeError(f"XTest key-up failed for {name!r}")
        self.x11.XSync(self.display, 0)
        time.sleep(0.15)

    def close(self, window: int) -> None:
        event = ClientMessage()
        event.type = 33
        event.send_event = 1
        event.display = self.display
        event.window = window
        event.message_type = self.x11.XInternAtom(
            self.display, b"WM_PROTOCOLS", 0
        )
        event.format = 32
        event.data.l[0] = self.x11.XInternAtom(
            self.display, b"WM_DELETE_WINDOW", 0
        )
        if not self.x11.XSendEvent(
            self.display, window, 0, 0, ctypes.byref(event)
        ):
            raise RuntimeError("cannot send WM_DELETE_WINDOW")
        self.x11.XSync(self.display, 0)

    def __del__(self) -> None:
        display = getattr(self, "display", None)
        if display:
            self.x11.XCloseDisplay(display)


def wait_for_display(display: str, timeout: float = 10.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = subprocess.run(
            ["xdpyinfo", "-display", display],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if result.returncode == 0:
            return
        time.sleep(0.1)
    raise RuntimeError(f"Xvfb display {display} did not become ready")


def find_window(display: str, timeout: float = 20.0) -> tuple[int, int, int]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        tree = subprocess.run(
            ["xwininfo", "-display", display, "-root", "-tree"],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        ).stdout
        match = WINDOW_RE.search(tree)
        if match:
            window = int(match.group(1), 16)
            info = subprocess.run(
                ["xwininfo", "-display", display, "-id", match.group(1)],
                check=True,
                text=True,
                stdout=subprocess.PIPE,
            ).stdout
            x = int(re.search(r"Absolute upper-left X:\s+(-?\d+)", info).group(1))
            y = int(re.search(r"Absolute upper-left Y:\s+(-?\d+)", info).group(1))
            return window, x, y
        time.sleep(0.1)
    raise RuntimeError("Jasper window did not appear")


def finish(process: subprocess.Popen[str], timeout: float = 15.0) -> int:
    try:
        return process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)
        raise RuntimeError("window process did not exit")


def terminate(process: subprocess.Popen[str]) -> None:
    if process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def run_click(display: str, binary: Path, latin: Path, cjk: Path, out: Path) -> None:
    log = (out / "click.log").open("w")
    process = subprocess.Popen(
        [str(binary), str(latin), str(cjk), "--once-after-activation"],
        env={
            **os.environ,
            "DISPLAY": display,
            "WINIT_UNIX_BACKEND": "x11",
            "WAYLAND_DISPLAY": "",
        },
        text=True,
        stdout=log,
        stderr=subprocess.STDOUT,
    )
    try:
        window, x, y = find_window(display)
        X11(display).click(window, x + 100, y + 50)
        if finish(process) != 0:
            raise RuntimeError("click window process failed")
    finally:
        log.close()
        terminate(process)
    text = (out / "click.log").read_text()
    if "activated=1" not in text or "retired=true" not in text:
        raise RuntimeError("XTest click did not activate and retire the view")


def run_key(display: str, binary: Path, latin: Path, cjk: Path, out: Path) -> None:
    log = (out / "keyboard.log").open("w")
    process = subprocess.Popen(
        [str(binary), str(latin), str(cjk)],
        env={
            **os.environ,
            "DISPLAY": display,
            "WINIT_UNIX_BACKEND": "x11",
            "WAYLAND_DISPLAY": "",
        },
        text=True,
        stdout=log,
        stderr=subprocess.STDOUT,
    )
    try:
        window, _, _ = find_window(display)
        x11 = X11(display)
        x11.focus(window)
        time.sleep(0.3)
        x11.key("a")
        time.sleep(0.25)
        x11.close(window)
        if finish(process) != 0:
            raise RuntimeError("keyboard window process failed")
    finally:
        log.close()
        terminate(process)
    text = (out / "keyboard.log").read_text()
    if 'external_snapshot_revision=2 external_snapshot="External display 中文a"' not in text:
        raise RuntimeError("XTest keyboard did not advance the display snapshot")


def run_ime(display: str, binary: Path, latin: Path, cjk: Path, out: Path) -> None:
    command = (
        "export XMODIFIERS=@im=ibus GTK_IM_MODULE=ibus QT_IM_MODULE=ibus "
        "NO_AT_BRIDGE=1; "
        "ibus-daemon --panel=disable -drx; "
        "sleep 2; ibus engine libpinyin; "
        f"exec {shlex.quote(str(binary))} {shlex.quote(str(latin))} "
        f"{shlex.quote(str(cjk))}"
    )
    log = (out / "ime.log").open("w")
    process = subprocess.Popen(
        ["dbus-run-session", "--", "bash", "-c", command],
        env={
            **os.environ,
            "DISPLAY": display,
            "WINIT_UNIX_BACKEND": "x11",
            "WAYLAND_DISPLAY": "",
        },
        text=True,
        stdout=log,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    try:
        window, _, _ = find_window(display, timeout=30.0)
        x11 = X11(display)
        x11.focus(window)
        time.sleep(0.8)
        for key in ["n", "i", "h", "a", "o", "space"]:
            x11.key(key)
        time.sleep(1.0)
        x11.close(window)
        if finish(process, timeout=20.0) != 0:
            raise RuntimeError("IBus window process failed")
    finally:
        log.close()
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)
    text = (out / "ime.log").read_text()
    if 'external_snapshot_revision=2 external_snapshot="External display 中文你好"' not in text:
        raise RuntimeError("IBus/libpinyin did not commit 你好")


def main() -> int:
    if len(sys.argv) != 4:
        print(f"usage: {sys.argv[0]} LATIN_FONT CJK_FONT OUTPUT_DIR", file=sys.stderr)
        return 2
    latin, cjk, output = map(Path, sys.argv[1:])
    for tool in ["cargo", "Xvfb", "xdpyinfo", "xwininfo", "dbus-run-session", "ibus"]:
        if shutil.which(tool) is None:
            raise RuntimeError(f"required tool is missing: {tool}")
    output.mkdir(parents=True, exist_ok=True)
    display = f":{os.environ.get('JASPER_UI_XVFB_DISPLAY', '105')}"
    env = {
        **os.environ,
        "DISPLAY": display,
        "WINIT_UNIX_BACKEND": "x11",
        "WAYLAND_DISPLAY": "",
    }
    subprocess.run(
        [
            "cargo",
            "build",
            "--locked",
            "--features",
            "desktop,native-engine-cpu",
            "--example",
            "window",
        ],
        check=True,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.STDOUT,
    )
    binary = Path("target/debug/examples/window").resolve()
    xvfb_log = (output / "xvfb.log").open("w")
    xvfb = subprocess.Popen(
        [shutil.which("Xvfb"), display, "-screen", "0", "1280x720x24", "-nolisten", "tcp"],
        stdout=xvfb_log,
        stderr=subprocess.STDOUT,
    )
    try:
        wait_for_display(display)
        run_click(display, binary, latin.resolve(), cjk.resolve(), output)
        run_key(display, binary, latin.resolve(), cjk.resolve(), output)
        run_ime(display, binary, latin.resolve(), cjk.resolve(), output)
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(timeout=5)
        except subprocess.TimeoutExpired:
            xvfb.kill()
            xvfb.wait(timeout=5)
        xvfb_log.close()
    print(f"system_window_smoke=pass output={output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"system window smoke failed: {error}", file=sys.stderr)
        raise
