//! Verification-only adapter around exact production byte-reader bodies.
//! 验证专用适配层；运行体从生产源码原样提取，不进入 UI 发行依赖。
include!(concat!(env!("OUT_DIR"), "/production.rs"));

pub fn endpoint(total: usize, pos: usize, len: usize) -> Option<usize> {
    checked_read_end(total, pos, len)
}

pub fn read<'a>(bytes: &'a [u8], pos: &mut usize, len: usize) -> Option<&'a [u8]> {
    take_bytes(bytes, pos, len)
}

// Independent widened-integer oracle: no production subtraction or branching.
// 独立宽整数规格，不重复生产实现的减法及分支结构。
pub fn assert_endpoint(total: usize, pos: usize, len: usize) {
    let sum = pos as u128 + len as u128;
    let expected = if pos <= total && sum <= total as u128 {
        Some(sum as usize)
    } else {
        None
    };
    assert_eq!(endpoint(total, pos, len), expected);
}

pub fn assert_read(bytes: &[u8], start: usize, len: usize) {
    let mut pos = start;
    let result = read(bytes, &mut pos, len);
    let end = start as u128 + len as u128;
    if start <= bytes.len() && end <= bytes.len() as u128 {
        assert_eq!(pos as u128, end);
        assert_eq!(result.unwrap(), &bytes[start..end as usize]);
    } else {
        assert!(result.is_none());
        assert_eq!(pos, start);
    }
}

/// Independent arithmetic oracle for host-supplied packed-row budgets.
/// 宿主提供的紧密行额度对应的独立算术规格。
pub fn assert_surface_row(width: u32, budget: usize) {
    let exact = width as u128 * 4;
    let expected = if width > 0 && exact <= budget as u128 {
        Some(exact as usize)
    } else {
        None
    };
    assert_eq!(rgba8_row_bytes(width, budget), expected);
}

/// Full-width oracle uses u128 so multiplication and addition do not wrap.
/// 使用 u128 独立计算规格，避免将 usize 运算回绕误认为正确结果。
pub fn assert_surface_span(row: usize, height: usize, stride: usize, capacity: usize) {
    let end = height.saturating_sub(1) as u128 * stride as u128 + row as u128;
    let expected = if row > 0 && height > 0 && stride >= row && end <= capacity as u128 {
        Some(end as usize)
    } else {
        None
    };
    assert_eq!(surface_span(row, height, stride, capacity), expected);
}

/// Check real production version and admission bodies against their contracts.
/// 对实际版本与准入函数检查契约，不扩展为整个场景事务的证明。
pub fn assert_versions(current: u64, expected: u64, empty: bool) {
    let increment = current as u128 + 1;
    assert_eq!(
        next_scene_version(current),
        if increment <= u64::MAX as u128 {
            Some(increment as u64)
        } else {
            None
        }
    );
    match decide_scene_version(current, expected, empty) {
        VersionDecision::Stale => assert_ne!(current, expected),
        VersionDecision::Unchanged => {
            assert_eq!(current, expected);
            assert!(empty);
        }
        VersionDecision::Exhausted => {
            assert_eq!(current, expected);
            assert!(!empty);
            assert_eq!(current, u64::MAX);
        }
        VersionDecision::Advance(next) => {
            assert_eq!(current, expected);
            assert!(!empty);
            assert_eq!(next as u128, increment);
        }
    }
}

/// Cursor failure is specifically Truncated, with no position change.
/// 游标读取失败必须准确返回 Truncated，并保持位置。
pub fn assert_cursor(bytes: &[u8], start: usize, len: usize) {
    let mut cursor = Cursor { bytes, pos: start };
    let result = cursor.take(len);
    let end = start as u128 + len as u128;
    if start <= bytes.len() && end <= bytes.len() as u128 {
        assert_eq!(cursor.pos as u128, end);
        assert_eq!(result.unwrap(), &bytes[start..end as usize]);
    } else {
        assert_eq!(result, Err(BatchError::Truncated));
        assert_eq!(cursor.pos, start);
    }
}

/// Check complete declared span, exact active bytes and invalid-row rejection.
/// 检查整个声明跨度、精确有效行字节，以及非法行的拒绝。
pub fn assert_borrow(bytes: &[u8], row_bytes: usize, height: usize, stride: usize, row: usize) {
    let end = height.saturating_sub(1) as u128 * stride as u128 + row_bytes as u128;
    let valid = row_bytes > 0
        && height > 0
        && stride >= row_bytes
        && end <= bytes.len() as u128
        && row < height;
    let result = surface_row(bytes, row_bytes, height, stride, row);
    if valid {
        let start = row as u128 * stride as u128;
        assert_eq!(
            result.unwrap(),
            &bytes[start as usize..(start + row_bytes as u128) as usize]
        );
    } else {
        assert_eq!(result, None);
    }
}

/// Check production u8/u16/u32 readers with an independent byte-shift oracle.
/// 使用独立按字节移位的规格检查实际 u8/u16/u32 读取器。
pub fn assert_scalars(bytes: &[u8], start: usize) {
    for width in [1usize, 2, 4] {
        let mut cursor = Cursor { bytes, pos: start };
        let result = match width {
            1 => cursor.u8().map(|value| value as u64),
            2 => cursor.u16().map(|value| value as u64),
            _ => cursor.u32().map(|value| value as u64),
        };
        let end = start as u128 + width as u128;
        if end <= bytes.len() as u128 {
            let mut expected = 0u64;
            for lane in 0..width {
                expected |= (bytes[start + lane] as u64) << (8 * lane);
            }
            assert_eq!(result, Ok(expected));
            assert_eq!(cursor.pos as u128, end);
        } else {
            assert_eq!(result, Err(BatchError::Truncated));
            assert_eq!(cursor.pos, start);
        }
    }
}

/// Compare the whole output, including padding and rows outside the write.
/// 对比完整输出，覆盖 padding 与目标行之外的所有字节。
pub fn assert_write(
    bytes: &[u8],
    row_bytes: usize,
    height: usize,
    stride: usize,
    row: usize,
    source: &[u8],
) {
    let mut actual = bytes.to_vec();
    let end = height.saturating_sub(1) as u128 * stride as u128 + row_bytes as u128;
    let valid = row_bytes > 0
        && height > 0
        && stride >= row_bytes
        && row < height
        && end <= bytes.len() as u128
        && source.len() == row_bytes;
    let accepted = write_surface_row(&mut actual, row_bytes, height, stride, row, source);
    assert_eq!(accepted, valid);
    for i in 0..bytes.len() {
        let start = row as u128 * stride as u128;
        let inside = valid && start <= i as u128 && (i as u128) < start + row_bytes as u128;
        let expected = if inside {
            source[(i as u128 - start) as usize]
        } else {
            bytes[i]
        };
        assert_eq!(actual[i], expected);
    }
}

/// Independent protocol-v1 header oracle, including diagnostic precedence.
/// 独立的 v1 协议头规格，包含诊断优先级。
pub fn assert_header(bytes: &[u8]) {
    let expected = if bytes.len() < 10 {
        Err(BatchError::Truncated)
    } else if &bytes[..4] != b"JUEB" {
        Err(BatchError::BadMagic)
    } else {
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        let count = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
        if version != 1 {
            Err(BatchError::UnsupportedVersion(version))
        } else if count > 1_000_000 {
            Err(BatchError::TooLarge)
        } else if count as usize > bytes.len() - 10 {
            Err(BatchError::Truncated)
        } else {
            Ok(count)
        }
    };
    assert_eq!(decode_header(bytes), expected);
}

