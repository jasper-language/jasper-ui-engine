//! Explicit host input ports for the bare-metal core.
//! 裸机核心的显式宿主输入端口；不执行文件、字体、图片或时钟系统调用。

/// Host-owned monotonic clock. The unit is milliseconds and must not move backwards.
/// 宿主持有的单调时钟；单位为毫秒，调用方不得让它倒退。
pub trait CoreClock {
    fn now_millis(&self) -> u64;
}

/// Host-owned font bytes. The host authenticates and retains the returned storage.
/// 宿主持有的字体字节；宿主负责认证并持续拥有返回的存储。
pub trait CoreFontSource {
    fn font_bytes(&self, id: &str) -> Option<&[u8]>;
}

/// Host-owned encoded image bytes. Decoding remains outside the bare-metal core unless a
/// non-std image decoder is explicitly introduced later.
/// 宿主持有的编码图片字节；除非以后显式引入非 std 解码器，解码不属于裸机核心。
pub trait CoreImageSource {
    fn image_bytes(&self, id: &str) -> Option<&[u8]>;
}

/// Host-owned read-only file or embedded asset bytes. This trait performs no lookup or I/O.
/// 宿主持有的只读文件或内嵌资源字节；这个 trait 不执行查找或 I/O。
pub trait CoreFileSource {
    fn file_bytes(&self, id: &str) -> Option<&[u8]>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Host;

    impl CoreClock for Host {
        fn now_millis(&self) -> u64 {
            42
        }
    }

    impl CoreFontSource for Host {
        fn font_bytes(&self, id: &str) -> Option<&[u8]> {
            (id == "font").then_some(&b"font-bytes"[..])
        }
    }

    impl CoreImageSource for Host {
        fn image_bytes(&self, id: &str) -> Option<&[u8]> {
            (id == "image").then_some(&b"image-bytes"[..])
        }
    }

    impl CoreFileSource for Host {
        fn file_bytes(&self, id: &str) -> Option<&[u8]> {
            (id == "file").then_some(&b"file-bytes"[..])
        }
    }

    #[test]
    fn host_ports_borrow_without_core_io() {
        let host = Host;
        assert_eq!(host.now_millis(), 42);
        assert_eq!(host.font_bytes("font"), Some(&b"font-bytes"[..]));
        assert_eq!(host.image_bytes("image"), Some(&b"image-bytes"[..]));
        assert_eq!(host.file_bytes("file"), Some(&b"file-bytes"[..]));
        assert_eq!(host.font_bytes("missing"), None);
        assert_eq!(host.image_bytes("missing"), None);
        assert_eq!(host.file_bytes("missing"), None);
    }
}
