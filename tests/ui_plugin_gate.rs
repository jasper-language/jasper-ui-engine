//! Properties of the pure `ui.*` plugin rules and the surface render gate, against independent models.
//! `ui.*` 插件纯规则与 Surface 渲染门的属性，与独立模型对照。
use jasper_ui_engine::surface_lifecycle::SurfaceLifecycle;
use jasper_ui_engine::ui_plugin_gate::{
    admit_paint_output, node_pixel_box, paint_count_admits, paint_rect_admits, relocation_value,
    ui_manifest_admit, validate_paint_commands, UiManifestError, UiPaintCommandV1, UiPaintError,
    UI_PAINT_FILL_RECT,
};
use proptest::prelude::*;

fn command(x: u32, y: u32, width: u32, height: u32) -> UiPaintCommandV1 {
    UiPaintCommandV1 {
        kind: UI_PAINT_FILL_RECT,
        x,
        y,
        width,
        height,
        ..Default::default()
    }
}

#[test]
fn rect_boundaries_are_exact() {
    assert!(paint_rect_admits(0, 0, 60, 40, 60, 40));
    assert!(paint_rect_admits(59, 39, 1, 1, 60, 40));
    assert!(!paint_rect_admits(60, 0, 1, 1, 60, 40));
    assert!(!paint_rect_admits(0, 0, 61, 40, 60, 40));
    assert!(!paint_rect_admits(0, 0, 0, 40, 60, 40));
    assert!(!paint_rect_admits(0, 0, 60, 0, 60, 40));
    assert!(!paint_rect_admits(u32::MAX, 0, u32::MAX, 1, u32::MAX, 1));
    assert!(!paint_rect_admits(0, 0, 1, 1, 0, 0));
}

#[test]
fn count_and_relocation_boundaries_are_exact() {
    assert_eq!(paint_count_admits(-1, 64), None);
    assert_eq!(paint_count_admits(i32::MIN, 64), None);
    assert_eq!(paint_count_admits(0, 0), Some(0));
    assert_eq!(paint_count_admits(64, 64), Some(64));
    assert_eq!(paint_count_admits(65, 64), None);
    assert_eq!(relocation_value(0x1000, 4088, 4096, 4096), Some(0x2000));
    assert_eq!(relocation_value(0x1000, 4089, 0, 4096), None);
    assert_eq!(relocation_value(0x1000, 0, 4097, 4096), None);
    assert_eq!(relocation_value(u64::MAX, 0, 1, 4096), None);
    assert_eq!(relocation_value(0, 0, 0, 7), None);
}

#[test]
fn a_whole_output_is_refused_by_its_first_bad_command() {
    let good = command(0, 0, 10, 10);
    let malformed = UiPaintCommandV1 { kind: 9, ..good };
    let dirty = UiPaintCommandV1 {
        reserved: [0, 1],
        ..good
    };
    assert_eq!(
        validate_paint_commands(&[good, malformed, command(99, 0, 1, 1)], 10, 10),
        Err(UiPaintError::Malformed(1))
    );
    assert_eq!(
        validate_paint_commands(&[good, dirty], 10, 10),
        Err(UiPaintError::Malformed(1))
    );
    assert_eq!(
        validate_paint_commands(&[good, command(1, 0, 10, 10)], 10, 10),
        Err(UiPaintError::OutOfNode(1))
    );
    assert_eq!(
        admit_paint_output(1, &[good, malformed], 10, 10),
        Ok(&[good][..]),
        "commands past the count are not read"
    );
    assert_eq!(
        admit_paint_output(3, &[good, good], 10, 10),
        Err(UiPaintError::Count(3))
    );
    assert_eq!(
        admit_paint_output(-2, &[good, good], 10, 10),
        Err(UiPaintError::Count(-2))
    );
}

