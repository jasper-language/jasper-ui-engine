//! Real native `.jaso` plugin, compiled here, loaded by L1 and painted into one headless frame.
//! 真实原生 `.jaso` 插件：在此编译，经 L1 装入，并绘入一帧无头画面。
#![cfg(all(feature = "ui-plugin-pack", feature = "native-blitz-cpu-pending"))]
use jaso::build::{Builder, Signer};
use jaso::code::ErrorCode;
use jaso::lifecycle::{StopError, UnloadReply};
use jaso::platform::{PlatformField, Target};
use jaso::trust::{Publisher, TrustMode, TrustPolicy};
use jaso::{Arch, Jaso, Stage};
use jasper_ui_engine::native::host::SceneHostView;
use jasper_ui_engine::scene::{Node, NodeId, Scene};
use jasper_ui_engine::style::{Color, Length, Style};
use jasper_ui_engine::ui_plugin_gate::{
    UiManifestError, UiPaintCommandV1, UiPaintError, UiPaintLayer, UI_PAINT_COMMAND_CAPACITY,
    UI_PAINT_FILL_RECT,
};
use jasper_ui_engine::ui_plugin_host::{
    host_platform, UiPluginAudit, UiPluginHost, UiPluginLoadError, UiPluginPaintError,
};
use std::process::Command;
use std::sync::OnceLock;

const W: u32 = 120;
const H: u32 = 80;
const OWNER: u64 = 31;
const VERSION: u64 = 1;
const BUDGET: usize = 1024 * 1024;
const NODE: NodeId = NodeId(2);
const PUBLISHER: &str = "ui.acceptance";
const NAMESPACE: &str = "ui-test-";
const GRANTED: [&str; 1] = ["ui.paint"];

fn manifest(name: &str, requires: &str, ui: &str) -> String {
    format!(
        "core.name = {name}\ncore.version = 0.1.0\ncore.abi = c.v1\ncore.stage = runtime\n\
         core.publisher = {PUBLISHER}\ncore.host = jaso.host.v1\ncore.requires = {requires}\n\
         core.exports = jaso_init, jaso_start, jaso_stop, ui_paint_v1\n{ui}"
    )
}

const UI_KEYS: &str = "ui.abi = 1\nui.kind = paint-node\nui.paint = ui_paint_v1\n";

fn signer() -> Signer {
    Signer::from_seed(&[0x5a; 32])
}

fn compile(mode: &str) -> Vec<u8> {
    let out =
        std::env::temp_dir().join(format!("jasper-ui-plugin-{}-{mode}.so", std::process::id()));
    let source = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ui_plugin_paint.rs"
    );
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let run = Command::new(rustc)
        .args([
            "--edition=2021",
            "--crate-type=cdylib",
            "--crate-name=ui_plugin_paint",
        ])
        .args([
            "-Cpanic=abort",
            "-Copt-level=2",
            "-Cstrip=debuginfo",
            "-Crelocation-model=pic",
        ])
        .args([
            "-Clink-arg=-nostdlib",
            "-Clink-arg=-nostartfiles",
            "-Clink-arg=-Wl,--no-eh-frame-hdr",
        ])
        .args([
            "-Clink-arg=-Wl,-z,max-page-size=4096",
            "-Clink-arg=-Wl,-z,separate-code",
            "-Clink-arg=-Wl,-z,noexecstack",
        ])
        .args([
            "-Clink-arg=-Wl,--hash-style=gnu",
            "--check-cfg=cfg(plugin_mode,values(any()))",
        ])
        .arg(format!("--cfg=plugin_mode=\"{mode}\""))
        .arg("-o")
        .arg(&out)
        .arg(source)
        .output()
        .expect("rustc must start: the acceptance test really compiles its plugin");
    assert!(
        run.status.success(),
        "plugin build failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let elf = std::fs::read(&out).expect("plugin output readable");
    std::fs::remove_file(&out).expect("plugin output removable");
    elf
}

fn elf() -> &'static [u8] {
    static ELF: OnceLock<Vec<u8>> = OnceLock::new();
    ELF.get_or_init(|| compile("normal"))
}

fn pack_elf(elf: &[u8], manifest: &str, target: Target) -> Vec<u8> {
    let raw =
        jaso::pack::from_elf(elf, manifest, Arch::X86_64, Stage::Runtime).expect("pack plugin ELF");
    let file = Jaso::parse(&raw).expect("packed file parses");
    let mut builder = Builder::from_file(&file, 0).expect("image 0 exists");
    builder.platform.target = target;
    builder.platform.float_abi = jaso::platform::FloatAbi::Hard;
    builder.finish_signed(&signer()).expect("sign plugin")
}