/// Check the public view API without using its private layout fields.
/// 仅通过公开视图 API 检查构造、元数据与行访问，不依赖私有字段。
pub fn assert_view(bytes: &[u8], width: u32, height: usize, stride: usize, row: usize) {
    let row_bytes = width as u128 * 4;
    let end = height.saturating_sub(1) as u128 * stride as u128 + row_bytes;
    let valid =
        width > 0 && height > 0 && stride as u128 >= row_bytes && end <= bytes.len() as u128;
    let result = Rgba8SurfaceView::new(bytes, width, height, stride);
    assert_eq!(result.is_some(), valid);
    if let Some(view) = result {
        assert_eq!(view.width(), width);
        assert_eq!(view.height(), height);
        assert_eq!(view.stride(), stride);
        for index in [row, 0, height - 1, height] {
            let slice = view.row(index);
            if index < height {
                let start = index as u128 * stride as u128;
                assert_eq!(
                    slice.unwrap(),
                    &bytes[start as usize..(start + row_bytes) as usize]
                );
            } else {
                assert_eq!(slice, None);
            }
        }
    }
}

/// Verify prefix consumption, payload bytes and exact error precedence/state.
/// 检查前缀消费、载荷字节及错误优先级/精确游标状态。
pub fn assert_payload(bytes: &[u8], start: usize, limit: u32) {
    let mut cursor = Cursor { bytes, pos: start };
    let result = cursor.bounded_bytes(limit);
    let prefix_end = start as u128 + 4;
    if prefix_end > bytes.len() as u128 {
        assert_eq!(result, Err(BatchError::Truncated));
        assert_eq!(cursor.pos, start);
        return;
    }
    let length = u32::from_le_bytes([
        bytes[start],
        bytes[start + 1],
        bytes[start + 2],
        bytes[start + 3],
    ]);
    let end = prefix_end + length as u128;
    if length > limit {
        assert_eq!(result, Err(BatchError::TooLarge));
        assert_eq!(cursor.pos as u128, prefix_end);
    } else if end > bytes.len() as u128 {
        assert_eq!(result, Err(BatchError::Truncated));
        assert_eq!(cursor.pos as u128, prefix_end);
    } else {
        assert_eq!(result.unwrap(), &bytes[prefix_end as usize..end as usize]);
        assert_eq!(cursor.pos as u128, end);
    }
}

/// Exercise checked view-to-buffer transfer without relying on private fields.
/// 不依赖私有字段，检查受检视图到调用方缓冲的传输。
pub fn assert_view_copy(
    bytes: &[u8],
    width: u32,
    height: usize,
    stride: usize,
    row: usize,
    destination: &[u8],
) {
    let Some(view) = Rgba8SurfaceView::new(bytes, width, height, stride) else {
        return;
    };
    let mut actual = destination.to_vec();
    let row_bytes = width as u128 * 4;
    let expected_success = row < height && destination.len() as u128 == row_bytes;
    assert_eq!(view.copy_row_to(row, &mut actual), expected_success);
    if expected_success {
        let start = row as u128 * stride as u128;
        assert_eq!(actual, &bytes[start as usize..(start + row_bytes) as usize]);
    } else {
        assert_eq!(actual, destination);
    }
}

/// Compare arithmetic production packing with the established channel bit lanes.
/// 用独立位通道规格对照生产算术编码。
pub fn assert_rgb_word(red: u8, green: u8, blue: u8) {
    let expected = ((red as u32) << 16) | ((green as u32) << 8) | blue as u32;
    let actual = rgb8_word(red, green, blue);
    assert_eq!(actual, expected);
    assert_eq!(actual >> 24, 0);
}

/// Whole conversion: exact words on success, entire original output on failure.
/// 整体转换：成功逐字精确；失败整个目标保持原值。
pub fn assert_pixel_conversion(pixels: &[u8], initial: &[u32]) {
    let mut actual = initial.to_vec();
    let result = rgba_to_u32(pixels, &mut actual);
    if pixels.len() as u128 != initial.len() as u128 * 4 {
        assert_eq!(result, Err(PixelError::LengthMismatch));
        assert_eq!(actual, initial);
    } else {
        assert_eq!(result, Ok(()));
        assert_eq!(actual.len(), initial.len());
        for i in 0..actual.len() {
            let start = i * 4;
            let expected = ((pixels[start] as u32) << 16)
                | ((pixels[start + 1] as u32) << 8)
                | pixels[start + 2] as u32;
            assert_eq!(actual[i], expected);
        }
    }
}

/// Independent wide-integer oracle for owned Canvas import and existing getters.
/// 用宽整数独立校验 Canvas 所有权导入与既有只读访问器。
pub fn assert_canvas_import(bytes: &[u8], width: u32, height: u32) {
    let owned = bytes.to_vec();
    let pointer = owned.as_ptr();
    let capacity = owned.capacity();
    let valid =
        width > 0 && height > 0 && width as u128 * height as u128 * 4 == bytes.len() as u128;
    match Canvas::from_rgba8(width, height, owned) {
        Ok(canvas) => {
            assert!(valid);
            assert_eq!(canvas.width(), width);
            assert_eq!(canvas.height(), height);
            assert_eq!(canvas.pixels(), bytes);
            assert_eq!(canvas.pixels().as_ptr(), pointer);
            assert_eq!(canvas.pixels.capacity(), capacity);
            assert!(canvas.clip_stack.is_empty());
        }
        Err(returned) => {
            assert!(!valid);
            assert_eq!(returned, bytes);
            assert_eq!(returned.as_ptr(), pointer);
            assert_eq!(returned.capacity(), capacity);
        }
    }
}

/// Check exact resource transitions against an independent state tuple oracle.
/// 用独立状态元组规格校验资源准入、撤销和再次签发。
pub fn assert_resource(
    session: u64,
    slot: u64,
    generation: u64,
    live: bool,
    caller: u64,
    key: ResourceKey,
) {
    let fresh = ResourceGate::new(session, slot);
    assert_eq!(
        (fresh.session, fresh.slot, fresh.generation, fresh.live),
        (session, slot, 0, false)
    );
    let mut gate = ResourceGate {
        session,
        slot,
        generation,
        live,
    };
    let allowed = live
        && generation > 0
        && caller == session
        && (key.session, key.slot, key.generation) == (session, slot, generation);
    assert_eq!(gate.allows(caller, key), allowed);
    assert_eq!(gate.revoke(caller, key), allowed);
    let after_live = live && !allowed;
    assert_eq!(
        (gate.session, gate.slot, gate.generation, gate.live),
        (session, slot, generation, after_live)
    );
    assert!(!gate.allows(caller, key));
    let expected =
        if caller == session && !after_live && (generation as u128 + 1) <= u64::MAX as u128 {
            Some(ResourceKey {
                session,
                slot,
                generation: (generation as u128 + 1) as u64,
            })
        } else {
            None
        };
    assert_eq!(gate.issue(caller), expected);
    match expected {
        Some(issued) => {
            assert_eq!(
                (gate.session, gate.slot, gate.generation, gate.live),
                (session, slot, issued.generation, true)
            );
            assert!(gate.allows(caller, issued));
            // Arbitrary wire keys may predict the next generation; identity is not secret.
            // 任意线上 key 可能恰好等于下一代身份，不能将其误认成旧代 key。
            assert_eq!(gate.allows(caller, key), key == issued);
            assert!(!gate.allows(caller ^ 1, issued));
            assert_eq!(gate.issue(caller), None);
            assert!(gate.revoke(caller, issued));
            assert!(!gate.revoke(caller, issued));
        }
        None => assert_eq!(
            (gate.session, gate.slot, gate.generation, gate.live),
            (session, slot, generation, after_live)
        ),
    }
    gate.retire();
    assert_eq!(
        (gate.session, gate.slot, gate.generation, gate.live),
        (session, slot, u64::MAX, false)
    );
    assert_eq!(gate.issue(session), None);
    assert!(!gate.allows(session, key));
    assert!(!gate.revoke(session, key));
    gate.retire();
    assert_eq!((gate.generation, gate.live), (u64::MAX, false));
}