#[test]
fn manifest_gate_order_is_stable() {
    let admit = |abi, kind, entry, exports: &[&'static str], requires: &[&'static str]| {
        ui_manifest_admit(
            abi,
            kind,
            entry,
            exports.iter().copied(),
            requires.iter().copied(),
        )
    };
    let (e, r) = (["paint"].as_slice(), ["ui.paint"].as_slice());
    assert_eq!(
        admit(Some("1"), Some("paint-node"), Some("paint"), e, r),
        Ok("paint")
    );
    assert_eq!(
        admit(None, None, None, &[], &[]),
        Err(UiManifestError::MissingAbi)
    );
    assert_eq!(
        admit(Some("01"), None, None, &[], &[]),
        Err(UiManifestError::UnsupportedAbi)
    );
    assert_eq!(
        admit(Some("1"), None, None, &[], &[]),
        Err(UiManifestError::MissingKind)
    );
    assert_eq!(
        admit(Some("1"), Some("x"), None, &[], &[]),
        Err(UiManifestError::UnsupportedKind)
    );
    assert_eq!(
        admit(Some("1"), Some("paint-node"), None, e, r),
        Err(UiManifestError::MissingEntry)
    );
    assert_eq!(
        admit(
            Some("1"),
            Some("paint-node"),
            Some("jaso_stop"),
            &["jaso_stop"],
            r
        ),
        Err(UiManifestError::ReservedEntry)
    );
    assert_eq!(
        admit(Some("1"), Some("paint-node"), Some("other"), e, r),
        Err(UiManifestError::EntryNotExported)
    );
    assert_eq!(
        admit(Some("1"), Some("paint-node"), Some("paint"), e, &["log"]),
        Err(UiManifestError::CapabilityNotRequested)
    );
}

#[test]
fn pixel_box_rounds_both_edges_and_refuses_the_unrepresentable() {
    let snapped = node_pixel_box(0.5, 1.5, 10.0, 3.0, 2.0).expect("box");
    assert_eq!(
        (snapped.x, snapped.y, snapped.width, snapped.height),
        (1, 3, 20, 6)
    );
    assert!(node_pixel_box(0.0, 0.0, 1.0, 1.0, 0.0).is_none());
    assert!(node_pixel_box(0.0, 0.0, 1.0, 1.0, f64::NAN).is_none());
    assert!(node_pixel_box(f32::NAN, 0.0, 1.0, 1.0, 1.0).is_none());
    assert!(node_pixel_box(0.0, 0.0, -1.0, 1.0, 1.0).is_none());
    assert!(node_pixel_box(0.0, 0.0, f32::INFINITY, 1.0, 1.0).is_none());
    assert!(node_pixel_box(0.0, 0.0, 3e9, 1.0, 1.0).is_none());
}

#[test]
fn render_gate_follows_one_lifecycle() {
    let mut surface = SurfaceLifecycle::new(1, 8, 8);
    assert!(surface.render_allowed(1, 1));
    assert!(!surface.render_allowed(2, 1), "foreign caller");
    assert!(!surface.render_allowed(1, 2), "future generation");
    let suspended = surface.suspend(1, 1).expect("suspend");
    assert!(!surface.render_allowed(1, 1), "old generation");
    assert!(!surface.render_allowed(1, suspended), "not active");
    let resumed = surface.resume(1, suspended, 8, 8).expect("resume");
    assert!(surface.render_allowed(1, resumed));
    surface.retire(1, resumed).expect("retire");
    assert!(!surface.render_allowed(1, resumed), "retired");
}