fn plugin(name: &str) -> Vec<u8> {
    pack_elf(elf(), &manifest(name, "ui.paint", UI_KEYS), Target::Host)
}

fn with_policy<R>(run: impl FnOnce(&TrustPolicy<'_>) -> R) -> R {
    let publishers = [Publisher {
        name: PUBLISHER,
        namespace: NAMESPACE,
        key: signer().public_key(),
    }];
    let policy = TrustPolicy::new(&publishers, TrustMode::Enforce).expect("policy");
    run(&policy)
}

fn load(
    host: &mut UiPluginHost,
    bytes: &[u8],
    granted: &[&str],
) -> Result<jasper_ui_engine::ui_plugin_host::UiPluginId, UiPluginLoadError> {
    with_policy(|policy| host.load(bytes, policy, granted))
}

fn scene() -> Scene {
    Scene::new(
        Node::container(NodeId(1))
            .with_style(
                Style::default()
                    .size(Length::Px(W as f32), Length::Px(H as f32))
                    .background(Color::rgb(30, 30, 40)),
            )
            .child(
                Node::rect(NODE).with_style(
                    Style::default()
                        .size(Length::Px(60.0), Length::Px(40.0))
                        .background(Color::rgb(200, 40, 40)),
                ),
            ),
    )
}

fn view() -> SceneHostView {
    let path = std::env::var("JASPER_UI_TEST_FONT")
        .expect("JASPER_UI_TEST_FONT must name an injected Latin font");
    let font = std::fs::read(path).expect("injected font readable");
    SceneHostView::new(OWNER, &scene(), &font, W, H).expect("native import")
}

fn plain_frame(view: &mut SceneHostView) -> Vec<u8> {
    let mut bytes = vec![0; (W * H * 4) as usize];
    assert!(
        view.render_to_surface(OWNER, VERSION, &mut bytes, (W * 4) as usize, BUDGET),
        "plain frame"
    );
    bytes
}

fn pixel(frame: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * W + x) * 4) as usize;
    frame[at..at + 4].try_into().expect("in frame")
}

#[test]
fn signed_plugin_loads_and_paints_into_one_frame() {
    let mut view = view();
    let plain = plain_frame(&mut view);
    assert_eq!(
        pixel(&plain, 10, 10),
        [200, 40, 40, 255],
        "node background before the plugin"
    );

    let node = view.node_pixel_box(OWNER, VERSION, NODE).expect("node box");
    assert_eq!((node.width, node.height), (60, 40));
    let mut host = UiPluginHost::new();
    let id = load(&mut host, &plugin("ui-test-paint"), &GRANTED).expect("L1 load");
    let commands = host
        .paint(id, node.width, node.height)
        .expect("plugin paints");
    assert_eq!(commands.len(), 2);

    let layers = [UiPaintLayer {
        node: NODE,
        commands: &commands,
    }];
    let frame = view
        .render_cpu_with_paint_layers(OWNER, VERSION, &layers, BUDGET)
        .expect("plugin frame");
    assert_eq!(
        pixel(&frame, 10, 10),
        [10, 200, 160, 255],
        "teal from the relocated table"
    );
    assert_eq!(
        pixel(&frame, 40, 30),
        [240, 170, 20, 255],
        "amber quarter painted after teal"
    );
    assert_eq!(
        pixel(&frame, 59, 39),
        [240, 170, 20, 255],
        "last node pixel"
    );
    assert_eq!(
        pixel(&frame, 60, 39),
        [30, 30, 40, 255],
        "right of the node stays scene"
    );
    assert_eq!(
        pixel(&frame, 59, 40),
        [30, 30, 40, 255],
        "below the node stays scene"
    );

    let again = plain_frame(&mut view);
    assert_eq!(again, plain, "a plugin frame does not enter the view frame");
    assert!(
        matches!(host.audit().last(), Some(UiPluginAudit::Loaded { name, publisher: Some(p) }) if name == "ui-test-paint" && p == PUBLISHER)
    );
}

