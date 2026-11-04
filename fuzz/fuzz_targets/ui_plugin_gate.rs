#![no_main]
use jaso::build::{Builder, SegmentIn, Signer};
use jaso::platform::{FloatAbi, Target};
use jaso::trust::{Publisher, TrustMode, TrustPolicy};
use jaso::{Arch, Jaso, Stage, FLAG_R, FLAG_X};
use jasper_ui_engine::ui_plugin_gate::{
    admit_paint_output, node_pixel_box, paint_rect_admits, relocation_value, ui_manifest_admit, UiPaintCommandV1,
};
use jasper_ui_engine::ui_plugin_host::{admit, host_platform};
use libfuzzer_sys::fuzz_target;
use std::sync::OnceLock;

const MANIFEST: &str = "core.name = ui-fuzz-paint\ncore.version = 0.1.0\ncore.abi = c.v1\ncore.stage = runtime\n\
    core.publisher = ui.fuzz\ncore.requires = ui.paint\ncore.exports = ui_paint_v1\n\
    ui.abi = 1\nui.kind = paint-node\nui.paint = ui_paint_v1\n";

fn signer() -> Signer {
    Signer::from_seed(&[0x33; 32])
}

fn baseline() -> &'static (Vec<u8>, Vec<u8>) {
    static FILES: OnceLock<(Vec<u8>, Vec<u8>)> = OnceLock::new();
    FILES.get_or_init(|| {
        let mut builder = Builder::native(Arch::X86_64, Stage::Runtime, MANIFEST.as_bytes());
        builder.platform.target = Target::Host;
        builder.platform.float_abi = FloatAbi::Hard;
        builder.segments.push(SegmentIn { vaddr: 0, mem_size: 16, flags: FLAG_R | FLAG_X, data: vec![0xc3; 16] });
        builder.exports.push((b"ui_paint_v1".to_vec(), 0));
        (builder.finish_signed(&signer()).unwrap(), builder.finish().unwrap())
    })
}

fn repair_digest(file: &mut [u8]) {
    if file.len() >= jaso::DIGEST_OFFSET + 32 {
        let digest = jaso::digest_of(file);
        file[jaso::DIGEST_OFFSET..jaso::DIGEST_OFFSET + 32].copy_from_slice(&digest);
    }
}

fn u32_at(data: &[u8], at: usize) -> u32 {
    let mut bytes = [0; 4];
    for (slot, byte) in bytes.iter_mut().zip(data.iter().skip(at)) {
        *slot = *byte;
    }
    u32::from_le_bytes(bytes)
}

fuzz_target!(|data: &[u8]| {
    let word = |i: usize| u32_at(data, i * 4);
    let (nw, nh) = (word(0) % 4096, word(1) % 4096);

    let commands: Vec<UiPaintCommandV1> = data
        .chunks_exact(32)
        .map(|c| UiPaintCommandV1 {
            kind: u32_at(c, 0) % 3,
            x: u32_at(c, 4) % 5000,
            y: u32_at(c, 8) % 5000,
            width: u32_at(c, 12) % 5000,
            height: u32_at(c, 16) % 5000,
            rgba: [c[20], c[21], c[22], c[23]],
            reserved: [u32_at(c, 24) % 2, u32_at(c, 28) % 2],
        })
        .collect();
    if let Ok(admitted) = admit_paint_output(word(2) as i32 % 8, &commands, nw, nh) {
        for c in admitted {
            assert!(paint_rect_admits(c.x, c.y, c.width, c.height, nw, nh));
            assert!(u64::from(c.x) + u64::from(c.width) <= u64::from(nw));
            assert!(u64::from(c.y) + u64::from(c.height) <= u64::from(nh));
        }
    }

    let base = u64::from(word(3)) << 32 | u64::from(word(4));
    if let Some(value) = relocation_value(base, word(5), u64::from(word(6)), word(7)) {
        assert_eq!(u128::from(value), u128::from(base) + u128::from(word(6)));
        assert!(u64::from(word(5)) + 8 <= u64::from(word(7)));
    }

    let float = |i: usize| f32::from_bits(word(i));
    if let Some(snapped) = node_pixel_box(float(8), float(9), float(10), float(11), f64::from(float(12))) {
        assert!(i64::from(snapped.x) + i64::from(snapped.width) <= i64::from(i32::MAX));
    }

    let text = String::from_utf8_lossy(data);
    let mut pieces = text.split('\n');
    let (abi, kind, entry) = (pieces.next(), pieces.next(), pieces.next());
    if let Ok(name) = ui_manifest_admit(abi, kind, entry, pieces.clone(), pieces.clone()) {
        assert_eq!(Some(name), entry);
        assert!(abi == Some("1") && kind == Some("paint-node"));
    }

    // Raw input and mutated valid files reach the real admission; it maps and calls nothing.
    // 原始输入与变异后的合法文件都进入真实准入；它不映射也不调用任何代码。
    let publishers = [Publisher { name: "ui.fuzz", namespace: "ui-fuzz-", key: signer().public_key() }];
    let enforce = TrustPolicy::new(&publishers, TrustMode::Enforce).unwrap();
    let development = TrustPolicy::new(&publishers, TrustMode::Development).unwrap();
    let platform = host_platform().unwrap();
    let granted = ["ui.paint"];
    let _ = admit(data, &enforce, &platform, &granted, |_| false);

    let (signed, unsigned) = baseline();
    let edits = data.chunks_exact(3).take(8);
    let mut forged = signed.clone();
    for edit in edits.clone() {
        let at = usize::from(edit[0]) | usize::from(edit[1]) << 8;
        let len = forged.len();
        forged[at % len] ^= edit[2];
    }
    repair_digest(&mut forged);
    if admit(&forged, &enforce, &platform, &granted, |_| false).is_ok() {
        // Anything the signature covers must be unchanged for the file to be admitted.
        // 文件要被准入，签名覆盖的内容必须未改动。
        let tail = forged.len() - 64;
        assert_eq!(forged[..tail], signed[..tail], "a changed signed file passed admission");
    }

    let mut open = unsigned.clone();
    for edit in edits {
        let at = usize::from(edit[0]) | usize::from(edit[1]) << 8;
        let len = open.len();
        open[at % len] ^= edit[2];
    }
    repair_digest(&mut open);
    if let Ok((file, image, _, entry)) = admit(&open, &development, &platform, &granted, |_| false) {
        assert!(file.manifest.exports().any(|name| name == entry));
        assert!(Jaso::parse(&open).is_ok());
        assert!(image.export(entry).is_some());
    }
});
