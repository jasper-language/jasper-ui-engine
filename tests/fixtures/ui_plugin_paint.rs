//! Native `paint-node` plugin compiled by the plugin acceptance test: no libc, no imports.
//! 由插件验收测试编译的原生 `paint-node` 插件：无 libc，无导入。
//!
//! The relocated pointer table proves base-plus-addend links, `READY` proves init ran before
//! paint, and `ui_paint_v1` reads the request size. `MODE` selects misbehaviour for negative tests.
//! 重定位过的指针表证明「基址 + 加数」已链接，`READY` 证明 init 先于 paint，`ui_paint_v1` 读取请求尺寸。
//! `MODE` 为负向测试选择异常行为。
#![no_std]
#![no_main]

use core::sync::atomic::{AtomicI32, Ordering};

#[repr(C)]
pub struct Host {
    version: u32,
    size: u32,
    log: extern "C" fn(*const u8, usize),
    alloc: extern "C" fn(usize, usize) -> *mut u8,
    dealloc: extern "C" fn(*mut u8, usize, usize),
}

#[repr(C)]
pub struct Request {
    version: u32,
    size: u32,
    node_width: u32,
    node_height: u32,
}

#[repr(C)]
pub struct Command {
    kind: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    rgba: [u8; 4],
    reserved: [u32; 2],
}

struct Colors([&'static [u8; 4]; 2]);
// SAFETY: read-only bytes.
// SAFETY：只读字节。
unsafe impl Sync for Colors {}

static TEAL: [u8; 4] = [10, 200, 160, 255];
static AMBER: [u8; 4] = [240, 170, 20, 255];
#[used]
static COLORS: Colors = Colors([&TEAL, &AMBER]);

static READY: AtomicI32 = AtomicI32::new(0);
static STOPS: AtomicI32 = AtomicI32::new(0);

#[no_mangle]
pub extern "C" fn jaso_init(host: *const Host) -> i32 {
    // SAFETY: the loader passes a valid table.
    // SAFETY：装载器传入有效的表。
    let host = unsafe { &*host };
    if host.version != 1 {
        return 7;
    }
    let block = (host.alloc)(16, 8);
    if block.is_null() {
        return 8;
    }
    (host.dealloc)(block, 16, 8);
    let line = b"paint plugin ready";
    (host.log)(line.as_ptr(), line.len());
    READY.store(41, Ordering::SeqCst);
    MODE_INIT
}

#[no_mangle]
pub extern "C" fn jaso_start() -> i32 {
    READY.fetch_add(1, Ordering::SeqCst);
    0
}

#[no_mangle]
pub extern "C" fn jaso_stop() -> i32 {
    STOPS.fetch_add(1, Ordering::SeqCst);
    0
}

const MODE_INIT: i32 = if cfg!(plugin_mode = "init-fails") { 9 } else { 0 };

/// Paints two inset rectangles: a full-node teal one, then an amber quarter at (w/2, h/2).
/// 绘制两个矩形：铺满节点的青色，再在 (w/2, h/2) 处绘制四分之一大小的琥珀色。
#[no_mangle]
pub unsafe extern "C" fn ui_paint_v1(request: *const Request, out: *mut Command, capacity: u32) -> i32 {
    let request = &*request;
    if READY.load(Ordering::SeqCst) != 42 || request.version != 1 || capacity < 2 {
        return -1;
    }
    if cfg!(plugin_mode = "negative-count") {
        return -5;
    }
    if cfg!(plugin_mode = "too-many") {
        return capacity as i32 + 1;
    }
    let (w, h) = (request.node_width, request.node_height);
    let grow = if cfg!(plugin_mode = "out-of-node") { 1 } else { 0 };
    let command = |x, y, width, height, rgba: &[u8; 4]| Command {
        kind: 1,
        x,
        y,
        width,
        height,
        rgba: *rgba,
        reserved: [0, 0],
    };
    out.write(command(0, 0, w + grow, h, COLORS.0[0]));
    out.add(1).write(command(w / 2, h / 2, w - w / 2, h - h / 2, COLORS.0[1]));
    2
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