#[test]
fn layers_are_admitted_before_any_painting() {
    let mut view = view();
    let good = UiPaintCommandV1 {
        kind: UI_PAINT_FILL_RECT,
        width: 60,
        height: 40,
        rgba: [1, 2, 3, 255],
        ..Default::default()
    };
    let outside = UiPaintCommandV1 { width: 61, ..good };
    let unknown = UiPaintCommandV1 { kind: 2, ..good };
    let reject = |view: &mut SceneHostView, node: NodeId, commands: &[UiPaintCommandV1]| {
        view.render_cpu_with_paint_layers(
            OWNER,
            VERSION,
            &[UiPaintLayer { node, commands }],
            BUDGET,
        )
        .is_none()
    };
    assert!(
        reject(&mut view, NODE, &[good, outside]),
        "a leaving rectangle refuses the frame"
    );
    assert!(
        reject(&mut view, NODE, &[unknown]),
        "an unknown kind refuses the frame"
    );
    assert!(
        reject(&mut view, NodeId(99), &[good]),
        "a node without bounds refuses the frame"
    );
    let layers = [UiPaintLayer {
        node: NODE,
        commands: &[good],
    }];
    assert!(
        view.render_cpu_with_paint_layers(OWNER + 1, VERSION, &layers, BUDGET)
            .is_none(),
        "foreign caller"
    );
    assert!(
        view.render_cpu_with_paint_layers(OWNER, VERSION + 1, &layers, BUDGET)
            .is_none(),
        "stale version"
    );
    assert!(
        view.render_cpu_with_paint_layers(OWNER, VERSION, &layers, 16)
            .is_none(),
        "over budget"
    );
    assert!(view
        .render_cpu_with_paint_layers(OWNER, VERSION, &layers, BUDGET)
        .is_some());
}

#[test]
fn wrong_platform_is_rejected_with_the_differing_field() {
    let bytes = pack_elf(
        elf(),
        &manifest("ui-test-os", "ui.paint", UI_KEYS),
        Target::JasperOs,
    );
    let mut host = UiPluginHost::new();
    match load(&mut host, &bytes, &GRANTED) {
        Err(UiPluginLoadError::Platform(mismatch)) => {
            assert_eq!(mismatch.field, PlatformField::Target)
        }
        other => panic!("expected platform mismatch, got {:?}", other.map(|id| id.0)),
    }
    assert!(host_platform().is_some(), "this acceptance host maps code");
    assert_eq!(
        host.unload_request(jasper_ui_engine::ui_plugin_host::UiPluginId(0)),
        None,
        "no table entry"
    );
}

#[test]
fn missing_capability_is_rejected_and_the_gate_requires_the_request() {
    let mut host = UiPluginHost::new();
    let denied = load(&mut host, &plugin("ui-test-deny"), &[]);
    assert_eq!(
        denied.map(|id| id.0),
        Err(UiPluginLoadError::Jaso(ErrorCode::CapabilityDenied))
    );

    // Asking for less than ui.paint is refused too, even though nothing was denied.
    // 未请求 ui.paint 同样被拒绝，即使没有能力被否决。
    let quiet = pack_elf(
        elf(),
        &manifest("ui-test-quiet", "log", UI_KEYS),
        Target::Host,
    );
    let refused = load(&mut host, &quiet, &["log"]);
    assert_eq!(
        refused.map(|id| id.0),
        Err(UiPluginLoadError::Manifest(
            UiManifestError::CapabilityNotRequested
        ))
    );
}

#[test]
fn tampering_and_trust_failures_are_rejected_before_mapping() {
    let good = plugin("ui-test-tamper");
    let mut host = UiPluginHost::new();

    let mut flipped = good.clone();
    let at = flipped
        .windows(10)
        .position(|w| w == b"paint-node")
        .expect("manifest bytes");
    flipped[at] ^= 1;
    assert_eq!(
        load(&mut host, &flipped, &GRANTED).map(|id| id.0),
        Err(UiPluginLoadError::Jaso(ErrorCode::FormatDigest))
    );

    // Digest repaired by the attacker: only the signature can still notice.
    // 攻击者修好摘要：只有签名还能发现。
    let digest = jaso::digest_of(&flipped);
    flipped[jaso::DIGEST_OFFSET..jaso::DIGEST_OFFSET + 32].copy_from_slice(&digest);
    assert_eq!(
        load(&mut host, &flipped, &GRANTED).map(|id| id.0),
        Err(UiPluginLoadError::Jaso(ErrorCode::SignatureInvalid))
    );

    let outside = pack_elf(
        elf(),
        &manifest("other-namespace", "ui.paint", UI_KEYS),
        Target::Host,
    );
    assert_eq!(
        load(&mut host, &outside, &GRANTED).map(|id| id.0),
        Err(UiPluginLoadError::Jaso(ErrorCode::TrustNamespace))
    );

    let raw = jaso::pack::from_elf(
        elf(),
        &manifest("ui-test-unsigned", "ui.paint", UI_KEYS),
        Arch::X86_64,
        Stage::Runtime,
    )
    .expect("pack");
    let unsigned = {
        let file = Jaso::parse(&raw).expect("parses");
        let mut builder = Builder::from_file(&file, 0).expect("image");
        builder.platform.target = Target::Host;
        builder.platform.float_abi = jaso::platform::FloatAbi::Hard;
        builder.finish().expect("unsigned")
    };
    assert_eq!(
        load(&mut host, &unsigned, &GRANTED).map(|id| id.0),
        Err(UiPluginLoadError::Jaso(ErrorCode::TrustUnsigned))
    );

    let publishers = [Publisher {
        name: PUBLISHER,
        namespace: NAMESPACE,
        key: signer().public_key(),
    }];
    let development = TrustPolicy::new(&publishers, TrustMode::Development).expect("policy");
    host.load(&unsigned, &development, &GRANTED)
        .expect("development admits unsigned");
    assert!(
        matches!(
            host.audit().last(),
            Some(UiPluginAudit::Loaded {
                publisher: None,
                ..
            })
        ),
        "and audits it"
    );
}