// Persistence reads the working directory, which Miri isolation forbids; Miri runs the unit cases.
// 持久化要读工作目录，Miri 隔离禁止；Miri 运行上面的单元用例。
#[cfg(not(miri))]
proptest! {
    #[test]
    fn rect_matches_wide_arithmetic(x: u32, y: u32, w: u32, h: u32, nw: u32, nh: u32) {
        let inside = w > 0 && h > 0 && u64::from(x) + u64::from(w) <= u64::from(nw) && u64::from(y) + u64::from(h) <= u64::from(nh);
        prop_assert_eq!(paint_rect_admits(x, y, w, h, nw, nh), inside);
    }

    #[test]
    fn small_rect_matches_wide_arithmetic(x in 0u32..70, y in 0u32..50, w in 0u32..70, h in 0u32..50) {
        let inside = w > 0 && h > 0 && x + w <= 60 && y + h <= 40;
        prop_assert_eq!(paint_rect_admits(x, y, w, h, 60, 40), inside);
    }

    #[test]
    fn count_matches_wide_arithmetic(returned: i32, capacity: u32) {
        let expected = (returned >= 0 && i64::from(returned) <= i64::from(capacity)).then_some(returned as u32);
        prop_assert_eq!(paint_count_admits(returned, capacity), expected);
    }

    #[test]
    fn relocation_matches_wide_arithmetic(base: u64, offset: u32, addend in prop_oneof![any::<u64>(), 0u64..0x20000], size: u32) {
        let fits = u64::from(offset) + 8 <= u64::from(size) && addend <= u64::from(size);
        let wide = u128::from(base) + u128::from(addend);
        let expected = (fits && wide <= u128::from(u64::MAX)).then_some(wide as u64);
        prop_assert_eq!(relocation_value(base, offset, addend, size), expected);
    }

    #[test]
    fn an_admitted_output_is_a_prefix_of_commands_inside_the_node(
        returned in -3i32..12,
        raw in proptest::collection::vec((0u32..3, 0u32..80, 0u32..50, 0u32..80, 0u32..50, 0u32..2), 0..10),
    ) {
        let buffer: Vec<_> = raw.iter().map(|&(kind, x, y, width, height, reserved)| UiPaintCommandV1 {
            kind, x, y, width, height, reserved: [reserved, 0], rgba: [0; 4],
        }).collect();
        match admit_paint_output(returned, &buffer, 60, 40) {
            Ok(commands) => {
                prop_assert_eq!(commands.len() as i32, returned);
                prop_assert_eq!(commands, &buffer[..returned as usize]);
                for c in commands {
                    prop_assert_eq!(c.kind, UI_PAINT_FILL_RECT);
                    prop_assert_eq!(c.reserved, [0, 0]);
                    prop_assert!(c.width > 0 && c.height > 0 && c.x + c.width <= 60 && c.y + c.height <= 40);
                }
            }
            Err(UiPaintError::Count(n)) => {
                prop_assert_eq!(n, returned);
                prop_assert!(returned < 0 || returned as usize > buffer.len());
            }
            Err(UiPaintError::Malformed(i) | UiPaintError::OutOfNode(i)) => prop_assert!((i as i32) < returned),
        }
    }

    #[test]
    fn pixel_box_edges_stay_within_half_a_pixel(x in -1000.0f32..1000.0, y in -1000.0f32..1000.0, w in 0.0f32..1000.0, h in 0.0f32..1000.0, scale in 0.25f64..4.0) {
        let snapped = node_pixel_box(x, y, w, h, scale).expect("representable");
        let close = |edge: i64, exact: f64| (edge as f64 - exact).abs() <= 0.5 + 1e-9;
        prop_assert!(close(i64::from(snapped.x), f64::from(x) * scale));
        prop_assert!(close(i64::from(snapped.y), f64::from(y) * scale));
        prop_assert!(close(i64::from(snapped.x) + i64::from(snapped.width), (f64::from(x) + f64::from(w)) * scale));
        prop_assert!(close(i64::from(snapped.y) + i64::from(snapped.height), (f64::from(y) + f64::from(h)) * scale));
    }

    // Back-filled chain for existing code: only the owner's exact active generation may render.
    // 为现有代码补链：只有归属者的精确活动代数才能渲染。
    #[test]
    fn render_gate_admits_only_the_owners_live_generation(steps in proptest::collection::vec((0u8..4, 1u64..4, 0u64..8), 0..24)) {
        const OWNER: u64 = 1;
        let mut surface = SurfaceLifecycle::new(OWNER, 8, 8);
        let (mut generation, mut active, mut retired) = (1u64, true, false);
        for (op, caller, expected) in steps {
            let matches = caller == OWNER && expected == generation;
            let accepted = match op {
                0 => surface.reconfigure(caller, expected, 8, 8).is_ok(),
                1 => surface.suspend(caller, expected).is_ok(),
                2 => surface.resume(caller, expected, 8, 8).is_ok(),
                _ => surface.retire(caller, expected).is_ok(),
            };
            if accepted {
                prop_assert!(matches);
                generation += 1;
                active = op == 0 || op == 2;
                retired = op == 3;
            }
            for probe in 0..10u64 {
                for who in 1..4u64 {
                    let model = who == OWNER && active && !retired && probe == generation;
                    prop_assert_eq!(surface.render_allowed(who, probe), model);
                }
            }
        }
    }
}
