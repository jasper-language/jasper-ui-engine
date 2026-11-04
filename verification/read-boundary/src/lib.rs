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

/// Independent wide-integer oracle for Canvas::pixel over an imported packed buffer.
/// 用宽整数独立校验导入缓冲上的 Canvas::pixel 越界拒绝与通道读取。
pub fn assert_canvas_pixel(bytes: &[u8], width: u32, height: u32, x: u32, y: u32) {
    let Ok(canvas) = Canvas::from_rgba8(width, height, bytes.to_vec()) else {
        return;
    };
    let result = canvas.pixel(x, y);
    if x >= width || y >= height {
        assert!(result.is_none());
        return;
    }
    let index = ((y as u128 * width as u128 + x as u128) * 4) as usize;
    let color = result.unwrap();
    assert_eq!([color.r, color.g, color.b, color.a], bytes[index..index + 4]);
}

/// Allocation-free admission oracle (bounded-model-checking friendly).
/// 无分配的准入规格（便于有界模型检查）。
pub fn assert_font_admission(seen: &[u64], id: u64, count: usize) {
    assert_eq!(MAX_INJECTED_FONTS, 16);
    assert_eq!(font_stack_count_admits(count), (1..=16).contains(&count));
    assert_eq!(font_order_admits(seen, id), !seen.iter().any(|&known| known == id));
}

/// Independent oracle for font stack admission and first-occurrence fallback order.
/// 独立校验字体栈数量准入与首次出现回退顺序。
pub fn assert_font_order(seen: &[u64], id: u64, count: usize) {
    assert_font_admission(seen, id, count);
    // Repeated admission keeps each id once, in host (first-seen) order.
    // 重复准入后每个 id 只保留一次，顺序为宿主首次出现顺序。
    let (mut order, mut expected) = (Vec::new(), Vec::<u64>::new());
    for &next in seen.iter().chain(core::iter::once(&id)) {
        if font_order_admits(&order, next) {
            order.push(next);
        }
        if expected.iter().all(|&known| known != next) {
            expected.push(next);
        }
    }
    assert_eq!(order, expected);
}

/// Independent colour construction oracle: channels kept exactly, rgb opaque, with_alpha
/// replaces alpha alone.
/// 独立颜色构造规格：通道原样，rgb 不透明，with_alpha 只替换 alpha。
pub fn assert_color_construct(r: u8, g: u8, b: u8, a: u8, alpha: u8) {
    let color = Color::new(r, g, b, a);
    assert_eq!([color.r, color.g, color.b, color.a], [r, g, b, a]);
    let opaque = Color::rgb(r, g, b);
    assert_eq!([opaque.r, opaque.g, opaque.b, opaque.a], [r, g, b, 255]);
    let replaced = color.with_alpha(alpha);
    assert_eq!([replaced.r, replaced.g, replaced.b, replaced.a], [r, g, b, alpha]);
}

/// Independent CSS numeric weight oracle: admitted exactly on 1..=1000.
/// 独立 CSS 数值字重规格：恰在 1..=1000 准入。
pub fn assert_font_weight(weight: u16) {
    assert_eq!(font_weight_admits(weight), (1..=1000).contains(&weight));
}

/// Independent family-name oracle: 1..=128 bytes, none a control byte, DEL, quote or backslash.
/// 独立族名规格：1..=128 字节，且无控制字节、DEL、双引号、反斜杠。
pub fn assert_font_family(name: &[u8]) {
    assert_eq!(MAX_FONT_FAMILY_BYTES, 128);
    let expected = (1..=128).contains(&name.len())
        && name.iter().all(|byte| !byte.is_ascii_control() && !matches!(byte, b'"' | b'\\'));
    assert_eq!(font_family_admits(name), expected);
}

/// Independent bold-alias admission oracle: nonempty equal-length tables, every face is its
/// group maximum, and every group maximum is at most 500.
/// 独立粗体替身准入规格：非空等长表、每个字面均等于本组最大字重，且组最大字重不超过 500。
pub fn assert_bold_alias(weights: &[u16], group_max: &[u16]) {
    let expected=weights.len()>=1 && weights.len()==group_max.len()
        && weights.iter().zip(group_max).all(|(&weight,&maximum)|
            weight==maximum && maximum<=BOLD_ALIAS_MAX_SOURCE_WEIGHT);
    assert_eq!(bold_alias_admits(weights,group_max),expected);
}

/// Independent millisecond oracle over the widened value: exact when it fits, u64::MAX otherwise.
/// 独立毫秒规格：可表示时精确，否则为 u64::MAX。
pub fn assert_desktop_timestamp(elapsed: u128) {
    let result = desktop_timestamp_millis(elapsed);
    if elapsed <= u64::MAX as u128 {
        assert_eq!(result as u128, elapsed);
    } else {
        assert_eq!(result, u64::MAX);
    }
}

/// One clock-free pointer transition: (operation, button tag, Other code, x bits, y bits, timestamp).
/// 一次无时钟指针转换：（操作、按钮标签、Other 码、x 位、y 位、时间戳）。
pub type DesktopPointerStep = (u8, u8, u16, u32, u32, u64);

/// Independent array oracle for the handle table: live values and each slot's newest generation.
/// 句柄表的独立数组模型：存活值与每槽最新代数；全部历史句柄都必须保持正确的存活判定。
pub type HandleStep = (u8, u8, u32);

fn handle_status_oracle(slots: &[Option<(u32, Option<u32>)>], handle: Handle) -> HandleStatus {
    let current = match slots.get(handle.index as usize) {
        Some(Some((generation, _))) => *generation,
        _ => return HandleStatus::Invalid,
    };
    if handle.generation % 2 == 0 || handle.generation > current {
        HandleStatus::Invalid
    } else if handle.generation == current {
        HandleStatus::Live
    } else {
        HandleStatus::Stale
    }
}

pub fn assert_handle_path(capacity: u32, first: u32, steps: &[HandleStep]) {
    const SLOTS: usize = 4;
    const HISTORY: usize = 8;
    let capacity = capacity % (SLOTS as u32 + 1);
    let first = first | 1;
    let Some(mut table) = HandleTable::with_first_generation(capacity, first) else {
        assert!(first >= HANDLE_GENERATION_RETIRED);
        return;
    };
    let mut slots: [Option<(u32, Option<u32>)>; SLOTS] = [None; SLOTS];
    let mut history: [Option<Handle>; HISTORY] = [None; HISTORY];
    let mut issued = 0;
    let mut step = 0;
    while step < steps.len() {
        let (operation, pick, value) = steps[step];
        step += 1;
        match operation % 3 {
            0 => {
                let mut reusable = false;
                let mut filled = 0;
                let mut slot = 0;
                while slot < SLOTS {
                    if let Some((generation, held)) = slots[slot] {
                        filled += 1;
                        reusable |= held.is_none() && generation != HANDLE_GENERATION_RETIRED;
                    }
                    slot += 1;
                }
                match table.insert(value) {
                    Ok(handle) => {
                        let at = handle.index as usize;
                        assert!(at < capacity as usize && handle.generation % 2 == 1);
                        match slots[at] {
                            Some((old, held)) => {
                                assert!(held.is_none() && old != HANDLE_GENERATION_RETIRED && handle.generation == old + 1);
                            }
                            None => {
                                assert_eq!((at, handle.generation), (filled, first));
                                assert!(!reusable);
                            }
                        }
                        slots[at] = Some((handle.generation, Some(value)));
                        if issued < HISTORY {
                            history[issued] = Some(handle);
                            issued += 1;
                        }
                    }
                    Err(returned) => {
                        assert_eq!(returned, value);
                        assert!(!reusable && filled == capacity as usize);
                    }
                }
            }
            1 => {
                let target = if issued > 0 && pick & 0x80 == 0 { history[pick as usize % issued] } else { None };
                let handle = target.unwrap_or(Handle { index: pick as u32, generation: value });
                let at = handle.index as usize;
                let held = if at < SLOTS {
                    match slots[at] {
                        Some((generation, held)) if generation == handle.generation => held,
                        _ => None,
                    }
                } else {
                    None
                };
                assert_eq!(table.remove(handle), held);
                if held.is_some() {
                    slots[at] = Some((handle.generation + 1, None));
                }
            }
            _ => {
                let foreign = Handle { index: pick as u32, generation: value };
                let live = (foreign.index as usize) < SLOTS
                    && foreign.generation % 2 == 1
                    && matches!(slots[foreign.index as usize], Some((generation, _)) if generation == foreign.generation);
                assert_eq!(table.is_live(foreign), live);
                assert_eq!(table.status(foreign), handle_status_oracle(&slots, foreign));
                if let Some(held) = table.get_mut(foreign) {
                    assert!(live);
                    *held = held.wrapping_add(1);
                    let (generation, _) = slots[foreign.index as usize].unwrap();
                    slots[foreign.index as usize] = Some((generation, Some(*held)));
                }
            }
        }
        let mut seen = 0;
        while seen < issued {
            let handle = history[seen].unwrap();
            let expected = match slots[handle.index as usize] {
                Some((generation, held)) if generation == handle.generation => held,
                _ => None,
            };
            assert_eq!(table.get(handle).copied(), expected);
            assert_eq!(table.is_live(handle), expected.is_some());
            assert_eq!(table.status(handle), handle_status_oracle(&slots, handle));
            seen += 1;
        }
    }
}

/// Two inserts, a foreign probe, removal and slot reuse with every input arbitrary.
/// 两次插入、外来句柄探测、移除与槽位复用，全部输入任意。
pub fn assert_handle_fixed_path(first: u32, index: u32, generation: u32, a: u8, b: u8) {
    let Some(mut table) = HandleTable::with_first_generation(2, first) else {
        assert!(first & 1 != 1 || first >= HANDLE_GENERATION_RETIRED);
        return;
    };
    assert!(first & 1 == 1 && first < HANDLE_GENERATION_RETIRED);
    let foreign = Handle { index, generation };
    let one = table.insert(a).unwrap();
    let two = table.insert(b).unwrap();
    assert_eq!((one, two), (Handle { index: 0, generation: first }, Handle { index: 1, generation: first }));
    assert!(table.insert(a).is_err());
    let expected = if foreign == one {
        Some(a)
    } else if foreign == two {
        Some(b)
    } else {
        None
    };
    assert_eq!(table.get(foreign).copied(), expected);
    assert_eq!(table.remove(foreign), expected);
    assert_eq!(table.remove(foreign), None);
    if expected.is_some() && first == HANDLE_GENERATION_RETIRED - 1 {
        // The released slot retired; nothing is reusable and capacity is spent.
        // 被释放的槽位已退役，无可复用槽位且容量已满。
        assert_eq!(table.insert(b), Err(b));
    } else if expected.is_some() {
        let reused = table.insert(b).unwrap();
        assert_eq!((reused.index, reused.generation), (foreign.index, first + 2));
        assert_eq!(table.get(foreign), None);
        assert_eq!(table.get(reused), Some(&b));
    }
}

/// Allocator transitions with arbitrary first generation and foreign handle.
/// 分配器转换，首代与外来句柄任意。
pub fn assert_handle_allocator_path(first: u32, index: u32, generation: u32) {
    let Some(mut allocator) = HandleAllocator::with_first_generation(1, first) else {
        assert!(first & 1 != 1 || first >= HANDLE_GENERATION_RETIRED);
        return;
    };
    assert!(first & 1 == 1 && first < HANDLE_GENERATION_RETIRED);
    let foreign = Handle { index, generation };
    let live = allocator.allocate().unwrap();
    assert_eq!(live, Handle { index: 0, generation: first });
    assert!(allocator.allocate().is_none());
    assert_eq!(allocator.is_live(foreign), foreign == live);
    assert_eq!(allocator.release(foreign), foreign == live);
    assert!(!allocator.is_live(foreign) || foreign != live);
    if foreign == live {
        match allocator.allocate() {
            Some(next) => assert_eq!(next, Handle { index: 0, generation: first + 2 }),
            None => assert_eq!(first, HANDLE_GENERATION_RETIRED - 1),
        }
    }
}