pub fn assert_role(code: u8) {
    let roles = [
        Role::Container,
        Role::Text,
        Role::Button,
        Role::TextField,
        Role::Checkbox,
        Role::Radio,
        Role::Switch,
        Role::Slider,
        Role::Image,
        Role::Link,
        Role::List,
        Role::ListItem,
        Role::Dialog,
        Role::Menu,
        Role::MenuItem,
        Role::ScrollView,
        Role::Unknown,
    ];
    let index = code as usize % roles.len();
    assert_eq!(
        roles[index].is_activatable(),
        matches!(index, 2 | 4 | 5 | 6 | 9 | 14)
    );
    assert_eq!(
        roles[index].activates_with_space(),
        matches!(index, 2 | 4 | 5 | 6 | 14)
    );
}

pub fn assert_bytes_slot(bytes: &[u8], session: u64, slot_id: u64, caller: u64) {
    let mut slot = UiBytesSlot::new(session, slot_id);
    let owned = bytes.to_vec();
    let pointer = owned.as_ptr();
    let capacity = owned.capacity();
    let returned = slot.install(session ^ 1, owned).unwrap_err();
    assert_eq!(returned, bytes);
    assert_eq!(returned.as_ptr(), pointer);
    let key = slot.install(session, returned).unwrap();
    assert_eq!(slot.bytes(session, key), Some(bytes));
    assert_eq!(slot.bytes(session, key).unwrap().as_ptr(), pointer);
    assert_eq!(slot.install(session, vec![99]), Err(vec![99]));
    assert_eq!(
        slot.bytes(caller, key),
        if caller == session { Some(bytes) } else { None }
    );
    let wrong_slot = ResourceKey {
        slot: slot_id ^ 1,
        ..key
    };
    assert_eq!(slot.bytes(session, wrong_slot), None);
    assert_eq!(slot.release(session, wrong_slot), None);
    assert_eq!(slot.release(session ^ 1, key), None);
    assert_eq!(slot.bytes(session, key), Some(bytes));
    let released = slot.release(session, key).unwrap();
    assert_eq!(released, bytes);
    assert_eq!(released.as_ptr(), pointer);
    assert_eq!(released.capacity(), capacity);
    assert_eq!(slot.bytes(session, key), None);
    assert_eq!(slot.release(session, key), None);
    let next = slot.install(session, vec![42]).unwrap();
    assert_eq!(next.generation, 2);
    assert_eq!(slot.bytes(session, key), None);
    assert_eq!(slot.release(session, key), None);
    assert_eq!(slot.bytes(session, next), Some(&[42][..]));
    assert_eq!(slot.release(session, next), Some(vec![42]));
    let final_payload = bytes.to_vec();
    let final_pointer = final_payload.as_ptr();
    let final_key = slot.install(session, final_payload).unwrap();
    let retired_payload = slot.retire().unwrap();
    assert_eq!(retired_payload, bytes);
    assert_eq!(retired_payload.as_ptr(), final_pointer);
    assert_eq!(slot.bytes(session, final_key), None);
    assert_eq!(slot.release(session, final_key), None);
    assert_eq!(slot.install(session, vec![99]), Err(vec![99]));
    assert_eq!(slot.retire(), None);
    let mut empty = UiBytesSlot::new(session, slot_id ^ 1);
    assert_eq!(empty.retire(), None);
    assert_eq!(empty.install(session, vec![99]), Err(vec![99]));
}

pub fn assert_command_modifier(flags: u8) {
    let event = KeyEvent {
        key: if flags & 16 == 0 { "Enter" } else { "text" }.into(),
        phase: if flags & 32 == 0 {
            KeyPhase::Down
        } else {
            KeyPhase::Up
        },
        repeat: flags & 64 != 0,
        modifiers: Modifiers {
            shift: flags & 1 != 0,
            control: flags & 2 != 0,
            alt: flags & 4 != 0,
            meta: flags & 8 != 0,
        },
    };
    assert_eq!(no_command_modifier(&event), flags & 14 == 0);
}

pub fn assert_text_classes(ch: char) {
    // Independent interval-table oracle; these are legacy approximation ranges,
    // not a claim of Unicode-complete typography or grapheme segmentation.
    // 独立区间表规格；仅描述旧近似范围，不代表完整 Unicode 排版/字素切分。
    let wide = [
        (4352, 4447),
        (11904, 42191),
        (44032, 55203),
        (63744, 64255),
        (65072, 65103),
        (65280, 65376),
        (65504, 65510),
        (127744, 129791),
    ];
    let zero = [
        (768, 879),
        (6832, 6911),
        (7616, 7679),
        (8203, 8205),
        (8400, 8447),
        (65024, 65039),
        (65056, 65071),
        (917760, 917999),
    ];
    let n = u32::from(ch);
    assert_eq!(
        is_wide(ch),
        wide.iter().any(|&(start, end)| (start..=end).contains(&n))
    );
    assert_eq!(
        is_zero_width(ch),
        zero.iter().any(|&(start, end)| (start..=end).contains(&n))
    );
}

pub fn assert_focus_cleanup(order: &[u64], focused: Option<u64>) {
    let focus = FocusManager {
        order: order.iter().copied().map(NodeId).collect(),
        focused: focused.map(NodeId),
    };
    assert_focus_state(focus, order, focused);
}

pub fn assert_focus_state(mut focus: FocusManager, order: &[u64], focused: Option<u64>) {
    let pointer = focus.order().as_ptr();
    assert_eq!(focus.focused(), focused.map(NodeId));
    assert_eq!(focus.order().len(), order.len());
    for (index, expected) in order.iter().enumerate() {
        assert_eq!(focus.order()[index].0, *expected);
    }
    focus.blur();
    assert_eq!(focus.focused(), None);
    assert_eq!(focus.order().len(), order.len());
    for (index, expected) in order.iter().enumerate() {
        assert_eq!(focus.order()[index].0, *expected);
    }
    assert_eq!(focus.order().as_ptr(), pointer);
    focus.blur();
    assert_eq!(focus.focused(), None);
}

/// Exercise real delegates and focus extraction, preserving order and allocation.
/// 调用真实委托和焦点取回，检查顺序及原分配保持。
pub fn assert_keyboard_focus(order: &[u64], focused: Option<u64>) {
    let focus = FocusManager {
        order: order.iter().copied().map(NodeId).collect(),
        focused: focused.map(NodeId),
    };
    assert_keyboard_state(focus, order, focused);
}

pub fn assert_keyboard_state(focus: FocusManager, order: &[u64], focused: Option<u64>) {
    let mut router = KeyboardRouter { focus };
    let pointer = router.focus.order().as_ptr();
    assert_eq!(router.focused(), focused.map(NodeId));
    assert_eq!(router.take_focus(), focused.map(NodeId));
    assert_eq!(router.focused(), None);
    assert_eq!(router.take_focus(), None);
    router.blur();
    assert_eq!(router.focused(), None);
    assert_eq!(router.focus.order().len(), order.len());
    for (index, expected) in order.iter().enumerate() {
        assert_eq!(router.focus.order()[index].0, *expected);
    }
    assert_eq!(router.focus.order().as_ptr(), pointer);
}

/// Actual focus selection against an independent native slice membership oracle.
/// 真实焦点选择，对照独立的原生切片成员判定。
pub fn assert_focus_selection(order: &[u64], focused: Option<u64>, target: u64) {
    let manager = FocusManager {
        order: order.iter().copied().map(NodeId).collect(),
        focused: focused.map(NodeId),
    };
    assert_focus_selection_state(manager, order, focused, target);
}

pub fn assert_focus_selection_state(
    mut manager: FocusManager,
    order: &[u64],
    focused: Option<u64>,
    target: u64,
) {
    let expected = order.contains(&target);
    let result = manager.focus(NodeId(target));
    if expected {
        assert_eq!(result, Ok(()));
        assert_eq!(manager.focused(), Some(NodeId(target)));
    } else {
        assert_eq!(result, Err(FocusError::NotFocusable(NodeId(target))));
        assert_eq!(manager.focused(), focused.map(NodeId));
    }
    assert_eq!(manager.order().len(), order.len());
    for (index, id) in order.iter().enumerate() {
        assert_eq!(manager.order()[index].0, *id);
    }
}