#[test]
fn ui_keys_are_a_versioned_gate() {
    let mut host = UiPluginHost::new();
    let cases = [
        (
            "ui-test-a",
            "ui.kind = paint-node\nui.paint = ui_paint_v1\n",
            UiManifestError::MissingAbi,
        ),
        (
            "ui-test-b",
            "ui.abi = 2\nui.kind = paint-node\nui.paint = ui_paint_v1\n",
            UiManifestError::UnsupportedAbi,
        ),
        (
            "ui-test-c",
            "ui.abi = 1\nui.kind = canvas\nui.paint = ui_paint_v1\n",
            UiManifestError::UnsupportedKind,
        ),
        (
            "ui-test-d",
            "ui.abi = 1\nui.kind = paint-node\n",
            UiManifestError::MissingEntry,
        ),
        (
            "ui-test-e",
            "ui.abi = 1\nui.kind = paint-node\nui.paint = jaso_init\n",
            UiManifestError::ReservedEntry,
        ),
        (
            "ui-test-f",
            "ui.abi = 1\nui.kind = paint-node\nui.paint = absent\n",
            UiManifestError::EntryNotExported,
        ),
    ];
    for (name, keys, expected) in cases {
        let bytes = pack_elf(elf(), &manifest(name, "ui.paint", keys), Target::Host);
        assert_eq!(
            load(&mut host, &bytes, &GRANTED).map(|id| id.0),
            Err(UiPluginLoadError::Manifest(expected)),
            "{name}"
        );
    }
}

#[test]
fn hostile_paint_output_is_refused_whole() {
    let mut host = UiPluginHost::new();
    let cases = [
        ("out-of-node", UiPaintError::OutOfNode(0)),
        ("negative-count", UiPaintError::Count(-5)),
        (
            "too-many",
            UiPaintError::Count(UI_PAINT_COMMAND_CAPACITY as i32 + 1),
        ),
    ];
    for (mode, expected) in cases {
        let bytes = pack_elf(
            &compile(mode),
            &manifest(&format!("ui-test-{mode}"), "ui.paint", UI_KEYS),
            Target::Host,
        );
        let id = load(&mut host, &bytes, &GRANTED).expect("load");
        assert_eq!(
            host.paint(id, 60, 40),
            Err(UiPluginPaintError::Output(expected)),
            "{mode}"
        );
    }
}

#[test]
fn failed_init_leaves_no_module() {
    let mut host = UiPluginHost::new();
    let bytes = pack_elf(
        &compile("init-fails"),
        &manifest("ui-test-init", "ui.paint", UI_KEYS),
        Target::Host,
    );
    assert_eq!(
        load(&mut host, &bytes, &GRANTED).map(|id| id.0),
        Err(UiPluginLoadError::Hook(9))
    );
    assert_eq!(
        host.unload_request(jasper_ui_engine::ui_plugin_host::UiPluginId(0)),
        None,
        "not in the table"
    );
    assert!(matches!(
        host.audit().last(),
        Some(UiPluginAudit::Rejected {
            error: UiPluginLoadError::Hook(9)
        })
    ));
}

#[test]
fn stop_and_unload_follow_the_beta_lifecycle() {
    let mut host = UiPluginHost::new();
    let bytes = plugin("ui-test-life");
    let id = load(&mut host, &bytes, &GRANTED).expect("load");
    assert_eq!(
        load(&mut host, &bytes, &GRANTED).map(|id| id.0),
        Err(UiPluginLoadError::DuplicateName)
    );
    assert_eq!(
        host.unload_request(id),
        Some(UnloadReply::Deferred),
        "beta never reports an unload"
    );
    assert!(
        host.paint(id, 60, 40).is_ok(),
        "deferred means still mapped and serving"
    );
    assert_eq!(host.stop(id), Ok(()));
    assert_eq!(host.paint(id, 60, 40), Err(UiPluginPaintError::NotRunning));
    assert_eq!(host.stop(id), Err(StopError::AlreadyStopped));
    assert_eq!(
        load(&mut host, &bytes, &GRANTED).map(|id| id.0),
        Err(UiPluginLoadError::DuplicateName),
        "a stopped name is not reused"
    );
}