/// Status through both types: live, then stale once released, against an arbitrary foreign handle.
/// 经两种类型核对状态：存活，释放后过期；外来句柄任意。
pub fn assert_handle_status_path(first: u32, index: u32, generation: u32) {
    let Some(mut allocator) = HandleAllocator::with_first_generation(1, first) else {
        return;
    };
    let foreign = Handle { index, generation };
    let expected = |current: u32| {
        if index != 0 || generation & 1 == 0 || generation > current {
            HandleStatus::Invalid
        } else if generation == current {
            HandleStatus::Live
        } else {
            HandleStatus::Stale
        }
    };
    let live = allocator.allocate().unwrap();
    assert_eq!(allocator.status(live), HandleStatus::Live);
    assert_eq!(allocator.status(foreign), expected(first));
    assert!(allocator.release(live));
    assert_eq!(allocator.status(live), HandleStatus::Stale);
    assert_eq!(allocator.status(foreign), if generation & 1 == 1 && index == 0 && generation <= first { HandleStatus::Stale } else { HandleStatus::Invalid });

    let mut table = HandleTable::new(1);
    let held = table.insert(0u8).unwrap();
    assert_eq!(table.status(held), HandleStatus::Live);
    assert_eq!(table.status(foreign), expected(1));
    table.remove(held);
    assert_eq!(table.status(held), HandleStatus::Stale);
}

/// A slot about to exhaust retires permanently and never reissues, whatever the other slots do.
/// 即将耗尽的槽位永久退役、不再签发；全零句柄与非法首代永远被拒。
pub fn assert_handle_retirement(index_probe: u32) {
    let last = HANDLE_GENERATION_RETIRED - 1;
    assert!(HandleAllocator::with_first_generation(1, 2).is_none());
    assert!(HandleAllocator::with_first_generation(1, HANDLE_GENERATION_RETIRED).is_none());
    assert!(HandleAllocator::with_first_generation(1, u32::MAX).is_none());
    let mut allocator = HandleAllocator::with_first_generation(1, last).unwrap();
    assert!(!allocator.is_live(Handle { index: 0, generation: 0 }));
    let handle = allocator.allocate().unwrap();
    assert_eq!(handle, Handle { index: 0, generation: last });
    assert!(allocator.allocate().is_none());
    assert!(!allocator.release(Handle { index: index_probe | 1, generation: last }));
    assert!(allocator.release(handle));
    assert!(!allocator.release(handle));
    assert!(allocator.allocate().is_none());
    assert!(!allocator.is_live(handle));
    assert!(!allocator.is_live(Handle { index: 0, generation: HANDLE_GENERATION_RETIRED }));
    assert_eq!(allocator.status(handle), HandleStatus::Stale);
    assert_eq!(allocator.status(Handle { index: 0, generation: 0 }), HandleStatus::Invalid);
    assert_eq!(allocator.status(Handle { index: 0, generation: HANDLE_GENERATION_RETIRED }), HandleStatus::Invalid);
    assert_eq!(allocator.status(Handle { index: 0, generation: u32::MAX }), HandleStatus::Invalid);
    assert_eq!(allocator.capacity(), 1);
}

/// Counts live tokens so a dropped or leaked stage value cannot go unnoticed.
struct FrameToken<'a>(&'a std::cell::Cell<i32>, u64);
impl<'a> FrameToken<'a> {
    fn new(live: &'a std::cell::Cell<i32>, value: u64) -> Self {
        live.set(live.get() + 1);
        Self(live, value)
    }
}
impl Drop for FrameToken<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

/// Real stage ordering against the exact model: each stage runs at most once, the builder runs
/// only at the exact successor and receives that source, and only `Ok` carries a candidate.
/// 真实阶段顺序对照精确模型：每阶段至多一次，构造仅在精确后继时运行且收到该来源，只有 Ok 带候选。
pub fn assert_scene_frame_order(source_ok: bool, source_version: u64, next: u64, candidate_ready: bool, token: u64) {
    let live = std::cell::Cell::new(0);
    let (applied, built) = (std::cell::Cell::new(0u8), std::cell::Cell::new(0u8));
    let result = order_scene_frame(
        next,
        || {
            applied.set(applied.get() + 1);
            assert_eq!(built.get(), 0);
            (source_ok, source_version, FrameToken::new(&live, token))
        },
        |source: FrameToken<'_>| {
            built.set(built.get() + 1);
            assert_eq!(applied.get(), 1);
            assert_eq!(source.1, token);
            candidate_ready.then(|| FrameToken::new(&live, source.1))
        },
    );
    let admitted = source_ok && source_version == next;
    assert_eq!(applied.get(), 1);
    assert_eq!(built.get(), admitted as u8);
    match result {
        Ok(candidate) => {
            assert!(admitted && candidate_ready);
            assert_eq!(candidate.1, token);
            assert_eq!(live.get(), 1);
        }
        Err(error) => {
            let expected = if admitted { SceneUpdateError::NativeRejected } else { SceneUpdateError::SourceRejected };
            assert_eq!(error, expected);
            assert!(!(admitted && candidate_ready));
            assert_eq!(live.get(), 0);
        }
    }
    assert_eq!(live.get(), 0);
}

/// A panicking stage unwinds out of the real kernel: the later stage never runs, the detached
/// values are dropped once, and an active value only assigned after `Ok` stays untouched.
/// 阶段 panic 穿出真实内核：后一阶段不运行，独立值恰释放一次，仅在 Ok 后赋值的活动值不变。
/// Not reachable under cargo-fuzz (its hook aborts on panic) or Kani (no unwinding).
#[cfg(test)]
pub fn assert_scene_frame_panic(panic_in_source: bool, source_version: u64, token: u64) {
    let live = std::cell::Cell::new(0);
    let built = std::cell::Cell::new(false);
    let mut active = token;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let candidate = order_scene_frame(
            source_version,
            || {
                let source = FrameToken::new(&live, token);
                if panic_in_source {
                    std::panic::resume_unwind(Box::new("source stage"));
                }
                (true, source_version, source)
            },
            |source: FrameToken<'_>| -> Option<u64> {
                built.set(true);
                let _partial = FrameToken::new(&live, source.1);
                std::panic::resume_unwind(Box::new("candidate stage"))
            },
        );
        active = candidate.map_or(active, |value| value ^ 1);
    }));
    assert!(outcome.is_err());
    assert_eq!(built.get(), !panic_in_source);
    assert_eq!(live.get(), 0);
    assert_eq!(active, token);
}

// Stand-ins for the three non-tree field types of the extracted `Node`; the kernel never reads
// them, and each carries a value so a mix-up between nodes is visible to the oracle.
// 提取出的 Node 的三个非树字段类型的替身；内核不读取它们，各带一个值，节点间串位时规格能发现。
#[derive(Clone, Debug, PartialEq)]
pub struct NodeKind(u8);
#[derive(Clone, Debug, PartialEq)]
pub struct Style(u8);
#[derive(Clone, Debug, PartialEq)]
pub struct Semantics(u8);

fn removal_leaf(id: u64, children: Vec<Node>) -> Node {
    let tag = id as u8;
    Node {
        id: NodeId(id),
        kind: NodeKind(tag),
        style: Style(tag ^ 1),
        semantics: Semantics(tag ^ 2),
        scroll: Point { x: tag as f32, y: -(tag as f32) },
        children,
    }
}

/// Builds a tree in preorder: each pair is an identity and how many of the following nodes stay
/// under it. 按前序建树：每对是身份及其后留在其下的节点数。
fn removal_tree(id: u64, spec: &mut std::slice::Iter<'_, (u8, u8)>, room: &mut usize) -> Node {
    let mut children = Vec::new();
    while *room > 0 {
        let Some(&(child, stay)) = spec.next() else { break };
        *room -= 1;
        let mut inner = (stay as usize % 4).min(*room);
        *room -= inner;
        children.push(removal_tree(child as u64 % 8, spec, &mut inner));
        *room += inner;
    }
    removal_leaf(id, children)
}

// Independent functional oracle: rebuilds the expected tree, shares no loop with the kernel.
// 独立函数式规格：重建期望树，不与内核共用循环。
fn removal_oracle(node: &Node, id: u64) -> Option<Node> {
    if let Some(at) = node.children.iter().position(|child| child.id.0 == id) {
        let mut rebuilt = node.clone();
        rebuilt.children = node.children[..at].iter().chain(&node.children[at + 1..]).cloned().collect();
        return Some(rebuilt);
    }
    node.children.iter().enumerate().find_map(|(at, child)| {
        removal_oracle(child, id).map(|changed| {
            let mut rebuilt = node.clone();
            rebuilt.children[at] = changed;
            rebuilt
        })
    })
}

/// Real removal against the oracle on an arbitrary tree, with the root's own identity included in
/// the identity domain. 真实移除在任意树上对照规格，身份域包含根自身。
pub fn assert_scene_node_remove(tree: Node, id: u64) {
    let expected = removal_oracle(&tree, id);
    let mut actual = tree.clone();
    assert_eq!(remove_node(&mut actual, NodeId(id)), expected.is_some());
    assert_eq!(actual, expected.unwrap_or(tree));
}

/// The same removal on a small tree given as first-level identities with an optional child, read
/// back node by node so the check does not clone trees.
/// 同一移除作用于小树（第一层身份加可选子节点），逐节点读回，检查不克隆树。
pub fn assert_scene_node_remove_shape(root: u64, shape: &[(u64, Option<u64>)], id: u64) {
    let leaf = |at: u64| removal_leaf(at, Vec::new());
    let mut children = Vec::new();
    let mut at = 0;
    while at < shape.len() {
        let (first, below) = shape[at];
        children.push(removal_leaf(first, below.map(leaf).into_iter().collect()));
        at += 1;
    }
    let mut tree = removal_leaf(root, children);
    let hit = remove_node(&mut tree, NodeId(id));
    // A direct child wins over any nested match; the earliest of either kind wins over later ones.
    // 直接子节点优先于任何嵌套匹配；同类中最早的优先。
    let direct = (0..shape.len()).find(|&at| shape[at].0 == id);
    let nested = (0..shape.len()).find(|&at| shape[at].1 == Some(id));
    assert_eq!(hit, direct.is_some() || nested.is_some());
    let same = |node: &Node, at: u64| {
        node.id.0 == at && node.kind.0 == at as u8 && node.style.0 == at as u8 ^ 1 && node.semantics.0 == at as u8 ^ 2
    };
    assert!(same(&tree, root));
    assert_eq!(tree.children.len() + direct.is_some() as usize, shape.len());
    let mut read = 0;
    let mut at = 0;
    while at < shape.len() {
        if direct != Some(at) {
            let (first, below) = shape[at];
            let kept = if direct.is_none() && nested == Some(at) { None } else { below };
            assert!(same(&tree.children[read], first));
            assert_eq!(tree.children[read].children.len(), kept.is_some() as usize);
            if let Some(child) = kept {
                assert!(same(&tree.children[read].children[0], child));
                assert!(tree.children[read].children[0].children.is_empty());
            }
            read += 1;
        }
        at += 1;
    }
}

pub fn assert_scene_node_remove_bytes(spec: &[(u8, u8)], root: u8, id: u8) {
    let mut room = spec.len();
    let tree = removal_tree(root as u64 % 8, &mut spec.iter(), &mut room);
    assert_scene_node_remove(tree, id as u64 % 8);
}

// Independent preorder walk with an explicit stack: the path to the first match, sharing no
// recursion or loop with the kernel. 独立的显式栈前序遍历：到首个匹配的路径，不与内核共用递归或循环。
fn find_oracle_path(root: &Node, id: u64) -> Option<Vec<usize>> {
    let mut pending = vec![(root, Vec::new())];
    while let Some((node, path)) = pending.pop() {
        if node.id.0 == id {
            return Some(path);
        }
        for (at, child) in node.children.iter().enumerate().rev() {
            let mut below = path.clone();
            below.push(at);
            pending.push((child, below));
        }
    }
    None
}