/// Retire the actual gate and router, including already vacant/exhausted states.
/// 对真实准入及路由器退役，含已空闲和代数耗尽状态。
pub fn assert_input_retire(
    order: &[u64],
    focused: Option<u64>,
    session: u64,
    slot: u64,
    generation: u64,
    live: bool,
) {
    let focus = FocusManager {
        order: order.iter().copied().map(NodeId).collect(),
        focused: focused.map(NodeId),
    };
    assert_input_retire_state(focus, order, focused, session, slot, generation, live);
}

pub fn assert_input_retire_state(
    focus: FocusManager,
    order: &[u64],
    focused: Option<u64>,
    session: u64,
    slot: u64,
    generation: u64,
    live: bool,
) {
    let mut router = KeyboardRouter { focus };
    let mut gate = ResourceGate {
        session,
        slot,
        generation,
        live,
    };
    let old = ResourceKey {
        session,
        slot,
        generation,
    };
    assert_eq!(router.retire_focus(&mut gate), focused.map(NodeId));
    assert_eq!(router.focused(), None);
    assert_eq!(
        (gate.session, gate.slot, gate.generation, gate.live),
        (session, slot, u64::MAX, false)
    );
    assert!(!gate.allows(session, old));
    assert_eq!(gate.issue(session), None);
    assert_eq!(router.retire_focus(&mut gate), None);
    assert_eq!(router.focus.order().len(), order.len());
    for (index, id) in order.iter().enumerate() {
        assert_eq!(router.focus.order()[index].0, *id);
    }
}

/// Compare real hover transitions and retirement with an independent case table.
/// 真实悬停转移及退役，对照独立情况表。
pub fn assert_hover_lifecycle(
    old: Option<u64>,
    target: Option<u64>,
    session: u64,
    slot: u64,
    generation: u64,
    live: bool,
) {
    let mut hover = HoverTracker {
        current: old.map(NodeId),
    };
    assert_eq!(hover.hovered(), old.map(NodeId));
    let expected = match (old, target) {
        (None, None) => vec![],
        (Some(a), Some(b)) if a == b => vec![],
        (None, Some(b)) => vec![HoverChange::Enter(NodeId(b))],
        (Some(a), None) => vec![HoverChange::Leave(NodeId(a))],
        (Some(a), Some(b)) => vec![HoverChange::Leave(NodeId(a)), HoverChange::Enter(NodeId(b))],
    };
    assert_eq!(hover.update(target.map(NodeId)), expected);
    assert_eq!(hover.hovered(), target.map(NodeId));
    assert!(hover.update(target.map(NodeId)).is_empty());
    let mut gate = ResourceGate {
        session,
        slot,
        generation,
        live,
    };
    let key = ResourceKey {
        session,
        slot,
        generation,
    };
    let leave = match target {
        None => vec![],
        Some(id) => vec![HoverChange::Leave(NodeId(id))],
    };
    assert_eq!(hover.retire_hover(&mut gate), leave);
    assert_eq!(hover.hovered(), None);
    assert_eq!(
        (gate.session, gate.slot, gate.generation, gate.live),
        (session, slot, u64::MAX, false)
    );
    assert!(!gate.allows(session, key));
    assert_eq!(gate.issue(session), None);
    assert!(hover.retire_hover(&mut gate).is_empty());
    assert!(hover.clear().is_empty());
}

/// Primitive integer comparisons are independent of the wrapper implementations.
/// 以原生整数比较对照实际指针身份 trait 实现。
pub fn assert_pointer_order(a: u64, b: u64, c: u64) {
    let x = PointerId(a);
    let y = PointerId(b);
    let z = PointerId(c);
    assert_eq!(x == y, a == b);
    assert_eq!(x != y, a != b);
    assert_eq!(x.cmp(&y), a.cmp(&b));
    assert_eq!(x.partial_cmp(&y), a.partial_cmp(&b));
    assert_eq!(x < y, a < b);
    assert_eq!(x <= y, a <= b);
    if x < y && y < z {
        assert!(x < z);
    }
    assert_eq!(x.cmp(&y), y.cmp(&x).reverse());
}

/// Actual map queries, including duplicate-key replacement and outside captures.
/// 真实 map 查询，涵盖重复 key 替换及起点位于所有节点之外的捕获。
pub fn assert_capture_query(
    first: u64,
    second: u64,
    query: u64,
    first_target: Option<u64>,
    second_target: Option<u64>,
) {
    let router = PointerRouter {
        tracker: PointerTracker {
            active: BTreeMap::new(),
        },
        captures: BTreeMap::new(),
    };
    assert!(!router.has_capture(PointerId(query)));
    assert_eq!(router.captured_target(PointerId(query)), None);
    let router = PointerRouter {
        tracker: PointerTracker {
            active: BTreeMap::new(),
        },
        captures: BTreeMap::from([
            (
                PointerId(first),
                Capture {
                    target: first_target.map(NodeId),
                    path: Vec::new(),
                },
            ),
            (
                PointerId(second),
                Capture {
                    target: second_target.map(NodeId),
                    path: Vec::new(),
                },
            ),
        ]),
    };
    let expected = if query == second {
        second_target
    } else if query == first {
        first_target
    } else {
        None
    };
    assert_eq!(
        router.has_capture(PointerId(query)),
        query == first || query == second
    );
    assert_eq!(
        router.captured_target(PointerId(query)),
        expected.map(NodeId)
    );
    assert_eq!(router.captures.len(), if first == second { 1 } else { 2 });
}