/// Real lookups against the oracle on an arbitrary tree: both agree on the very same node, and a
/// write through the loan changes that node and nothing else.
/// 真实查找在任意树上对照规格：两条路径指向同一节点，经借用写入只改变该节点。
pub fn assert_scene_node_find(tree: Node, id: u64) {
    let path = find_oracle_path(&tree, id);
    let target = path.as_ref().map(|path| path.iter().fold(&tree, |node, &at| &node.children[at]));
    match (find_node(&tree, NodeId(id)), target) {
        (Some(found), Some(target)) => assert!(std::ptr::eq(found, target)),
        (None, None) => {}
        (found, target) => panic!("read lookup {found:?} against {target:?}"),
    }
    let mut actual = tree.clone();
    let mut expected = tree.clone();
    let written = |node: &mut Node| {
        if id % 2 == 0 {
            *node = removal_leaf(100, vec![removal_leaf(101, Vec::new())]);
        } else {
            node.scroll = Point { x: -1.5, y: -2.5 };
        }
    };
    match find_node_mut(&mut actual, NodeId(id)) {
        Some(found) => {
            assert_eq!(&*found, target.expect("a hit needs a match"));
            written(found);
            written(path.unwrap().iter().fold(&mut expected, |node, &at| &mut node.children[at]));
        }
        None => assert!(path.is_none()),
    }
    assert_eq!(actual, expected);
}

pub fn assert_scene_node_find_bytes(spec: &[(u8, u8)], root: u8, id: u8) {
    let mut room = spec.len();
    let tree = removal_tree(root as u64 % 8, &mut spec.iter(), &mut room);
    assert_scene_node_find(tree, id as u64 % 8);
}

/// The same lookups on a small tree, read back node by node so the check does not clone trees.
/// 同一查找作用于小树，逐节点读回，检查不克隆树。
pub fn assert_scene_node_find_shape(root: u64, shape: &[(u64, Option<u64>)], id: u64) {
    let leaf = |at: u64| removal_leaf(at, Vec::new());
    let mut children = Vec::new();
    let mut at = 0;
    while at < shape.len() {
        let (first, below) = shape[at];
        children.push(removal_leaf(first, below.map(leaf).into_iter().collect()));
        at += 1;
    }
    let mut tree = removal_leaf(root, children);
    // The match in preorder, as (child, inside it). 前序首个匹配，记为（子节点，是否在其内）。
    let mut hit = None;
    let mut at = 0;
    while hit.is_none() && root != id && at < shape.len() {
        if shape[at].0 == id {
            hit = Some((at, false));
        } else if shape[at].1 == Some(id) {
            hit = Some((at, true));
        }
        at += 1;
    }
    let at_root = root == id;
    let expected_hit = at_root || hit.is_some();
    assert_eq!(find_node(&tree, NodeId(id)).map(|found| found.id.0 == id), expected_hit.then_some(true));
    let read = find_node(&tree, NodeId(id)).map(|found| found as *const Node);
    let lent = find_node_mut(&mut tree, NodeId(id));
    assert_eq!(lent.is_some(), expected_hit);
    assert_eq!(lent.as_ref().map(|found| &**found as *const Node), read);
    if let Some(found) = lent {
        found.scroll = Point { x: -1.5, y: -2.5 };
    }
    let check = |node: &Node, at: u64, marked: bool| {
        assert!(node.id.0 == at && node.kind.0 == at as u8 && node.style.0 == at as u8 ^ 1 && node.semantics.0 == at as u8 ^ 2);
        assert_eq!(node.scroll.x == -1.5, marked);
    };
    check(&tree, root, at_root);
    assert_eq!(tree.children.len(), shape.len());
    let mut at = 0;
    while at < shape.len() {
        let (first, below) = shape[at];
        check(&tree.children[at], first, hit == Some((at, false)));
        assert_eq!(tree.children[at].children.len(), below.is_some() as usize);
        if let Some(child) = below {
            check(&tree.children[at].children[0], child, hit == Some((at, true)));
        }
        at += 1;
    }
}

/// Real fold against an independent model: each patch in order, once, up to the first failure;
/// the result is that failure or the model's fold; the detached value is released exactly once.
/// 真实折叠对照独立模型：按序每补丁一次直到首个失败；结果是该失败或模型折叠值；独立值恰释放一次。
pub fn assert_scene_batch_stage(start: u64, patches: &[u8]) {
    let fails = |patch: u8| patch % 5 == 0;
    let step = |state: u64, patch: u8| state.wrapping_mul(31).wrapping_add(patch as u64);
    let live = std::cell::Cell::new(0);
    let calls = std::cell::Cell::new(0usize);
    let result = stage_scene_batch(FrameToken::new(&live, start), patches, |staged: FrameToken<'_>, patch: &u8| {
        assert_eq!(*patch, patches[calls.get()]);
        calls.set(calls.get() + 1);
        if fails(*patch) {
            return Err((staged.1, *patch));
        }
        Ok(FrameToken::new(&live, step(staged.1, *patch)))
    });
    let mut expected = start;
    let mut failure = None;
    let mut applied = 0;
    while applied < patches.len() && failure.is_none() {
        if fails(patches[applied]) {
            failure = Some((expected, patches[applied]));
        } else {
            expected = step(expected, patches[applied]);
        }
        applied += 1;
    }
    assert_eq!(calls.get(), applied);
    match (result, failure) {
        (Ok(end), None) => {
            assert_eq!(end.1, expected);
            assert_eq!(live.get(), 1);
        }
        (Err(error), Some(first)) => {
            assert_eq!(error, first);
            assert_eq!(live.get(), 0);
        }
        _ => panic!("stage outcome differs from the model"),
    }
    assert_eq!(live.get(), 0);
}

/// A panicking application unwinds out of the real fold: later patches never run, every detached
/// value is released once, and an active value only assigned after `Ok` stays untouched.
/// 应用 panic 穿出真实折叠：后续补丁不运行，独立值恰释放一次，仅在 Ok 后赋值的活动值不变。
#[cfg(test)]
pub fn assert_scene_batch_panic(start: u64, patches: &[u8], panic_at: usize) {
    let live = std::cell::Cell::new(0);
    let calls = std::cell::Cell::new(0usize);
    let mut active = start;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let next = stage_scene_batch(FrameToken::new(&live, start), patches, |staged: FrameToken<'_>, patch: &u8| -> Result<FrameToken<'_>, ()> {
            let index = calls.get();
            calls.set(index + 1);
            if index == panic_at {
                std::panic::resume_unwind(Box::new("patch application"));
            }
            Ok(FrameToken::new(&live, staged.1 ^ *patch as u64 ^ 1))
        });
        active = next.map_or(active, |value| value.1);
    }));
    assert_eq!(outcome.is_err(), panic_at < patches.len());
    assert_eq!(calls.get(), patches.len().min(panic_at + 1));
    assert_eq!(live.get(), 0);
    if outcome.is_err() {
        assert_eq!(active, start);
    }
}

/// One candidate write as the model records it. 模型记录的一次候选写入。
#[derive(Clone, Copy, Debug, PartialEq)]
enum RestoreWrite {
    Viewport(u64, u64),
    Press(u8),
    Focus(u8),
    Hover(u8),
    Resolve,
    Adopt(Option<u8>, u64),
}

/// Recording candidate: admission is a bit mask, and the write numbered `fail_at` fails.
/// 记录型候选：准入是位掩码，编号为 `fail_at` 的写入失败。
struct RecordedRestore {
    writes: [Option<RestoreWrite>; 8],
    len: usize,
    press_mask: u8,
    focus_mask: u8,
    fail_at: u8,
    panic_at: u8,
    failed: bool,
}

impl RecordedRestore {
    fn write(&mut self, write: RestoreWrite) -> Option<()> {
        assert!(!self.failed);
        let number = self.len as u8;
        if number == self.panic_at {
            std::panic::resume_unwind(Box::new("candidate write"));
        }
        if number == self.fail_at {
            self.failed = true;
            return None;
        }
        self.writes[self.len] = Some(write);
        self.len += 1;
        Some(())
    }
}

impl<'a> RestoreCandidate<u8, FrameToken<'a>, u64, u8, FrameToken<'a>> for RecordedRestore {
    fn restore_viewport(&mut self, viewport: FrameToken<'a>, scroll: u64) -> Option<()> {
        self.write(RestoreWrite::Viewport(viewport.1, scroll))
    }
    fn press_admitted(&self, id: u8) -> bool {
        self.press_mask >> (id & 7) & 1 == 1
    }
    fn restore_press(&mut self, id: u8) -> Option<()> {
        self.write(RestoreWrite::Press(id))
    }
    fn focus_admitted(&self, id: u8) -> bool {
        self.focus_mask >> (id & 7) & 1 == 1
    }
    fn restore_focus(&mut self, id: u8) -> Option<()> {
        self.write(RestoreWrite::Focus(id))
    }
    fn restore_hover(&mut self, pointer: u8) -> Option<()> {
        self.write(RestoreWrite::Hover(pointer))
    }
    fn resolve_restored(&mut self) -> Option<()> {
        self.write(RestoreWrite::Resolve)
    }
    fn adopt_pointer_and_cache(&mut self, pointer: Option<u8>, frame_cache: FrameToken<'a>) {
        assert!(!self.failed);
        self.writes[self.len] = Some(RestoreWrite::Adopt(pointer, frame_cache.1));
        self.len += 1;
    }
}

/// The real replay against an independently written script: the writes are exactly its prefix up
/// to the failure, `None` only after a failed write, and both carried values are released once.
/// 真实重放对照独立脚本：写入恰为失败前的前缀，仅写失败后为 None，两个携带值都恰释放一次。
pub fn assert_scene_interaction_restore(viewport: u64, scroll: u64, pressed: Option<u8>, focused: Option<u8>, pointer: Option<u8>, cache: u64, press_mask: u8, focus_mask: u8, fail_at: u8) {
    let mut script = [None; 8];
    let mut count = 0;
    let mut push = |write| {
        script[count] = Some(write);
        count += 1;
    };
    push(RestoreWrite::Viewport(viewport, scroll));
    match pressed {
        Some(id) if press_mask >> (id & 7) & 1 == 1 => push(RestoreWrite::Press(id)),
        _ => {}
    }
    match focused {
        Some(id) if focus_mask >> (id & 7) & 1 == 1 => push(RestoreWrite::Focus(id)),
        _ => {}
    }
    if let Some(position) = pointer {
        push(RestoreWrite::Hover(position));
    }
    push(RestoreWrite::Resolve);
    push(RestoreWrite::Adopt(pointer, cache));
    // Adoption cannot fail, so only the writes before it are failure positions.
    // 采纳不会失败，只有它之前的写入才是失败位置。
    let fallible = count - 1;
    let live = std::cell::Cell::new(0);
    let mut candidate = RecordedRestore { writes: [None; 8], len: 0, press_mask, focus_mask, fail_at, panic_at: u8::MAX, failed: false };
    let result = restore_candidate_interaction(&mut candidate, CarriedInteraction {
        viewport: FrameToken::new(&live, viewport),
        scroll,
        pressed,
        focused,
        pointer,
        frame_cache: FrameToken::new(&live, cache),
    });
    assert_eq!(live.get(), 0);
    let fails = (fail_at as usize) < fallible;
    assert_eq!(result.is_none(), fails);
    assert_eq!(candidate.failed, fails);
    assert_eq!(candidate.len, if fails { fail_at as usize } else { count });
    assert_eq!(candidate.writes[..candidate.len], script[..candidate.len]);
}

/// A panicking candidate write unwinds out of the replay: nothing after it runs, both carried values
/// are released once, and the recorded writes stop at the panic.
/// 候选写入 panic 穿出重放：其后不再运行，两个携带值都恰释放一次，记录止于 panic。
#[cfg(test)]
pub fn assert_scene_interaction_restore_panic(viewport: u64, scroll: u64, cache: u64, panic_at: u8) {
    let live = std::cell::Cell::new(0);
    let mut candidate = RecordedRestore { writes: [None; 8], len: 0, press_mask: 0xff, focus_mask: 0xff, fail_at: u8::MAX, panic_at, failed: false };
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        restore_candidate_interaction(&mut candidate, CarriedInteraction {
            viewport: FrameToken::new(&live, viewport),
            scroll,
            pressed: Some(1),
            focused: Some(2),
            pointer: Some(3),
            frame_cache: FrameToken::new(&live, cache),
        })
    }));
    // Five fallible writes precede the infallible adoption. 不会失败的采纳之前有五次可失败写入。
    assert_eq!(outcome.is_err(), panic_at < 5);
    assert_eq!(candidate.len, if panic_at < 5 { panic_at as usize } else { 6 });
    assert_eq!(live.get(), 0);
}

/// Runs the real state against an array model of held buttons kept as (variant rank, code); the
/// expected drag button is the model's smallest entry, independent of the production set.
/// 以真实状态对照数组模型（变体序、码）；拖动按钮取模型最小项，不依赖生产集合。
pub fn assert_desktop_pointer_path(steps: &[DesktopPointerStep]) {
    let button_of = |tag: u8, code: u16| match tag % 6 {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        _ => MouseButton::Other(code),
    };
    let key_of = |tag: u8, code: u16| (tag % 6, if tag % 6 == 5 { code } else { 0 });
    let mut state = DesktopPointerState::new();
    let mut held: [Option<(u8, u16)>; 8] = [None; 8];
    let mut position = (0f32.to_bits(), 0f32.to_bits());
    let mut modifiers = Modifiers { shift: false, control: false, alt: false, meta: false };
    let mut step = 0;
    while step < steps.len() && step < held.len() {
        let (operation, tag, code, x, y, timestamp) = steps[step];
        step += 1;
        let button = button_of(tag, code);
        let key = key_of(tag, code);
        let (event, phase, expected_button, pressure) = match operation % 4 {
            0 => {
                position = (x, y);
                let event = state.moved_at(Point { x: f32::from_bits(x), y: f32::from_bits(y) }, timestamp);
                let mut smallest: Option<(u8, u16)> = None;
                let mut slot = 0;
                while slot < held.len() {
                    if let Some(entry) = held[slot] {
                        let lower = match smallest {
                            Some(known) => entry < known,
                            None => true,
                        };
                        if lower {
                            smallest = Some(entry);
                        }
                    }
                    slot += 1;
                }
                let phase = if smallest.is_some() { PointerPhase::Move } else { PointerPhase::Hover };
                (event, phase, smallest.map(|(rank, code)| button_of(rank, code)), 0.0f32)
            }
            1 => {
                let mut known = false;
                let mut free = held.len();
                let mut slot = 0;
                while slot < held.len() {
                    known |= held[slot] == Some(key);
                    if held[slot].is_none() {
                        free = slot;
                    }
                    slot += 1;
                }
                if !known && free < held.len() {
                    held[free] = Some(key);
                }
                (state.pressed_at(button, timestamp), PointerPhase::Down, Some(button), 1.0)
            }
            2 => {
                let mut slot = 0;
                while slot < held.len() {
                    if held[slot] == Some(key) {
                        held[slot] = None;
                    }
                    slot += 1;
                }
                (state.released_at(button, timestamp), PointerPhase::Up, Some(button), 0.0)
            }
            _ => {
                modifiers = Modifiers { shift: tag & 1 != 0, control: tag & 2 != 0, alt: tag & 4 != 0, meta: tag & 8 != 0 };
                state.set_modifiers(modifiers);
                assert_eq!(state.modifiers(), modifiers);
                continue;
            }
        };
        assert_eq!(event.phase, phase);
        assert_eq!(event.button, expected_button);
        assert_eq!(event.timestamp_millis, timestamp);
        assert_eq!((event.position.x.to_bits(), event.position.y.to_bits()), position);
        assert_eq!(event.pressure.to_bits(), pressure.to_bits());
        assert_eq!(event.modifiers, modifiers);
        assert_eq!(event.pointer_id, PointerId(0));
        assert_eq!(event.kind, PointerKind::Mouse);
        assert_eq!(event.is_primary, matches!(expected_button, Some(MouseButton::Left) | None));
        assert!(!event.eraser);
        assert_eq!(event.twist.to_bits(), 0);
        assert_eq!((event.tilt.x.to_bits(), event.tilt.y.to_bits()), (0, 0));
        assert_eq!((event.contact.width.to_bits(), event.contact.height.to_bits()), (0, 0));
    }
}

/// Independent glyph-skew oracle in the f64 domain: a transform equal to a pure horizontal skew
/// (a = d = 1.0, b = e = f = 0.0, either zero sign) has its skew negated; anything else, NaN
/// coefficients included, keeps every bit of c.
/// 浮点域独立斜切规格：等于纯水平斜切（a = d = 1.0，b = e = f = 0.0，零的符号不限）时斜切取反；
/// 其余（含 NaN 系数）c 的每一位不变。
pub fn assert_glyph_skew(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) {
    let [fa, fb, fd, fe, ff] = [a, b, d, e, f].map(f64::from_bits);
    let pure = fa == 1.0 && fb == 0.0 && fd == 1.0 && fe == 0.0 && ff == 0.0;
    let expected = if pure { (-f64::from_bits(c)).to_bits() } else { c };
    assert_eq!(upright_skew_x(a, b, c, d, e, f), expected);
}

/// A pure skew whose zero entries carry the given signs (bit i of `signs` negates zero i).
/// 纯斜切，零系数的符号由 `signs` 的各位给出。
pub fn assert_glyph_skew_pure(c: u64, signs: u8) {
    let zero = |bit: u8| if signs & bit != 0 { (-0.0f64).to_bits() } else { 0.0f64.to_bits() };
    let one = 1.0f64.to_bits();
    assert_glyph_skew(one, zero(1), c, one, zero(2), zero(4));
    assert_eq!(upright_skew_x(one, zero(1), c, one, zero(2), zero(4)), c ^ (1 << 63));
}

/// Independent frame-reuse oracle: reuse only a clean cache of identical extent whose
/// length equals the admitted packed length; any invalidation forces a repaint.
/// 独立帧复用规格：仅复用未失效、尺寸一致且长度等于准入紧凑长度的缓存；任何失效都强制重绘。
pub fn assert_frame_reuse(dirty: bool, cached: (u32, u32, usize), width: u32, height: u32, expected: usize) {
    let (cw, ch, len) = cached;
    let same = (cw, ch, len) == (width, height, expected);
    assert_eq!(frame_reuse_admits(dirty, cw, ch, len, width, height, expected), same && !dirty);
    assert!(!frame_reuse_admits(true, cw, ch, len, width, height, expected));
    assert_eq!(frame_reuse_admits(false, width, height, expected, width, height, expected), true);
}

/// Independent damage-span oracle: the result is the min/max hull of the old span and `at`.
/// 独立损伤区间规格：结果为旧区间与 `at` 的最小/最大包络。
pub fn assert_damage_span(span: Option<(u32, u32)>, at: u32) {
    let expected = match span {
        None => (at, at),
        Some((lo, hi)) => (lo.min(at), hi.max(at)),
    };
    let result = damage_span_extend(span, at);
    assert_eq!(result, expected);
    assert!(result.0 <= at && at <= result.1);
    assert_eq!(damage_span_extend(Some(result), at), result, "covering index is a fixed point");
}

/// Independent frame-damage oracle: per-pixel bounding box over a `width`-wide packed
/// RGBA8 frame; anything not comparable is Full.
/// 独立帧损伤规格：逐像素包围盒；不可比较者为 Full。
pub fn assert_frame_damage(previous: Option<(u32, u32, &[u8])>, width: u32, height: u32, next: &[u8]) {
    let comparable = match previous {
        Some((pw, ph, bytes)) => pw == width && ph == height && width > 0
            && bytes.len() == next.len()
            && (width as u64).checked_mul(height as u64).and_then(|n| n.checked_mul(4)) == Some(next.len() as u64),
        None => false,
    };
    let expected = if !comparable {
        FrameDamage::Full
    } else {
        let before = previous.unwrap().2;
        let mut hull: Option<(u32, u32, u32, u32)> = None;
        for y in 0..height {
            for x in 0..width {
                let i = ((y as usize * width as usize) + x as usize) * 4;
                if before[i..i + 4] != next[i..i + 4] {
                    hull = Some(match hull {
                        None => (x, y, x, y),
                        Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                    });
                }
            }
        }
        match hull {
            None => FrameDamage::Unchanged,
            Some((a, b, c, d)) => FrameDamage::Rect { x: a, y: b, width: c - a + 1, height: d - b + 1 },
        }
    };
    assert_eq!(frame_damage(previous, width, height, next), expected);
}


/// Independent in-row segment write oracle over widened integers.
/// 宽整数下的独立行内片段写入规格。
pub fn assert_segment(bytes: &[u8], row_bytes: usize, height: usize, stride: usize, row: usize,
    offset: usize, source: &[u8]) {
    let mut actual = bytes.to_vec();
    let end = height.saturating_sub(1) as u128 * stride as u128 + row_bytes as u128;
    let valid = row_bytes > 0 && height > 0 && stride >= row_bytes && row < height
        && end <= bytes.len() as u128 && offset as u128 + source.len() as u128 <= row_bytes as u128;
    assert_eq!(write_surface_segment(&mut actual, row_bytes, height, stride, row, offset, source), valid);
    let start = row as u128 * stride as u128 + offset as u128;
    for i in 0..bytes.len() {
        let inside = valid && start <= i as u128 && (i as u128) < start + source.len() as u128;
        let expected = if inside { source[(i as u128 - start) as usize] } else { bytes[i] };
        assert_eq!(actual[i], expected);
    }
}

/// Independent refresh-tick oracle by characterization: the tick is the unique instant at or
/// after `origin`, aligned to `interval`, in `(now - interval, now]`.
/// 刻画式独立刷新节拍规格：节拍是 `origin` 之后、按 `interval` 对齐、落在 `(now - interval, now]`
/// 中的唯一时刻。
pub fn assert_frame_tick(origin: u64, interval: u64, now: u64) {
    let result = frame_tick_at(origin, interval, now);
    if interval == 0 || now < origin {
        assert_eq!(result, None);
        return;
    }
    let t = result.expect("aligned tick exists");
    assert!(origin <= t && t <= now && now - t < interval);
    assert_eq!((t - origin) % interval, 0);
}

/// Independent frame-pacing oracle: idle when clean, wait for the first tick, one RenderNow
/// per period (the characterized tick), otherwise the next tick unless it leaves u64.
/// 独立帧节拍规格：无变化为 Idle，首节拍前等待，每周期一次 RenderNow（刻画所得节拍），
/// 否则等待下一节拍（超出 u64 为 None）。
pub fn assert_frame_pace(dirty: bool, presented: Option<u64>, origin: u64, interval: u64, now: u64) {
    let result = frame_pace(dirty, presented, origin, interval, now);
    let expected = if interval == 0 {
        None
    } else if !dirty {
        Some(FramePace::Idle)
    } else if now < origin {
        Some(FramePace::WaitUntil(origin))
    } else {
        assert_frame_tick(origin, interval, now);
        let tick = frame_tick_at(origin, interval, now).expect("tick characterized above");
        if !matches!(presented, Some(last) if last >= tick) {
            Some(FramePace::RenderNow(tick))
        } else {
            tick.checked_add(interval).map(FramePace::WaitUntil)
        }
    };
    assert_eq!(result, expected);
    if let Some(FramePace::RenderNow(tick)) = result {
        // Presenting that tick ends the period: asking again never renders twice.
        assert!(!matches!(frame_pace(dirty, Some(tick), origin, interval, now), Some(FramePace::RenderNow(_))));
    }
}

/// Independent source-text offset oracle: a layout offset maps only for an identical-length
/// layout, an in-range offset (end included) and a character boundary.
/// 独立源文本偏移规格：仅当布局长度相同、偏移在范围内（含末尾）且为字符边界时才映射。
pub fn assert_text_offset(layout_len: usize, source_len: usize, offset: usize, on_boundary: bool) {
    let expected = if layout_len != source_len {
        false
    } else if offset > source_len {
        false
    } else {
        on_boundary
    };
    assert_eq!(text_offset_admits(layout_len, source_len, offset, on_boundary), expected);
}

/// Independent source-text range oracle: a layout range maps only for an identical-length
/// layout, an ordered range ending in range (end included) and character boundaries at both ends.
/// 独立源文本区间规格：仅当布局长度相同、区间有序且终点在范围内（含末尾）、两端均为字符边界时才映射。
pub fn assert_text_range(layout_len: usize, source_len: usize, start: usize, end: usize, start_on_boundary: bool, end_on_boundary: bool) {
    let expected = if layout_len != source_len || start > end || end > source_len {
        false
    } else {
        start_on_boundary && end_on_boundary
    };
    assert_eq!(text_range_admits(layout_len, source_len, start, end, start_on_boundary, end_on_boundary), expected);
    // A range admits exactly when both ends admit as offsets and the range is ordered.
    let both = text_offset_admits(layout_len, source_len, start, start_on_boundary)
        && text_offset_admits(layout_len, source_len, end, end_on_boundary);
    assert_eq!(expected, both && start <= end);
}