#[cfg(kani)]
mod kani_checks {
    use super::*;
    #[kani::proof]
    #[kani::unwind(17)]
    fn pointer_order() {
        assert_pointer_order(kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn capture_query() {
        // Representative real-map layout: empty / one key. Query and stored
        // target remain arbitrary; arbitrary maps are covered by Verus.
        // 代表性真实map布局：空表/单键；查询和目标任意，任意map由Verus覆盖。
        let query: u64 = kani::any();
        let target: Option<u64> = kani::any();
        let empty = PointerRouter {
            tracker: PointerTracker {
                active: BTreeMap::new(),
            },
            captures: BTreeMap::new(),
        };
        assert!(!empty.has_capture(PointerId(query)));
        assert_eq!(empty.captured_target(PointerId(query)), None);
        std::mem::forget(empty);
        let mut captures = BTreeMap::new();
        captures.insert(
            PointerId(7),
            Capture {
                target: target.map(NodeId),
                path: Vec::new(),
            },
        );
        let router = PointerRouter {
            tracker: PointerTracker {
                active: BTreeMap::new(),
            },
            captures,
        };
        assert_eq!(router.has_capture(PointerId(query)), query == 7);
        assert_eq!(
            router.captured_target(PointerId(query)),
            if query == 7 { target.map(NodeId) } else { None }
        );
        // The production queries borrow &self and do not destroy the map.
        // 生产查询只借用&self，不负责map析构；其析构由原生/Miri另验。
        std::mem::forget(router);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn hover_lifecycle() {
        assert_hover_lifecycle(
            kani::any(),
            kani::any(),
            kani::any(),
            kani::any(),
            kani::any(),
            kani::any(),
        );
    }

    #[kani::proof]
    #[kani::unwind(17)]
    fn focus_selection() {
        let order: [u64; 4] = kani::any();
        let len: u8 = kani::any();
        kani::assume(len <= 4);
        let focused: Option<u64> = kani::any();
        let mut nodes = vec![
            NodeId(order[0]),
            NodeId(order[1]),
            NodeId(order[2]),
            NodeId(order[3]),
        ];
        nodes.truncate(len as usize);
        let focus = FocusManager {
            order: nodes,
            focused: focused.map(NodeId),
        };
        assert_focus_selection_state(focus, &order[..len as usize], focused, kani::any());
    }

    #[kani::proof]
    #[kani::unwind(17)]
    fn input_retirement() {
        let order: [u64; 4] = kani::any();
        let len: u8 = kani::any();
        kani::assume(len <= 4);
        let focused: Option<u64> = kani::any();
        let mut nodes = vec![
            NodeId(order[0]),
            NodeId(order[1]),
            NodeId(order[2]),
            NodeId(order[3]),
        ];
        nodes.truncate(len as usize);
        let focus = FocusManager {
            order: nodes,
            focused: focused.map(NodeId),
        };
        assert_input_retire_state(
            focus,
            &order[..len as usize],
            focused,
            kani::any(),
            kani::any(),
            kani::any(),
            kani::any(),
        );
    }

    #[kani::proof]
    #[kani::unwind(17)]
    fn keyboard_focus() {
        let order: [u64; 4] = kani::any();
        let len: u8 = kani::any();
        kani::assume(len <= 4);
        let focused: Option<u64> = kani::any();
        let mut nodes = vec![
            NodeId(order[0]),
            NodeId(order[1]),
            NodeId(order[2]),
            NodeId(order[3]),
        ];
        nodes.truncate(len as usize);
        let focus = FocusManager {
            order: nodes,
            focused: focused.map(NodeId),
        };
        assert_keyboard_state(focus, &order[..len as usize], focused);
    }

    #[kani::proof]
    #[kani::unwind(17)]
    fn focus_cleanup() {
        let order: [u64; 4] = kani::any();
        let len: u8 = kani::any();
        kani::assume(len <= 4);
        let focused: Option<u64> = kani::any();
        let mut nodes = vec![
            NodeId(order[0]),
            NodeId(order[1]),
            NodeId(order[2]),
            NodeId(order[3]),
        ];
        nodes.truncate(len as usize);
        let focus = FocusManager {
            order: nodes,
            focused: focused.map(NodeId),
        };
        assert_focus_state(focus, &order[..len as usize], focused);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn text_classification() {
        assert_text_classes(kani::any());
    }

    #[kani::proof]
    #[kani::unwind(17)]
    fn bytes_resource_lifecycle() {
        let bytes: [u8; 8] = kani::any();
        let len: u8 = kani::any();
        kani::assume(len <= 8);
        assert_bytes_slot(
            &bytes[..len as usize],
            kani::any(),
            kani::any(),
            kani::any(),
        );
    }
    #[kani::proof]
    #[kani::unwind(17)]
    fn command_modifier_predicate() {
        assert_command_modifier(kani::any());
    }

    #[kani::proof]
    fn resource_transitions() {
        let key = ResourceKey {
            session: kani::any(),
            slot: kani::any(),
            generation: kani::any(),
        };
        assert_resource(
            kani::any(),
            kani::any(),
            kani::any(),
            kani::any(),
            kani::any(),
            key,
        );
    }

    #[kani::proof]
    #[kani::unwind(18)]
    fn role_classifiers() {
        assert_role(kani::any());
    }

    #[kani::proof]
    #[kani::unwind(17)]
    fn owned_canvas_import() {
        let bytes: [u8; 16] = kani::any();
        let len: u8 = kani::any();
        kani::assume(len <= 16);
        assert_canvas_import(&bytes[..len as usize], kani::any(), kani::any());
    }

    #[kani::proof]
    fn all_rgb_words() {
        assert_rgb_word(kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    #[kani::unwind(9)]
    fn two_pixel_conversion() {
        let pixels: [u8; 8] = kani::any();
        let words: [u32; 2] = kani::any();
        let pixel_len: u8 = kani::any();
        let word_len: u8 = kani::any();
        kani::assume(pixel_len <= 8 && word_len <= 2);
        assert_pixel_conversion(&pixels[..pixel_len as usize], &words[..word_len as usize]);
    }

    #[kani::proof]
    #[kani::unwind(9)]
    fn eight_byte_payload() {
        let bytes: [u8; 8] = kani::any();
        let len: u8 = kani::any();
        kani::assume(len <= 8);
        assert_payload(&bytes[..len as usize], kani::any(), kani::any());
    }

    #[kani::proof]
    #[kani::unwind(17)]
    fn sixteen_byte_view_transfer() {
        let bytes: [u8; 16] = kani::any();
        let destination: [u8; 8] = kani::any();
        let width: u8 = kani::any();
        let height: u8 = kani::any();
        let stride: u8 = kani::any();
        assert_view_copy(
            &bytes,
            width as u32,
            height as usize,
            stride as usize,
            kani::any(),
            &destination,
        );
    }

    #[kani::proof]
    #[kani::unwind(17)]
    fn sixteen_byte_headers() {
        let bytes: [u8; 16] = kani::any();
        let len: u8 = kani::any();
        kani::assume(len <= 16);
        assert_header(&bytes[..len as usize]);
    }

    #[kani::proof]
    #[kani::unwind(17)]
    fn sixteen_byte_surface_view() {
        let bytes: [u8; 16] = kani::any();
        let width: u8 = kani::any();
        let height: u8 = kani::any();
        let stride: u8 = kani::any();
        assert_view(
            &bytes,
            width as u32,
            height as usize,
            stride as usize,
            kani::any(),
        );
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn eight_byte_row_write() {
        let bytes: [u8; 8] = kani::any();
        let source: [u8; 2] = kani::any();
        let row_bytes: u8 = kani::any();
        let height: u8 = kani::any();
        let stride: u8 = kani::any();
        assert_write(
            &bytes,
            row_bytes as usize,
            height as usize,
            stride as usize,
            kani::any(),
            &source,
        );
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn eight_byte_scalars() {
        let bytes: [u8; 8] = kani::any();
        assert_scalars(&bytes, kani::any());
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn four_byte_cursor() {
        let bytes: [u8; 4] = kani::any();
        assert_cursor(&bytes, kani::any(), kani::any());
    }

    #[kani::proof]
    #[kani::unwind(9)]
    fn eight_byte_surface_borrow() {
        let bytes: [u8; 8] = kani::any();
        let row_bytes: u8 = kani::any();
        let height: u8 = kani::any();
        let stride: u8 = kani::any();
        assert_borrow(
            &bytes,
            row_bytes as usize,
            height as usize,
            stride as usize,
            kani::any(),
        );
    }

    #[kani::proof]
    fn all_versions() {
        assert_versions(kani::any(), kani::any(), kani::any());
    }

    // Bounded geometry for tractable SAT multiplication; capacity remains full width.
    // 几何参数限定 8 位以控制 SAT 乘法成本，容量仍为全 usize；全域由 Verus 覆盖。
    #[kani::proof]
    fn bounded_surface_spans() {
        let row: u8 = kani::any();
        let height: u8 = kani::any();
        let stride: u8 = kani::any();
        assert_surface_span(row as usize, height as usize, stride as usize, kani::any());
    }

    #[kani::proof]
    fn all_surface_rows() {
        assert_surface_row(kani::any(), kani::any());
    }

    #[kani::proof]
    fn all_usize_endpoints() {
        assert_endpoint(kani::any(), kani::any(), kani::any());
    }

    // All byte values and all usize cursors/lengths, for a four-byte allocation.
    // 四字节分配内穷尽字节值以及所有 usize 起点/长度；不是任意缓冲长度证明。
    #[kani::proof]
    #[kani::unwind(5)]
    fn four_byte_reads() {
        let bytes: [u8; 4] = kani::any();
        assert_read(&bytes, kani::any(), kani::any());
    }
}

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn pointer_order_property(a in any::<u64>(), b in any::<u64>(), c in any::<u64>()) {
            assert_pointer_order(a, b, c); assert_pointer_order(a, a, c);
        }
        #[test]
        fn capture_query_property(a in any::<u64>(), b in any::<u64>(), q in any::<u64>(), at in any::<Option<u64>>(), bt in any::<Option<u64>>()) {
            assert_capture_query(a, b, q, at, bt);
            assert_capture_query(a, b, a, at, bt);
            assert_capture_query(a, b, b, at, bt);
            assert_capture_query(a, a, a, at, bt);
        }
        #[test]
        fn hover_lifecycle_property(old in any::<Option<u64>>(), target in any::<Option<u64>>(), session in any::<u64>(), slot in any::<u64>(), generation in any::<u64>(), live in any::<bool>()) {
            assert_hover_lifecycle(old, target, session, slot, generation, live);
            assert_hover_lifecycle(old, old, session, slot, generation, live);
        }
        #[test]
        fn focus_selection_property(order in prop::collection::vec(any::<u64>(), 0..32), focused in any::<Option<u64>>(), target in any::<u64>()) {
            assert_focus_selection(&order, focused, target);
            if let Some(id) = order.last() { assert_focus_selection(&order, focused, *id); }
        }
        #[test]
        fn input_retire_property(order in prop::collection::vec(any::<u64>(), 0..32), focused in any::<Option<u64>>(), session in any::<u64>(), slot in any::<u64>(), generation in any::<u64>(), live in any::<bool>()) {
            assert_input_retire(&order, focused, session, slot, generation, live);
        }
        #[test]
        fn keyboard_focus_property(order in prop::collection::vec(any::<u64>(), 0..32), focused in any::<Option<u64>>()) {
            assert_keyboard_focus(&order, focused);
        }
        #[test]
        fn focus_cleanup_property(order in prop::collection::vec(any::<u64>(), 0..32), focused in any::<Option<u64>>()) {
            assert_focus_cleanup(&order, focused);
        }
        #[test]
        fn text_classes_property(ch in any::<char>()) { assert_text_classes(ch); }
        #[test]
        fn bytes_slot_property(bytes in prop::collection::vec(any::<u8>(), 0..128),
                               session in any::<u64>(), slot_id in any::<u64>(), caller in any::<u64>()) {
            assert_bytes_slot(&bytes, session, slot_id, caller);
        }
        #[test]
        fn modifier_property(flags in any::<u8>()) { assert_command_modifier(flags); }
        #[test]
        fn resource_property(session in any::<u64>(), slot in any::<u64>(), generation in any::<u64>(),
                             live in any::<bool>(), caller in any::<u64>(), key_generation in any::<u64>()) {
            assert_resource(session, slot, generation, live, caller,
                ResourceKey { session: caller, slot, generation: key_generation });
            assert_resource(session, slot, generation, live, session,
                ResourceKey { session, slot, generation });
        }
        #[test]
        fn role_property(code in any::<u8>()) { assert_role(code); }
        #[test]
        fn canvas_import_property(bytes in prop::collection::vec(any::<u8>(), 0..256),
                                  width in any::<u32>(), height in any::<u32>(),
                                  small_width in 1u32..8, small_height in 1u32..8) {
            assert_canvas_import(&bytes, width, height);
            let mut packed = vec![0; (small_width * small_height * 4) as usize];
            for (i, byte) in packed.iter_mut().enumerate() {
                *byte = bytes.get(i % bytes.len().max(1)).copied().unwrap_or(0);
            }
            assert_canvas_import(&packed, small_width, small_height);
            assert_canvas_import(&packed[..packed.len()-1], small_width, small_height);
        }
        #[test]
        fn rgb_word_property(red in any::<u8>(), green in any::<u8>(), blue in any::<u8>()) {
            assert_rgb_word(red, green, blue);
        }
        #[test]
        fn pixel_conversion_property(pixels in prop::collection::vec(any::<u8>(), 0..256),
                                      initial in prop::collection::vec(any::<u32>(), 0..64)) {
            assert_pixel_conversion(&pixels, &initial);
            let aligned = &pixels[..pixels.len() / 4 * 4];
            assert_pixel_conversion(aligned, &vec![0xa5a5a5a5; aligned.len() / 4]);
        }
        #[test]
        fn payload_property(bytes in prop::collection::vec(any::<u8>(), 0..128),
                            start in any::<usize>(), limit in any::<u32>(),
                            prefix_len in 0usize..8, payload in prop::collection::vec(any::<u8>(), 0..32)) {
            assert_payload(&bytes, start, limit);
            let mut frame = vec![0x55; prefix_len];
            frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            frame.extend_from_slice(&payload);
            assert_payload(&frame, prefix_len, limit);
            assert_payload(&frame, prefix_len, payload.len() as u32);
            if !payload.is_empty() {
                assert_payload(&frame, prefix_len, payload.len() as u32 - 1);
                frame.pop();
                assert_payload(&frame, prefix_len, payload.len() as u32);
            }
        }
        #[test]
        fn view_copy_property(bytes in prop::collection::vec(any::<u8>(), 0..256),
                              destination in prop::collection::vec(any::<u8>(), 0..32),
                              width in 1u32..8, height in 1usize..5,
                              padding in 0usize..8, row in any::<usize>()) {
            let stride = width as usize * 4 + padding;
            assert_view_copy(&bytes, width, height, stride, row, &destination);
            for active_row in 0..height {
                assert_view_copy(&bytes, width, height, stride, active_row, &destination);
            }
        }
        #[test]
        fn header_property(bytes in prop::collection::vec(any::<u8>(), 0..128),
                           version in any::<u16>(), count in any::<u32>(),
                           small_count in 0u32..64, tail in 0usize..64) {
            assert_header(&bytes);
            let mut frame = b"JUEB".to_vec();
            frame.extend_from_slice(&version.to_le_bytes());
            frame.extend_from_slice(&count.to_le_bytes());
            frame.resize(10 + tail, 0);
            assert_header(&frame);
            frame[4..6].copy_from_slice(&1u16.to_le_bytes());
            frame[6..10].copy_from_slice(&small_count.to_le_bytes());
            assert_header(&frame);
        }
        #[test]
        fn surface_view_property(bytes in prop::collection::vec(any::<u8>(), 0..256),
                                 width in any::<u32>(), height in any::<usize>(),
                                 stride in any::<usize>(), row in any::<usize>(),
                                 small_width in 1u32..8, small_height in 1usize..5,
                                 padding in 0usize..8) {
            assert_view(&bytes, width, height, stride, row);
            assert_view(&bytes, small_width, small_height, small_width as usize * 4 + padding, row);
        }
        #[test]
        fn row_write_property(bytes in prop::collection::vec(any::<u8>(), 0..4096),
                              source in prop::collection::vec(any::<u8>(), 0..32),
                              row_bytes in any::<usize>(), height in any::<usize>(),
                              stride in any::<usize>(), row in any::<usize>()) {
            assert_write(&bytes, row_bytes, height, stride, row, &source);
            for active_row in 0..4 {
                assert_write(&bytes, source.len(), 4, source.len() + 2, active_row, &source);
            }
        }
        #[test]
        fn scalar_property(bytes in prop::collection::vec(any::<u8>(), 0..4096),
                           start in any::<usize>(), small_start in 0usize..4096) {
            assert_scalars(&bytes, start);
            assert_scalars(&bytes, small_start);
            assert_scalars(&bytes, 0);
        }
        #[test]
        fn cursor_property(bytes in prop::collection::vec(any::<u8>(), 0..4096),
                           start in any::<usize>(), len in any::<usize>(),
                           small_start in 0usize..4096, small_len in 0usize..4096) {
            assert_cursor(&bytes, start, len);
            assert_cursor(&bytes, small_start, small_len);
        }
        #[test]
        fn borrow_property(bytes in prop::collection::vec(any::<u8>(), 0..4096),
                           row_bytes in any::<usize>(), height in any::<usize>(),
                           stride in any::<usize>(), row in any::<usize>()) {
            assert_borrow(&bytes, row_bytes, height, stride, row);
            for active_row in 0..4 {
                assert_borrow(&bytes, 2, 4, 3, active_row);
            }
        }
        #[test]
        fn version_property(current in any::<u64>(), expected in any::<u64>(), empty in any::<bool>()) {
            assert_versions(current, expected, empty);
            assert_versions(current, current, empty);
        }
        #[test]
        fn surface_span_property(row in any::<usize>(), height in any::<usize>(),
                                 stride in any::<usize>(), capacity in any::<usize>(),
                                 small_row in 1usize..128, small_height in 1usize..128,
                                 padding in 0usize..64) {
            assert_surface_span(row, height, stride, capacity);
            let stride = small_row + padding;
            let end = (small_height - 1) * stride + small_row;
            assert_surface_span(small_row, small_height, stride, end);
            assert_surface_span(small_row, small_height, stride, end - 1);
        }
        #[test]
        fn surface_row_property(width in any::<u32>(), budget in any::<usize>()) {
            assert_surface_row(width, budget);
            assert_surface_row(width, width as usize);
        }
        #[test]
        fn endpoint_property(total in any::<usize>(), pos in any::<usize>(), len in any::<usize>()) {
            assert_endpoint(total, pos, len);
        }
        #[test]
        fn read_property(bytes in prop::collection::vec(any::<u8>(), 0..4096),
                         start in any::<usize>(), len in any::<usize>(),
                         small_start in 0usize..4096, small_len in 0usize..4096) {
            assert_read(&bytes, start, len);
            assert_read(&bytes, small_start, small_len);
        }
    }
}

#[cfg(test)]
mod memory {
    use super::*;

    #[test]
    fn slices_and_failure_positions() {
        assert_focus_selection(&[], None, 0);
        assert_focus_selection(&[1, 1, u64::MAX], Some(7), u64::MAX);
        assert_input_retire(&[], None, 0, 0, 0, false);
        assert_input_retire(&[1, 1, u64::MAX], Some(7), 7, 8, u64::MAX, true);
        assert_hover_lifecycle(None, None, 0, 0, 0, false);
        assert_hover_lifecycle(Some(7), Some(7), 7, 42, 1, true);
        assert_hover_lifecycle(Some(7), Some(u64::MAX), 7, 42, u64::MAX, true);
        assert_hover_lifecycle(None, Some(7), 7, 42, 1, true);
        assert_hover_lifecycle(Some(7), None, 7, 42, 1, true);
        assert_pointer_order(0, u64::MAX, 1);
        assert_capture_query(0, u64::MAX, u64::MAX, Some(1), Some(2));
        assert_capture_query(7, 7, 7, Some(1), None);
        assert_capture_query(7, 8, 9, Some(1), Some(2));
        assert_keyboard_focus(&[], None);
        assert_keyboard_focus(&[1, 1, u64::MAX], Some(42));
        assert_focus_cleanup(&[], None);
        assert_focus_cleanup(&[1, 2, 3], Some(2));
        assert_focus_cleanup(&[1, 1, u64::MAX], Some(42));
        for value in [
            0, 767, 768, 879, 880, 4351, 4352, 4447, 4448, 11903, 11904, 42191, 42192, 917759,
            917760, 917999, 918000, 0x10ffff,
        ] {
            if let Some(ch) = char::from_u32(value) {
                assert_text_classes(ch);
            }
        }
        for flags in 0..=u8::MAX {
            assert_command_modifier(flags);
        }
        for len in [0, 1, 8, 32] {
            assert_bytes_slot(&vec![37; len], 7, 42, 7);
            assert_bytes_slot(&vec![37; len], 7, 42, 8);
        }
        for code in 0..=u8::MAX {
            assert_role(code);
        }
        for generation in [0, 1, u64::MAX - 1, u64::MAX] {
            for live in [false, true] {
                for caller in [7, 8] {
                    assert_resource(
                        7,
                        42,
                        generation,
                        live,
                        caller,
                        ResourceKey {
                            session: 7,
                            slot: 42,
                            generation,
                        },
                    );
                }
            }
        }
        for width in [0, 1, 2, u32::MAX] {
            for height in [0, 1, 2, u32::MAX] {
                for len in 0..=16 {
                    assert_canvas_import(&vec![42; len], width, height);
                }
            }
        }
        for width in [0, 1, 16, u32::MAX] {
            for budget in [0, 4, 63, 64, usize::MAX] {
                assert_surface_row(width, budget);
            }
        }
        for (row, height, stride, capacity) in [
            (16, 3, 20, 56),
            (16, 3, 20, 55),
            (16, 1, 20, 16),
            (0, 1, 0, 0),
            (1, 0, 1, 1),
            (16, 2, 15, 32),
            (1, usize::MAX, usize::MAX, usize::MAX),
            (usize::MAX, 2, usize::MAX, usize::MAX),
        ] {
            assert_surface_span(row, height, stride, capacity);
        }
        for current in [0, 1, u64::MAX - 1, u64::MAX] {
            for empty in [false, true] {
                assert_versions(current, current, empty);
                assert_versions(current, current.wrapping_add(1), empty);
            }
        }
        let packed = [255, 0, 0, 0, 0, 128, 255, 255];
        for len in 0..=packed.len() {
            for words in 0..=2 {
                assert_pixel_conversion(&packed[..len], &vec![u32::MAX; words]);
            }
        }
        for channel in [0, 1, 128, 255] {
            assert_rgb_word(channel, channel, channel);
        }
        let mut a = [u32::MAX];
        let mut b = [u32::MAX];
        rgba_to_u32(&[12, 34, 56, 0], &mut a).unwrap();
        rgba_to_u32(&[12, 34, 56, 255], &mut b).unwrap();
        assert_eq!(a, b);
        let rgba: [u8; 20] = core::array::from_fn(|i| i as u8);
        let retained_row = {
            let view = Rgba8SurfaceView::new(&rgba, 2, 2, 12).unwrap();
            assert_eq!(view.row(0), Some(&rgba[..8]));
            view.row(1).unwrap()
        };
        assert_eq!(retained_row, &rgba[12..20]);
        let view = Rgba8SurfaceView::new(&rgba, 2, 2, 12).unwrap();
        let mut output = [0x55u8; 8];
        assert!(view.copy_row_to(1, &mut output));
        assert_eq!(output, rgba[12..20]);
        let mut short = [0x55u8; 7];
        assert!(!view.copy_row_to(0, &mut short));
        assert_eq!(short, [0x55; 7]);
        assert!(!view.copy_row_to(2, &mut output));
        assert_eq!(output, rgba[12..20]);
        for row in [0, 1, 2, usize::MAX] {
            assert_view_copy(&rgba, 2, 2, 12, row, &[0x55; 8]);
            assert_view_copy(&rgba, 2, 2, 12, row, &[0x55; 7]);
        }
        let payload = b"\x03\x00\x00\x00abc";
        for len in 0..=payload.len() {
            assert_payload(&payload[..len], 0, 2);
            assert_payload(&payload[..len], 0, 3);
            assert_payload(&payload[..len], usize::MAX, u32::MAX);
        }
        assert_payload(b"\xff\xff\xff\xff", 0, 16);
        assert_payload(b"\x00\x00\x00\x00", 0, 0);
        for row in [0, 1, 2, usize::MAX] {
            assert_view(&rgba, 2, 2, 12, row);
            assert_view(&rgba[..19], 2, 2, 12, row);
            assert_view(&rgba, u32::MAX, 2, 12, row);
        }
        let header = b"JUEB\x01\x00\x01\x00\x00\x00\x06";
        for len in 0..=header.len() {
            assert_header(&header[..len]);
        }
        assert_header(b"JUEB\x02\x00\xff\xff\xff\xff");
        let padded = [1, 2, 0xaa, 0xbb, 3, 4];
        for row in [0, 1, 2, usize::MAX] {
            for source in [&[][..], &[9, 8][..], &[9, 8, 7][..]] {
                assert_write(&padded, 2, 2, 4, row, source);
                assert_write(&padded[..5], 2, 2, 4, row, source);
            }
        }
        let mut destination = padded;
        assert!(write_surface_row(&mut destination, 2, 2, 4, 1, &[9, 8]));
        assert_eq!(destination, [1, 2, 0xaa, 0xbb, 9, 8]);
        for row in [0, 1, 2, usize::MAX] {
            assert_borrow(&padded, 2, 2, 4, row);
            assert_borrow(&padded[..5], 2, 2, 4, row);
        }
        assert_eq!(surface_row(&padded, 2, 2, 4, 1), Some(&[3, 4][..]));
        let bytes = [1u8, 3, 7, 255];
        for start in [0, 1, 4, 5, usize::MAX] {
            for len in [0, 1, 3, 4, 5, usize::MAX] {
                assert_read(&bytes, start, len);
                assert_cursor(&bytes, start, len);
            }
        }
        for start in [0, 1, 2, 3, 4, usize::MAX] {
            assert_scalars(&bytes, start);
        }
        assert_endpoint(usize::MAX, usize::MAX, 1);
        assert_endpoint(usize::MAX, usize::MAX, 0);
    }
}

#[cfg(test)]
mod schedules {
    use super::*;
    use loom::{sync::Arc, thread};

    #[test]
    fn shared_immutable_buffer_independent_cursors() {
        // Production functions have no synchronization. This explores caller-side
        // ownership isolation, not an invented engine lock or IPC protocol.
        // 生产函数无同步原语；仅检查调用侧共享只读缓冲和独立游标的隔离。
        loom::model(|| {
            let mut storage = UiBytesSlot::new(7, 43);
            let storage_key = storage.install(7, vec![11, 22, 33, 44]).unwrap();
            let storage = Arc::new(storage);
            let storage_a = storage.clone();
            let storage_b = storage.clone();
            let bytes = Arc::new([11u8, 22, 33, 44]);
            let first = bytes.clone();
            let a = thread::spawn(move || {
                assert_eq!(storage_a.bytes(7, storage_key), Some(&[11, 22, 33, 44][..]));
                assert_text_classes('中');
                assert_command_modifier(1);
                let mut gate = ResourceGate::new(7, 42);
                let key = gate.issue(7).unwrap();
                thread::yield_now();
                assert!(gate.allows(7, key));
                assert!(!gate.allows(8, key));
                assert!(gate.revoke(7, key));
                let next = gate.issue(7).unwrap();
                assert!(!gate.allows(7, key));
                assert!(gate.allows(7, next));
                gate.retire();
                assert!(gate.issue(7).is_none());
                assert!(!gate.allows(7, next));
                assert_focus_cleanup(&[1, 2], Some(1));
                assert_keyboard_focus(&[1, 2], Some(1));
                assert_pointer_order(1, 2, 3);
                assert_capture_query(1, 2, 1, Some(7), Some(8));
                assert_hover_lifecycle(Some(1), Some(2), 7, 42, 1, true);
                assert_focus_selection(&[1, 2], Some(1), 2);
                assert_input_retire(&[1, 2], Some(1), 7, 42, 1, true);
                assert_role(9);
                assert_surface_span(16, 3, 20, 56);
                assert_eq!(surface_span(16, 3, 20, 56), Some(56));
                assert_versions(u64::MAX, u64::MAX, false);
                assert_surface_row(16, 64);
                assert_eq!(rgba8_row_bytes(16, 64), Some(64));
                assert_borrow(first.as_ref(), 1, 2, 3, 1);
                assert_eq!(surface_row(first.as_ref(), 1, 2, 3, 1), Some(&[44][..]));
                assert_cursor(first.as_ref(), 1, 2);
                assert_scalars(first.as_ref(), 0);
                let canvas = Canvas::from_rgba8(1, 1, first.to_vec()).unwrap();
                thread::yield_now();
                assert_eq!(canvas.pixels(), first.as_ref());
                assert_eq!(canvas.width(), 1);
                assert_eq!(canvas.height(), 1);
                let mut words = [u32::MAX];
                rgba_to_u32(first.as_ref(), &mut words).unwrap();
                thread::yield_now();
                assert_eq!(words, [0x000b1621]);
                assert_view(first.as_ref(), 1, 1, 4, 0);
                let view = Rgba8SurfaceView::new(first.as_ref(), 1, 1, 4).unwrap();
                let mut transferred = [0u8; 4];
                assert!(view.copy_row_to(0, &mut transferred));
                thread::yield_now();
                assert_eq!(transferred, [11, 22, 33, 44]);
                assert_payload(b"\x01\x00\x00\x00a", 0, 1);
                assert_header(b"JUEB\x01\x00\x00\x00\x00\x00");
                let mut output = *first;
                assert!(write_surface_row(&mut output, 1, 2, 3, 1, &[99]));
                thread::yield_now();
                assert_eq!(output, [11, 22, 33, 99]);
                let mut pos = 0;
                assert_eq!(read(first.as_ref(), &mut pos, 2), Some(&[11, 22][..]));
                thread::yield_now();
                assert_eq!(pos, 2);
                assert_eq!(read(first.as_ref(), &mut pos, usize::MAX), None);
                assert_eq!(pos, 2);
            });
            let second = bytes.clone();
            let b = thread::spawn(move || {
                assert_eq!(storage_b.bytes(8, storage_key), None);
                assert_text_classes('a');
                assert_keyboard_focus(&[3, 4], None);
                assert_capture_query(3, 3, 3, Some(7), None);
                assert_hover_lifecycle(Some(3), None, 8, 42, 1, true);
                assert_focus_selection(&[3, 4], None, 5);
                assert_input_retire(&[3, 4], None, 8, 42, 0, false);
                assert_command_modifier(8);
                let mut gate = ResourceGate::new(8, 42);
                assert!(gate.issue(7).is_none());
                let key = gate.issue(8).unwrap();
                thread::yield_now();
                assert!(!gate.allows(7, key));
                assert!(!gate.revoke(7, key));
                assert!(gate.allows(8, key));
                assert_role(14);
                assert_surface_span(16, 3, 20, 55);
                assert_eq!(surface_span(16, 3, 20, 55), None);
                assert_versions(7, 6, true);
                assert_surface_row(16, 63);
                assert_eq!(rgba8_row_bytes(16, 63), None);
                assert_borrow(second.as_ref(), 1, 3, 3, 0);
                assert_eq!(surface_row(second.as_ref(), 1, 3, 3, 0), None);
                assert_cursor(second.as_ref(), 1, usize::MAX);
                assert_scalars(second.as_ref(), 2);
                let returned = Canvas::from_rgba8(1, 2, second.to_vec()).unwrap_err();
                thread::yield_now();
                assert_eq!(returned, second.as_ref());
                let mut words = [0x55555555; 2];
                assert_eq!(
                    rgba_to_u32(second.as_ref(), &mut words),
                    Err(PixelError::LengthMismatch)
                );
                thread::yield_now();
                assert_eq!(words, [0x55555555; 2]);
                assert_view(second.as_ref(), 1, 2, 4, 0);
                let view = Rgba8SurfaceView::new(second.as_ref(), 1, 1, 4).unwrap();
                let mut transferred = [0x55u8; 4];
                assert!(!view.copy_row_to(1, &mut transferred));
                thread::yield_now();
                assert_eq!(transferred, [0x55; 4]);
                assert_payload(b"\x02\x00\x00\x00", 0, 1);
                assert_header(b"JUEB\x02\x00\x00\x00\x00\x00");
                let mut output = *second;
                assert!(write_surface_row(&mut output, 1, 2, 3, 0, &[88]));
                thread::yield_now();
                assert_eq!(output, [88, 22, 33, 44]);
                let mut pos = 1;
                assert_eq!(read(second.as_ref(), &mut pos, 3), Some(&[22, 33, 44][..]));
                thread::yield_now();
                assert_eq!(pos, 4);
                assert_eq!(read(second.as_ref(), &mut pos, 0), Some(&[][..]));
            });
            a.join().unwrap();
            b.join().unwrap();
            let mut storage = Arc::try_unwrap(storage).ok().unwrap();
            assert_eq!(storage.release(7, storage_key), Some(vec![11, 22, 33, 44]));
            let next = storage.install(7, vec![99]).unwrap();
            assert_eq!(storage.retire(), Some(vec![99]));
            assert_eq!(storage.bytes(7, next), None);
            assert_eq!(storage.install(7, vec![1]), Err(vec![1]));
            assert_eq!(*bytes, [11, 22, 33, 44]);
        });
    }
}