/// Independent GPU readback layout oracle over widened integers: admitted extents are the
/// CPU-path ones, rows pad to the next multiple of 256 bytes and the buffer is padded × height.
/// 独立 GPU 读回布局规格（加宽整数）：准入尺寸与 CPU 路径相同，行填充到下一个 256 字节倍数，
/// 缓冲为填充行 × 高度。
pub fn assert_gpu_readback_layout(width: u32, height: u32) {
    let result = gpu_readback_layout(width, height);
    if width == 0 || height == 0 || width > 65535 || height > 65535 {
        assert_eq!(result, None);
        return;
    }
    let (padded, bytes) = result.expect("admitted extent has a readback layout");
    let row = width as u64 * 4;
    assert_eq!(padded as u64 % 256, 0);
    assert!(padded as u64 >= row && (padded as u64) < row + 256);
    assert_eq!(bytes as u128, padded as u128 * height as u128);
}

/// Independent accessibility-action oracle: a disabled node offers nothing; otherwise focus
/// follows focusability and activation follows the button role.
/// 独立无障碍动作规格：禁用节点无动作；否则聚焦取决于可聚焦性，激活取决于按钮角色。
pub fn assert_access_actions(is_button: bool, focusable: bool, disabled: bool) {
    let actions = access_actions(is_button, focusable, disabled);
    let expected = if disabled { (false, false) } else { (focusable, is_button) };
    assert_eq!((actions.focus, actions.activate), expected);
    // Same answer as the engine's admission rules: focus needs focusable and enabled, activation
    // needs an enabled button.
    assert_eq!(actions.focus, focusable && !disabled);
    assert_eq!(actions.activate, is_button && !disabled);
}

/// Independent node-change oracle: absence in the newer tree wins, then absence in the older
/// tree; content equality decides only for nodes in both.
/// 独立节点差别规格：新树中没有优先，其次旧树中没有；内容相等只对两树都有的节点起作用。
pub fn assert_access_change(in_previous: bool, in_next: bool, equal: bool) {
    let expected = match (in_previous, in_next) {
        (false, false) => AccessChange::Absent,
        (false, true) => AccessChange::Added,
        (true, false) => AccessChange::Removed,
        (true, true) => if equal { AccessChange::Unchanged } else { AccessChange::Changed },
    };
    assert_eq!(access_change(in_previous, in_next, equal), expected);
    // Equality is irrelevant unless the node is in both trees.
    if !(in_previous && in_next) {
        assert_eq!(access_change(in_previous, in_next, !equal), expected);
    }
}

/// Independent action-request oracle: a request is admitted only by the matching reported
/// action; unsupported requests never are, whatever the node offers.
/// 独立动作请求规格：请求只由对应的已报告动作准入；不支持的请求无论节点提供什么都不准入。
pub fn assert_access_request(kind: u8, focus: bool, activate: bool) {
    let request = match kind % 3 {
        0 => AccessRequest::Focus,
        1 => AccessRequest::Activate,
        _ => AccessRequest::Unsupported,
    };
    let expected = (request == AccessRequest::Focus && focus) || (request == AccessRequest::Activate && activate);
    assert_eq!(access_request_admits(request, focus, activate), expected);
    // Offering both actions admits every supported request and still no unsupported one.
    assert_eq!(access_request_admits(request, true, true), request != AccessRequest::Unsupported);
    assert!(!access_request_admits(request, false, false));
}

/// Independent damage-rectangle admission oracle over widened integers.
/// 宽整数下的独立损伤矩形准入规格。
pub fn assert_damage_rect(x: u32, y: u32, rect_width: u32, rect_height: u32, width: u32, height: u32) {
    let expected = rect_width > 0 && rect_height > 0
        && x as u64 + rect_width as u64 <= width as u64 && y as u64 + rect_height as u64 <= height as u64;
    assert_eq!(damage_rect_admits(x, y, rect_width, rect_height, width, height), expected);
}

/// Independent damage-only write oracle: when every admission holds, exactly the pixels
/// inside the damage (all for Full, none for Unchanged) take the packed frame's values and
/// every other byte (other pixels, padding, tail) is kept; otherwise nothing changes.
/// 独立只写损伤规格：全部准入成立时，恰好损伤内像素（Full 全部、Unchanged 无）取紧凑帧值，
/// 其余字节（其他像素、padding、尾部）保持；否则不变。
pub fn assert_damage_write(destination: &[u8], stride: usize, width: u32, height: u32, pixels: &[u8],
    damage: FrameDamage) {
    let row = width as u128 * 4;
    let surface = width > 0 && height > 0 && row <= destination.len() as u128 && stride as u128 >= row
        && (height as u128 - 1) * stride as u128 + row <= destination.len() as u128
        && row * height as u128 == pixels.len() as u128;
    let rect = match damage {
        FrameDamage::Unchanged => Some((0, 0, 0, 0)),
        FrameDamage::Full => Some((0, 0, width, height)),
        FrameDamage::Rect { x, y, width: w, height: h } => (w > 0 && h > 0
            && x as u64 + w as u64 <= width as u64 && y as u64 + h as u64 <= height as u64).then_some((x, y, w, h)),
    };
    let valid = surface && rect.is_some();
    let mut actual = destination.to_vec();
    assert_eq!(write_damaged_frame(&mut actual, stride, width, height, pixels, damage), valid);
    let mut expected = destination.to_vec();
    if valid {
        let (x0, y0, w, h) = rect.unwrap();
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                let to = y as usize * stride + x as usize * 4;
                let from = (y as usize * width as usize + x as usize) * 4;
                expected[to..to + 4].copy_from_slice(&pixels[from..from + 4]);
            }
        }
    }
    assert_eq!(actual, expected);
}

/// End-to-end: a surface holding `before` plus the damage `frame_damage` reports for `after`
/// converges on `after`, padding untouched.
/// 端到端：持有 `before` 的 Surface 经 `frame_damage` 报告的损伤写入后收敛到 `after`，padding 不变。
pub fn assert_damage_convergence(before: &[u8], after: &[u8], width: u32, height: u32, padding: usize) {
    let row = width as usize * 4;
    let stride = row + padding;
    let rows = height as usize;
    if width == 0 || rows == 0 || before.len() != row * rows || after.len() != row * rows { return; }
    let mut surface = vec![0xAB; stride * (rows - 1) + row];
    for y in 0..rows { surface[y * stride..y * stride + row].copy_from_slice(&before[y * row..(y + 1) * row]); }
    let damage = frame_damage(Some((width, height, before)), width, height, after);
    assert_damage_write(&surface, stride, width, height, after, damage);
    assert!(write_damaged_frame(&mut surface, stride, width, height, after, damage));
    for y in 0..rows {
        assert_eq!(&surface[y * stride..y * stride + row], &after[y * row..(y + 1) * row]);
        if y + 1 < rows { assert!(surface[y * stride + row..(y + 1) * stride].iter().all(|b| *b == 0xAB)); }
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
    fn damage_span_hull() {
        assert_damage_span(kani::any(), kani::any());
    }

    /// Concrete extent, symbolic content: any subset of pixels changed in any lane to any value;
    /// mismatched previous width, absent previous frame and a short next frame included.
    fn frame_damage_for_extent(width: u32, height: u32) {
        let len = (width * height * 4) as usize;
        let before = [7u8; 16];
        let mut after = before;
        for pixel in 0..(width * height) as usize {
            if kani::any() {
                let lane: usize = kani::any();
                kani::assume(lane < 4);
                after[pixel * 4 + lane] = kani::any();
            }
        }
        let previous_width: u32 = kani::any();
        let next_len = if kani::any() { len.saturating_sub(4) } else { len };
        let previous = if kani::any() { Some((previous_width, height, &before[..len])) } else { None };
        assert_frame_damage(previous, width, height, &after[..next_len]);
    }

    #[kani::proof]
    #[kani::unwind(18)]
    fn frame_damage_bounding_box_2x2() {
        frame_damage_for_extent(2, 2);
    }

    #[kani::proof]
    #[kani::unwind(18)]
    fn frame_damage_bounding_box_thin() {
        let wide: bool = kani::any();
        if wide { frame_damage_for_extent(2, 1) } else { frame_damage_for_extent(1, 2) }
    }

    /// A clock value either near zero or near u64::MAX, so both ends of the range are covered.
    /// 时钟值取零附近或 u64::MAX 附近，覆盖两端。
    fn clock() -> u64 {
        let low: u16 = kani::any();
        if kani::any() { low as u64 } else { u64::MAX - low as u64 }
    }

    /// `base` moved by a small symbolic distance either way (no wrap): keeps `now - origin`
    /// narrow so the 64-bit divider stays tractable; full-width symbolic `%` timed out.
    /// `base` 向任一方向移动较小的符号距离（不回绕）：使 `now - origin` 位宽较窄、64 位除法器可解；
    /// 全宽符号 `%` 超时。
    fn near(base: u64) -> u64 {
        near_by(base, kani::any::<u16>() as u64)
    }

    fn near_by(base: u64, delta: u64) -> u64 {
        let moved = if kani::any() { base.checked_add(delta) } else { base.checked_sub(delta) };
        kani::assume(moved.is_some());
        moved.unwrap()
    }

    #[kani::proof]
    fn frame_tick_alignment() {
        let origin = clock();
        assert_frame_tick(origin, kani::any::<u16>() as u64, near(origin));
    }

    #[kani::proof]
    fn frame_tick_alignment_wide_period() {
        let interval = if kani::any() { u64::MAX } else { u64::MAX - kani::any::<u8>() as u64 };
        assert_frame_tick(clock(), interval, clock());
    }

    #[kani::proof]
    fn frame_pace_decision() {
        // Narrower than the tick harness: this one runs the divider four times.
        // 比节拍 harness 更窄：此处除法器执行四次。
        let origin = clock();
        let now = near_by(origin, kani::any::<u8>() as u64);
        let presented = if kani::any() { Some(near_by(now, kani::any::<u8>() as u64)) } else { None };
        assert_frame_pace(kani::any(), presented, origin, kani::any::<u8>() as u64, now);
    }

    #[kani::proof]
    fn frame_pace_overflow() {
        let interval = u64::MAX - kani::any::<u8>() as u64;
        let presented = if kani::any() { Some(clock()) } else { None };
        assert_frame_pace(true, presented, clock(), interval, clock());
    }

    #[kani::proof]
    fn text_offset_admission() {
        assert_text_offset(kani::any(), kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    fn text_range_admission() {
        assert_text_range(kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    fn gpu_readback_layout_admission() {
        assert_gpu_readback_layout(kani::any(), kani::any());
    }

    #[kani::proof]
    fn access_actions_rules() {
        assert_access_actions(kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    fn access_change_rules() {
        assert_access_change(kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    fn access_request_rules() {
        assert_access_request(kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    fn damage_rect_admission() {
        assert_damage_rect(kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn eight_byte_segment_write() {
        let bytes: [u8; 8] = kani::any();
        let source: [u8; 2] = kani::any();
        let take: bool = kani::any();
        let row_bytes: u8 = kani::any();
        let height: u8 = kani::any();
        let stride: u8 = kani::any();
        let offset: u8 = kani::any();
        assert_segment(&bytes, row_bytes as usize, height as usize, stride as usize, kani::any(),
            offset as usize, if take { &source[..] } else { &source[..1] });
    }

    /// Concrete geometry and damage per branch: every damage kind and a fixed set of rectangles
    /// (inside, edge, whole, empty, out of bounds, overflowing). Content is concrete but pairwise
    /// distinct, so any misplaced copy is observable. Symbolic rectangle coordinates made the
    /// unrolled 64-bit offset arithmetic exceed the time/memory budget (2x2 exhausted CBMC
    /// memory), so each case is a separate constant call; arbitrary rectangles up to 5x5 frames
    /// are covered by Proptest.
    /// 每个分支几何与损伤均固定：全部损伤种类与一组固定矩形（内部、边缘、整帧、空、越界、溢出）。
    /// 内容固定但两两不同，任何错位复制都可观测。符号矩形坐标使展开后的 64 位偏移运算超出时间/内存
    /// 预算（2×2 耗尽 CBMC 内存），故逐例常量调用；5×5 以内任意矩形由 Proptest 覆盖。
    fn damage_write_case(width: u32, height: u32, padding: usize, cut: usize, short: bool) {
        let stride = width as usize * 4 + padding;
        let destination: [u8; 10] = core::array::from_fn(|i| 200 + i as u8);
        let pixels: [u8; 8] = core::array::from_fn(|i| 100 + i as u8);
        let dest_len = stride * (height as usize - 1) + width as usize * 4 - cut;
        let packed = (width * height * 4) as usize - if short { 4 } else { 0 };
        let (destination, pixels) = (&destination[..dest_len], &pixels[..packed]);
        let rect = |x, y, w, h| FrameDamage::Rect { x, y, width: w, height: h };
        match kani::any::<u8>() {
            0 => assert_damage_write(destination, stride, width, height, pixels, FrameDamage::Unchanged),
            1 => assert_damage_write(destination, stride, width, height, pixels, FrameDamage::Full),
            2 => assert_damage_write(destination, stride, width, height, pixels, rect(0, 0, 1, 1)),
            3 => assert_damage_write(destination, stride, width, height, pixels, rect(1, 0, 1, 1)),
            4 => assert_damage_write(destination, stride, width, height, pixels, rect(0, 1, 1, 1)),
            5 => assert_damage_write(destination, stride, width, height, pixels, rect(0, 0, width, height)),
            6 => assert_damage_write(destination, stride, width, height, pixels, rect(0, 0, 0, 1)),
            7 => assert_damage_write(destination, stride, width, height, pixels, rect(0, 0, 1, 0)),
            8 => assert_damage_write(destination, stride, width, height, pixels, rect(0, 0, width + 1, 1)),
            9 => assert_damage_write(destination, stride, width, height, pixels, rect(0, height, 1, 1)),
            _ => assert_damage_write(destination, stride, width, height, pixels, rect(u32::MAX, 0, 2, 1)),
        }
    }

    #[kani::proof]
    #[kani::unwind(11)]
    fn damage_only_write_thin() {
        if kani::any() { damage_write_case(2, 1, 1, 0, false) } else { damage_write_case(1, 2, 1, 0, false) }
    }

    #[kani::proof]
    #[kani::unwind(11)]
    fn damage_only_write_rejections() {
        match kani::any::<u8>() % 4 {
            0 => damage_write_case(1, 2, 0, 1, false),
            1 => damage_write_case(2, 1, 1, 0, true),
            2 => damage_write_case(1, 2, 2, 0, false),
            _ => damage_write_case(1, 1, 0, 0, false),
        }
    }

    #[kani::proof]
    fn frame_reuse_admission() {
        assert_frame_reuse(kani::any(), (kani::any(), kani::any(), kani::any()), kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    fn color_construct_exact() {
        assert_color_construct(kani::any(), kani::any(), kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    fn desktop_timestamp_saturation() {
        assert_desktop_timestamp(kani::any());
    }

    // Real state and Vec, arbitrary buttons, positions and timestamps; no clock in the state path.
    // 真实状态与 Vec，任意按钮、坐标与时间戳；状态路径中没有时钟。
    #[kani::proof]
    #[kani::unwind(17)]
    fn desktop_pointer_state_path() {
        let press = || -> DesktopPointerStep { (1, kani::any(), kani::any(), 0, 0, kani::any()) };
        let motion = || -> DesktopPointerStep { (0, 0, 0, kani::any(), kani::any(), kani::any()) };
        let release = || -> DesktopPointerStep { (2, kani::any(), kani::any(), 0, 0, kani::any()) };
        let modifiers: DesktopPointerStep = (3, kani::any(), 0, 0, 0, 0);
        assert_desktop_pointer_path(&[press(), modifiers, press(), motion(), release(), motion(), release(), motion()]);
    }

    // Arbitrary stage outcomes through the real kernel; unwinding is outside Kani's model.
    // 任意阶段结果穿过真实内核；展开不在 Kani 模型内。
    #[kani::proof]
    fn scene_frame_order() {
        assert_scene_frame_order(kani::any(), kani::any(), kani::any(), kani::any(), kani::any());
    }

    // Arbitrary identities on one root with one childless child, the only shape that finished:
    // recursive `Vec<Node>` drop glue grows with the bound, and larger shapes timed out with no
    // verdict. 根加一个无子节点的子节点上的任意身份，这是唯一跑完的形状：递归 Vec<Node> 的 drop 展开随界增长，更大的形状超时且无判定。
    #[kani::proof]
    #[kani::unwind(2)]
    fn scene_node_remove() {
        let child: u64 = kani::any();
        assert_scene_node_remove_shape(kani::any(), &[(child, None)], kani::any());
    }

    // The same smallest shape as the removal: root plus one childless child, any identities.
    // 与移除相同的最小形状：根加一个无子节点的子节点，身份任意。
    #[kani::proof]
    #[kani::unwind(2)]
    fn scene_node_find() {
        let child: u64 = kani::any();
        assert_scene_node_find_shape(kani::any(), &[(child, None)], kani::any());
    }

    // Arbitrary start and patches of every length up to four through the real fold; unwinding is
    // outside Kani's model. 任意起点与至多四个任意补丁穿过真实折叠；展开不在 Kani 模型内。
    #[kani::proof]
    #[kani::unwind(6)]
    fn scene_batch_stage() {
        let patches: [u8; 4] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= patches.len());
        assert_scene_batch_stage(kani::any(), &patches[..len]);
    }

    // Arbitrary carried values, admission masks and failure position through the real replay;
    // unwinding is outside Kani's model. 任意携带值、准入掩码与失败位置穿过真实重放；展开不在 Kani 模型内。
    #[kani::proof]
    #[kani::unwind(9)]
    fn scene_interaction_restore() {
        assert_scene_interaction_restore(kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any());
    }

    // Real table and Vec storage: fixed operation shape, arbitrary first generation, values and
    // foreign handle. The arbitrary-shape model path runs in Proptest/fuzz; retirement below.
    // 真实表与 Vec 存储：操作形状固定，首代、值与外来句柄任意；任意形状模型路径由 Proptest/fuzz 覆盖。
    #[kani::proof]
    #[kani::unwind(4)]
    #[kani::solver(kissat)]
    fn handle_table_path() {
        // The first generation only reaches the allocator, whose harness keeps it arbitrary.
        // 首代只进入分配器，其 harness 保持任意。
        assert_handle_fixed_path(1, kani::any(), kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    #[kani::unwind(4)]
    #[kani::solver(kissat)]
    fn handle_allocator_path() {
        assert_handle_allocator_path(kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    #[kani::unwind(4)]
    #[kani::solver(kissat)]
    fn handle_status_path() {
        assert_handle_status_path(kani::any(), kani::any(), kani::any());
    }

    #[kani::proof]
    #[kani::unwind(3)]
    fn handle_retirement() {
        assert_handle_retirement(kani::any());
    }

    #[kani::proof]
    fn glyph_skew_mirror() {
        assert_glyph_skew(kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any());
        assert_glyph_skew_pure(kani::any(), kani::any());
    }

    #[kani::proof]
    fn font_weight_admission() {
        assert_font_weight(kani::any());
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn font_family_admission() {
        let bytes: [u8; 4] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= 4);
        assert_font_family(&bytes[..len]);
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn font_order_first_occurrence() {
        let seen: [u64; 4] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= 4);
        assert_font_admission(&seen[..len], kani::any(), kani::any());
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn bold_alias_admission() {
        let weights: [u16;4] = kani::any();
        let maxima: [u16;4] = kani::any();
        let len: usize = kani::any();
        kani::assume(len<=4);
        assert_bold_alias(&weights[..len],&maxima[..len]);
        assert_bold_alias(&weights[..len],&maxima[..len.saturating_sub(1)]);
    }

    #[kani::proof]
    #[kani::unwind(37)]
    fn canvas_pixel_read() {
        let bytes: [u8; 36] = kani::any();
        let width: u32 = kani::any();
        let height: u32 = kani::any();
        kani::assume(width >= 1 && width <= 3 && height >= 1 && height <= 3);
        let len = (width * height * 4) as usize;
        assert_canvas_pixel(&bytes[..len], width, height, kani::any(), kani::any());
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
        fn damage_span_property(span in any::<Option<(u32, u32)>>(), at in any::<u32>(),
                                lo in 0u32..8, hi in 0u32..8, small in 0u32..10) {
            assert_damage_span(span, at);
            assert_damage_span(Some((lo, hi)), small);
            assert_damage_span(None, small);
        }
        #[test]
        fn frame_damage_property(w in 0u32..5, h in 0u32..5, pw in 0u32..5,
                                 before in prop::collection::vec(0u8..3, 0..90),
                                 edits in prop::collection::vec((0usize..90, 1u8..3), 0..6)) {
            let len = (w * h * 4) as usize;
            let mut before = before;
            before.resize(len, 1);
            let mut after = before.clone();
            for (at, delta) in edits {
                if len > 0 { let i = at % len; after[i] = after[i].wrapping_add(delta); }
            }
            assert_frame_damage(Some((w, h, &before)), w, h, &after);
            assert_frame_damage(Some((pw, h, &before)), w, h, &after);
            assert_frame_damage(None, w, h, &after);
            assert_frame_damage(Some((w, h, &before)), w, h, &after[..len.saturating_sub(1)]);
        }
        #[test]
        fn frame_pace_property(dirty in any::<bool>(), presented in any::<Option<u64>>(), origin in any::<u64>(),
                               interval in any::<u64>(), now in any::<u64>(), so in 0u64..50, si in 0u64..9,
                               sn in 0u64..200, sp in 0u64..200) {
            assert_frame_tick(origin, interval, now);
            assert_frame_tick(so, si, sn);
            assert_frame_tick(u64::MAX - so, si, u64::MAX - sn);
            assert_frame_pace(dirty, presented, origin, interval, now);
            assert_frame_pace(dirty, Some(sp), so, si, sn);
            assert_frame_pace(true, Some(u64::MAX - sp), so, u64::MAX - si, u64::MAX - sn);
        }
        #[test]
        fn text_offset_property(layout_len in any::<usize>(), source_len in any::<usize>(), offset in any::<usize>(),
                                boundary in any::<bool>(), small in 0usize..8, delta in 0usize..3) {
            assert_text_offset(layout_len, source_len, offset, boundary);
            assert_text_offset(small, small, small + delta, boundary);
            assert_text_offset(small + delta, small, small, boundary);
            assert_text_offset(usize::MAX, usize::MAX, usize::MAX - small, boundary);
        }
        #[test]
        fn text_range_property(layout_len in any::<usize>(), source_len in any::<usize>(), start in any::<usize>(), end in any::<usize>(),
                               sb in any::<bool>(), eb in any::<bool>(), small in 0usize..8, a in 0usize..10, b in 0usize..10) {
            assert_text_range(layout_len, source_len, start, end, sb, eb);
            assert_text_range(small, small, a, b, sb, eb);
            assert_text_range(small + 1, small, a, b, sb, eb);
            assert_text_range(usize::MAX, usize::MAX, usize::MAX - a, usize::MAX - b, sb, eb);
        }
        #[test]
        fn gpu_readback_layout_property(width in any::<u32>(), height in any::<u32>(), w in 0u32..70000, h in 0u32..70000) {
            assert_gpu_readback_layout(width, height);
            assert_gpu_readback_layout(w, h);
            assert_gpu_readback_layout(65535 - w % 65536, 65535 - h % 65536);
        }
        #[test]
        fn access_actions_property(button in any::<bool>(), focusable in any::<bool>(), disabled in any::<bool>()) {
            assert_access_actions(button, focusable, disabled);
        }
        #[test]
        fn access_change_property(previous in any::<bool>(), next in any::<bool>(), equal in any::<bool>()) {
            assert_access_change(previous, next, equal);
        }
        #[test]
        fn access_request_property(kind in any::<u8>(), focus in any::<bool>(), activate in any::<bool>()) {
            assert_access_request(kind, focus, activate);
        }
        #[test]
        fn damage_rect_property(x in any::<u32>(), y in any::<u32>(), w in any::<u32>(), h in any::<u32>(),
                                width in any::<u32>(), height in any::<u32>(),
                                sx in 0u32..6, sy in 0u32..6, sw in 0u32..6, sh in 0u32..6, fw in 0u32..6, fh in 0u32..6) {
            assert_damage_rect(x, y, w, h, width, height);
            assert_damage_rect(sx, sy, sw, sh, fw, fh);
            assert_damage_rect(u32::MAX - sx, sy, sw, sh, u32::MAX, fh);
        }
        #[test]
        fn segment_write_property(bytes in prop::collection::vec(any::<u8>(), 0..256),
                                  source in prop::collection::vec(any::<u8>(), 0..12),
                                  row_bytes in any::<usize>(), height in any::<usize>(),
                                  stride in any::<usize>(), row in any::<usize>(), offset in any::<usize>(),
                                  small_row in 0usize..4, small_offset in 0usize..16) {
            assert_segment(&bytes, row_bytes, height, stride, row, offset, &source);
            assert_segment(&bytes, 12, 4, 14, small_row, small_offset, &source);
        }
        #[test]
        fn damage_write_property(w in 0u32..5, h in 0u32..5, padding in 0usize..3, extra in 0usize..3, cut in 0usize..3,
                                 before in prop::collection::vec(any::<u8>(), 0..80),
                                 edits in prop::collection::vec((0usize..80, 1u8..=255), 0..6),
                                 kind in 0u8..3, x in 0u32..6, y in 0u32..6, rw in 0u32..6, rh in 0u32..6,
                                 fill in any::<u8>()) {
            let len = (w * h * 4) as usize;
            let mut before = before;
            before.resize(len, 3);
            let mut after = before.clone();
            for (at, delta) in edits {
                if len > 0 { let i = at % len; after[i] = after[i].wrapping_add(delta); }
            }
            assert_damage_convergence(&before, &after, w, h, padding);
            let stride = w as usize * 4 + padding;
            let surface_len = (stride * h as usize + extra).saturating_sub(padding + cut);
            let surface = vec![fill; surface_len];
            let damage = match kind {
                0 => FrameDamage::Unchanged,
                1 => FrameDamage::Full,
                _ => FrameDamage::Rect { x, y, width: rw, height: rh },
            };
            assert_damage_write(&surface, stride, w, h, &after, damage);
            assert_damage_write(&surface, stride, w, h, &after[..len.saturating_sub(1)], damage);
            assert_damage_write(&surface, stride.saturating_sub(1), w, h, &after, damage);
        }
        #[test]
        fn frame_reuse_property(dirty in any::<bool>(), cw in 0u32..4, ch in 0u32..4, len in 0usize..80,
                                w in 0u32..4, h in 0u32..4, wide in any::<(u32, u32, usize)>()) {
            let expected = (w * h * 4) as usize;
            assert_frame_reuse(dirty, (cw, ch, len), w, h, expected);
            assert_frame_reuse(dirty, (w, h, expected), w, h, expected);
            assert_frame_reuse(dirty, wide, wide.0, wide.1, wide.2);
        }
        #[test]
        fn color_construct_property(r in any::<u8>(), g in any::<u8>(), b in any::<u8>(), a in any::<u8>(), alpha in any::<u8>()) {
            assert_color_construct(r, g, b, a, alpha);
        }
        #[test]
        fn handle_table_property(capacity in 0u32..6, first in any::<u32>(), near in 0u32..4, probe in any::<u32>(), steps in proptest::collection::vec((0u8..3, any::<u8>(), any::<u32>()), 0..40), small in proptest::collection::vec((0u8..3, 0u8..4, 0u32..6), 0..40)) {
            assert_handle_path(capacity, first, &steps);
            assert_handle_path(capacity, HANDLE_GENERATION_RETIRED.wrapping_sub(near * 2 + 1), &small);
            assert_handle_path(capacity, 1, &small);
            assert_handle_retirement(probe);
            assert_handle_allocator_path(first, probe % 3, first);
            assert_handle_allocator_path(first | 1, 0, first | 1);
            assert_handle_status_path(first, probe % 3, first);
            assert_handle_status_path(first | 1, near % 2, near | 1);
            assert_handle_fixed_path(first, probe % 3, first, near as u8, 9);
            assert_handle_fixed_path(first | 1, near % 3, first | 1, near as u8, 9);
        }

        #[test]
        fn desktop_pointer_property(elapsed in any::<u128>(), near in 0u64..3, steps in proptest::collection::vec(any::<DesktopPointerStep>(), 0..12)) {
            assert_desktop_timestamp(elapsed);
            assert_desktop_timestamp(u64::MAX as u128 + near as u128);
            assert_desktop_timestamp(u64::MAX as u128 - near as u128);
            assert_desktop_pointer_path(&steps);
        }
        #[test]
        fn scene_frame_order_property(source_ok in any::<bool>(), version in any::<u64>(), next in any::<u64>(), ready in any::<bool>(), token in any::<u64>(), panic_in_source in any::<bool>()) {
            assert_scene_frame_order(source_ok, version, next, ready, token);
            assert_scene_frame_order(source_ok, version, version, ready, token);
            assert_scene_frame_panic(panic_in_source, version, token);
        }

        #[test]
        fn scene_node_remove_property(spec in prop::collection::vec(any::<(u8, u8)>(), 0..24), root in any::<u8>(), id in any::<u8>()) {
            assert_scene_node_remove_bytes(&spec, root, id);
            let at = |i: usize| spec.get(i).map_or(0, |pair| pair.0 as u64 % 6);
            assert_scene_node_remove_shape(at(0), &[(at(1), Some(at(2))), (at(3), Some(at(4))), (at(5), None)], id as u64 % 6);
        }
        #[test]
        fn scene_node_find_property(spec in prop::collection::vec(any::<(u8, u8)>(), 0..24), root in any::<u8>(), id in any::<u8>()) {
            assert_scene_node_find_bytes(&spec, root, id);
            let at = |i: usize| spec.get(i).map_or(0, |pair| pair.0 as u64 % 6);
            assert_scene_node_find_shape(at(0), &[(at(1), Some(at(2))), (at(3), Some(at(4))), (at(5), None)], id as u64 % 6);
        }
        #[test]
        fn scene_batch_stage_property(start in any::<u64>(), patches in prop::collection::vec(any::<u8>(), 0..16), panic_at in 0usize..18) {
            assert_scene_batch_stage(start, &patches);
            assert_scene_batch_panic(start, &patches, panic_at);
        }
        #[test]
        fn scene_interaction_restore_property(viewport in any::<u64>(), scroll in any::<u64>(), pressed in any::<Option<u8>>(), focused in any::<Option<u8>>(), pointer in any::<Option<u8>>(), cache in any::<u64>(), press_mask in any::<u8>(), focus_mask in any::<u8>(), fail_at in 0u8..9, panic_at in 0u8..9) {
            assert_scene_interaction_restore(viewport, scroll, pressed, focused, pointer, cache, press_mask, focus_mask, fail_at);
            assert_scene_interaction_restore_panic(viewport, scroll, cache, panic_at);
        }
        #[test]
        fn glyph_skew_property(coeffs in any::<[u64; 6]>(), c in any::<u64>(), signs in any::<u8>(), pick in any::<u8>()) {
            let [a, b, _, d, e, f] = coeffs;
            assert_glyph_skew(a, b, c, d, e, f);
            assert_glyph_skew_pure(c, signs);
            // Near-pure transforms: replace a subset of coefficients with the pure values.
            // 近纯变换：用纯斜切的值替换部分系数。
            let one = 1.0f64.to_bits();
            let keep = |bit: u8, pure: u64, other: u64| if pick & bit != 0 { pure } else { other };
            assert_glyph_skew(keep(1, one, a), keep(2, 0, b), c, keep(4, one, d), keep(8, 1 << 63, e), keep(16, 0, f));
        }
        #[test]
        fn font_weight_property(weight in any::<u16>(), near in 0u16..1003) {
            assert_font_weight(weight);
            assert_font_weight(near);
        }
        #[test]
        fn font_family_property(name in prop::collection::vec(any::<u8>(), 0..140),
                                plain in prop::collection::vec(0x20u8..0x7f, 120..136)) {
            assert_font_family(&name);
            assert_font_family(&plain);
        }
        #[test]
        fn font_order_property(seen in prop::collection::vec(0u64..6, 0..12), id in 0u64..8,
                               wide in any::<u64>(), count in any::<usize>(), small in 0usize..20) {
            assert_font_order(&seen, id, count);
            assert_font_order(&seen, wide, small);
            if let Some(&first) = seen.first() {
                assert_font_order(&seen, first, 16);
            }
        }
        #[test]
        fn bold_alias_property(weights in prop::collection::vec(any::<u16>(),0..16),
                               maxima in prop::collection::vec(any::<u16>(),0..16)) {
            assert_bold_alias(&weights,&maxima);
            assert_bold_alias(&weights,&weights);
            let low:Vec<u16>=weights.iter().map(|weight|weight%501).collect();
            assert_bold_alias(&low,&low);
        }
        #[test]
        fn canvas_pixel_property(bytes in prop::collection::vec(any::<u8>(), 0..256),
                                 width in 1u32..8, height in 1u32..8, x in any::<u32>(), y in any::<u32>(),
                                 small_x in 0u32..9, small_y in 0u32..9) {
            let mut packed = vec![0; (width * height * 4) as usize];
            for (i, byte) in packed.iter_mut().enumerate() {
                *byte = bytes.get(i % bytes.len().max(1)).copied().unwrap_or(0);
            }
            assert_canvas_pixel(&packed, width, height, x, y);
            assert_canvas_pixel(&packed, width, height, small_x, small_y);
            assert_canvas_pixel(&packed, width, height, width - 1, height - 1);
            assert_canvas_pixel(&packed, width, height, width, 0);
            assert_canvas_pixel(&packed, width, height, 0, height);
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
    fn handle_table_boundaries() {
        assert_handle_path(2, 1, &[(0, 0, 5), (0, 0, 6), (0, 0, 7), (1, 0, 0), (1, 0, 0), (0, 0, 8), (2, 0, 3), (2, 1, 2)]);
        assert_handle_path(0, 1, &[(0, 0, 5), (1, 200, 0), (2, 9, 1)]);
        assert_handle_path(1, HANDLE_GENERATION_RETIRED - 1, &[(0, 0, 5), (1, 0, 0), (0, 0, 6), (2, 0, HANDLE_GENERATION_RETIRED)]);
        assert_handle_path(1, HANDLE_GENERATION_RETIRED, &[(0, 0, 5)]);
        assert_handle_retirement(u32::MAX);
        assert_handle_status_path(3, 0, 1);
        assert_handle_status_path(3, 0, 3);
        assert_handle_status_path(3, 0, 5);
        assert_handle_fixed_path(1, 0, 1, 5, 6);
        assert_handle_fixed_path(1, 1, 2, 5, 6);
        assert_handle_fixed_path(HANDLE_GENERATION_RETIRED - 1, 1, HANDLE_GENERATION_RETIRED - 1, 5, 6);
        // Owned values drop exactly once through remove, reuse and the table itself.
        // 自有值经移除、复用与表析构恰好释放一次。
        let mut table = HandleTable::new(2);
        let first = table.insert(vec![1u8; 8]).unwrap();
        let second = table.insert(vec![2u8; 8]).unwrap();
        assert_eq!(table.insert(vec![3u8; 8]).unwrap_err(), vec![3u8; 8]);
        table.get_mut(first).unwrap().push(9);
        assert_eq!(table.remove(first).unwrap().len(), 9);
        let third = table.insert(vec![4u8; 8]).unwrap();
        assert_eq!((third.index, third.generation), (first.index, first.generation + 2));
        assert!(table.get(first).is_none());
        assert_eq!(table.get(second).unwrap()[0], 2);
    }

    #[test]
    fn bold_alias_boundaries() {
        for (weights,maxima) in [
            (&[][..],&[][..]),
            (&[0][..],&[0][..]),
            (&[400][..],&[400][..]),
            (&[400][..],&[700][..]),
            (&[500,500][..],&[500,500][..]),
            (&[501][..],&[501][..]),
            (&[400,400][..],&[400][..]),
            (&[u16::MAX][..],&[u16::MAX][..]),
        ] {
            assert_bold_alias(weights,maxima);
        }
    }

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
        for span in [None, Some((0, 0)), Some((3, 9)), Some((9, 3)), Some((0, u32::MAX))] {
            for at in [0, 3, 5, 9, u32::MAX] {
                assert_damage_span(span, at);
            }
        }
        let base: Vec<u8> = (0..3 * 2 * 4).map(|i| i as u8).collect();
        for touched in [None, Some(0), Some(5), Some(23)] {
            let mut next = base.clone();
            if let Some(i) = touched { next[i] ^= 0x80; }
            assert_frame_damage(Some((3, 2, &base)), 3, 2, &next);
            assert_frame_damage(Some((2, 3, &base)), 3, 2, &next);
            assert_frame_damage(None, 3, 2, &next);
            assert_frame_damage(Some((3, 2, &base[..20])), 3, 2, &next[..20]);
        }
        for (origin, interval) in [(0, 0), (0, 1), (5, 3), (100, 16), (u64::MAX - 1, u64::MAX - 1)] {
            for now in [0, 4, 5, 7, 8, 100, 115, 116, u64::MAX - 1, u64::MAX] {
                assert_frame_tick(origin, interval, now);
                for presented in [None, Some(0), Some(5), Some(100), Some(u64::MAX)] {
                    assert_frame_pace(true, presented, origin, interval, now);
                    assert_frame_pace(false, presented, origin, interval, now);
                }
            }
        }
        for (layout_len, source_len) in [(0, 0), (3, 3), (3, 4), (4, 3), (usize::MAX, usize::MAX)] {
            for offset in [0, 2, 3, 4, usize::MAX] {
                assert_text_offset(layout_len, source_len, offset, true);
                assert_text_offset(layout_len, source_len, offset, false);
            }
        }
        for (layout_len, source_len) in [(0, 0), (3, 3), (3, 4), (usize::MAX, usize::MAX)] {
            for (start, end) in [(0, 0), (0, 3), (2, 1), (3, 3), (1, 4), (0, usize::MAX)] {
                for (sb, eb) in [(true, true), (true, false), (false, true)] {
                    assert_text_range(layout_len, source_len, start, end, sb, eb);
                }
            }
        }
        for (width, height) in [(0, 1), (1, 0), (1, 1), (63, 2), (64, 2), (65, 2), (320, 200), (65535, 65535), (65536, 1), (1, 65536), (u32::MAX, u32::MAX)] {
            assert_gpu_readback_layout(width, height);
        }
        for bits in 0u8..8 {
            assert_access_actions(bits & 1 == 1, bits & 2 == 2, bits & 4 == 4);
            assert_access_change(bits & 1 == 1, bits & 2 == 2, bits & 4 == 4);
            assert_access_request(bits >> 1, bits & 1 == 1, bits & 4 == 4);
        }
        for (x, y, w, h) in [(0, 0, 1, 1), (2, 1, 1, 1), (0, 0, 3, 2), (3, 0, 1, 1), (1, 1, 0, 1), (u32::MAX, 0, 2, 1)] {
            assert_damage_rect(x, y, w, h, 3, 2);
            let damage = FrameDamage::Rect { x, y, width: w, height: h };
            assert_damage_write(&[0xEE; 3 * 4 * 2 + 2], 14, 3, 2, &base, damage);
        }
        assert_damage_write(&[0xEE; 26], 14, 3, 2, &base, FrameDamage::Full);
        assert_damage_write(&[0xEE; 26], 14, 3, 2, &base, FrameDamage::Unchanged);
        assert_damage_write(&[0xEE; 25], 14, 3, 2, &base, FrameDamage::Full);
        let mut touched = base.clone();
        touched[13] ^= 0x40;
        assert_damage_convergence(&base, &touched, 3, 2, 2);
        for offset in [0, 1, 2, 3, usize::MAX] {
            for source in [&[][..], &[9][..], &[9, 8][..], &[9, 8, 7][..]] {
                assert_segment(&[1, 2, 0xaa, 0xbb, 3, 4], 2, 2, 4, 1, offset, source);
            }
        }
        for dirty in [false, true] {
            for (w, h) in [(0, 0), (1, 1), (320, 200), (u32::MAX, u32::MAX)] {
                for len in [0, 4, 256_000, usize::MAX] {
                    assert_frame_reuse(dirty, (w, h, len), w, h, 256_000);
                    assert_frame_reuse(dirty, (w, h.wrapping_add(1), len), w, h, len);
                }
            }
        }
        for weight in [0, 1, 2, 400, 700, 999, 1000, 1001, u16::MAX] {
            assert_font_weight(weight);
        }
        for (weights,maxima) in [
            (&[][..],&[][..]),
            (&[400][..],&[400][..]),
            (&[400][..],&[700][..]),
            (&[500,500][..],&[500,500][..]),
            (&[501][..],&[501][..]),
            (&[400,400][..],&[400][..]),
        ] {
            assert_bold_alias(weights,maxima);
        }
        for elapsed in [0, 1, u64::MAX as u128, u64::MAX as u128 + 1, u128::MAX] {
            assert_desktop_timestamp(elapsed);
        }
        for version in [0, 1, u64::MAX] {
            for (source_ok, ready) in [(false, false), (false, true), (true, false), (true, true)] {
                assert_scene_frame_order(source_ok, version, version, ready, version);
                assert_scene_frame_order(source_ok, version, version.wrapping_add(1), ready, version);
            }
            assert_scene_frame_panic(true, version, version);
            assert_scene_frame_panic(false, version, version);
            assert_scene_node_remove_bytes(&[(1, 2), (2, 0), (3, 0), (2, 1), (3, 0)], version as u8, 3);
            assert_scene_node_remove_bytes(&[(1, 0), (2, 0)], version as u8, version as u8);
            assert_scene_node_remove_shape(1, &[(2, Some(3)), (4, Some(5))], version % 7);
            assert_scene_node_find_bytes(&[(1, 2), (2, 0), (3, 0), (2, 1), (3, 0)], version as u8, 3);
            assert_scene_node_find_bytes(&[(1, 0), (2, 0)], version as u8, version as u8);
            assert_scene_node_find_shape(1, &[(2, Some(3)), (4, Some(5))], version % 7);
            assert_scene_batch_stage(version, &[]);
            assert_scene_batch_stage(version, &[1, 2, 3]);
            assert_scene_batch_stage(version, &[1, 5, 3]);
            assert_scene_batch_panic(version, &[1, 2, 3], 0);
            assert_scene_batch_panic(version, &[1, 2, 3], 2);
            assert_scene_batch_panic(version, &[1, 2, 3], 3);
            for fail_at in 0..7 {
                assert_scene_interaction_restore(version, 2, Some(1), Some(2), Some(3), 4, 0xff, 0xff, fail_at);
                assert_scene_interaction_restore_panic(version, 2, 4, fail_at);
            }
            assert_scene_interaction_restore(version, 2, Some(1), Some(2), None, 4, 0, 0, 9);
        }
        assert_desktop_pointer_path(&[
            (0, 0, 0, 1.5f32.to_bits(), f32::NAN.to_bits(), u64::MAX),
            (1, 5, 9, 0, 0, 3),
            (1, 1, 0, 0, 0, 2),
            (3, 15, 0, 0, 0, 0),
            (0, 0, 0, 2.5f32.to_bits(), 0, 1),
            (2, 1, 0, 0, 0, 0),
            (2, 5, 9, 0, 0, 7),
            (0, 0, 0, 0, 0, 7),
        ]);
        let skew = 14f64.to_radians().tan().to_bits();
        for signs in 0..8 {
            assert_glyph_skew_pure(skew, signs);
        }
        for (a, b, d, e, f) in [(1.0f64, 0.0f64, 1.0f64, 0.0f64, 3.0f64), (2.0, 0.0, 1.0, 0.0, 0.0), (1.0, f64::NAN, 1.0, 0.0, 0.0)] {
            assert_glyph_skew(a.to_bits(), b.to_bits(), skew, d.to_bits(), e.to_bits(), f.to_bits());
        }
        for channel in [0, 1, 127, 254, 255] {
            assert_color_construct(channel, 255 - channel, channel / 2, 255 - channel / 3, channel);
        }
        for name in [&b""[..], &b"DejaVu Sans"[..], "思源黑体".as_bytes(), &b"a\"b"[..], &b"a\\b"[..], &b"\x1f"[..], &b"\x7f"[..],
                     &[b'x'; 128][..], &[b'x'; 129][..]] {
            assert_font_family(name);
        }
        for seen in [&[][..], &[3][..], &[3, 3][..], &[1, 2, 3, u64::MAX][..]] {
            for id in [0, 1, 3, u64::MAX] {
                for count in [0, 1, 16, 17, usize::MAX] {
                    assert_font_order(seen, id, count);
                }
            }
        }
        let ramp: Vec<u8> = (0..48).collect();
        for (width, height) in [(1, 1), (2, 3), (3, 2), (4, 3)] {
            let len = (width * height * 4) as usize;
            for x in [0, 1, width - 1, width, u32::MAX] {
                for y in [0, 1, height - 1, height, u32::MAX] {
                    assert_canvas_pixel(&ramp[..len], width, height, x, y);
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
                assert_handle_path(2, 1, &[(0, 0, 5), (0, 0, 6), (1, 0, 0), (0, 0, 7), (2, 0, 3)]);
                assert_handle_retirement(0);
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
                assert_eq!(canvas.pixel(0, 0).map(|c| [c.r, c.g, c.b, c.a]), Some(*first));
                assert!(canvas.pixel(1, 0).is_none());
                assert_font_order(&[first[0] as u64, first[1] as u64], first[2] as u64, first[3] as usize);
                assert_font_weight(u16::from_le_bytes([first[0], first[1] & 7]));
                assert_glyph_skew_pure(u64::from_le_bytes([first[0], first[1], first[2], first[3], 0, 0, 0, 0]), first[2]);
                assert_desktop_timestamp(first[0] as u128);
                assert_scene_frame_order(first[0] & 1 == 1, first[1] as u64, first[2] as u64, first[3] & 1 == 1, first[0] as u64);
                assert_scene_batch_stage(first[0] as u64, &first[..(first[1] as usize & 3)]);
                assert_scene_node_remove_bytes(&[(first[0], first[1]), (first[2], first[3])], first[0], first[2]);
                assert_scene_interaction_restore(first[0] as u64, first[1] as u64, Some(first[2]), Some(first[3]), Some(first[0]), first[1] as u64, first[2], first[3], first[0]);
                assert_desktop_pointer_path(&[(1, first[0], first[1] as u16, 0, 0, first[2] as u64), (0, 0, 0, first[3] as u32, 0, 1), (2, first[0], first[1] as u16, 0, 0, 2)]);
                assert_color_construct(first[0], first[1], first[2], first[3], first[0] ^ first[3]);
                assert_font_family(&first[..(first[2] as usize & 3)]);
                assert_damage_span(Some((first[0] as u32, first[1] as u32)), first[2] as u32);
                assert_damage_rect(first[0] as u32, first[1] as u32, first[2] as u32, first[3] as u32, 2, 2);
                assert_frame_tick(first[0] as u64, first[1] as u64, first[2] as u64);
                assert_access_change(first[0] & 1 == 1, first[1] & 1 == 1, first[2] & 1 == 1);
                assert_access_request(first[3], first[0] & 2 == 2, first[1] & 2 == 2);
                assert_frame_pace(first[3] & 1 == 1, Some(first[0] as u64), first[1] as u64, first[2] as u64, first[3] as u64);
                assert_segment(first.as_ref(), 2, 2, 2, 1, first[0] as usize % 3, &first[2..]);
                assert_frame_damage(Some((1, 1, first.as_ref())), 1, 1, &[first[3], first[2], first[1], first[0]]);
                assert_frame_reuse(first[0] & 1 == 1, (first[1] as u32, first[2] as u32, first[3] as usize),
                    first[1] as u32, first[2] as u32, first[3] as usize);
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
