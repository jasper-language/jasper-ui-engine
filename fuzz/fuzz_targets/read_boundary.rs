#![no_main]
use jasper_ui_read_verification::{
    assert_borrow, assert_bytes_slot, assert_canvas_import, assert_canvas_pixel, assert_capture_query,
    assert_command_modifier, assert_cursor, assert_endpoint, assert_focus_cleanup,
    assert_damage_convergence, assert_damage_rect, assert_damage_span, assert_damage_write, assert_focus_selection, assert_font_order, assert_frame_damage, assert_frame_pace, assert_frame_reuse, assert_frame_tick, assert_gpu_readback_layout, assert_header, assert_access_actions, assert_text_offset, assert_text_range, assert_hover_lifecycle, assert_input_retire,
    assert_keyboard_focus, assert_payload, assert_pixel_conversion, assert_pointer_order,
    assert_read, assert_resource, assert_rgb_word, assert_role, assert_scalars, assert_surface_row,
    assert_surface_span, assert_text_classes, assert_versions, assert_view, assert_view_copy,
    assert_segment, assert_write, FrameDamage, ResourceKey,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Three full-width values followed by a byte allocation; fixed x86_64 lane.
    // 三个完整宽度数值及后续字节缓冲，本目标固定 x86_64。
    assert_header(data);
    assert_payload(data, 0, 16);
    if data.len() < 24 {
        return;
    }
    let total = u64::from_le_bytes(data[0..8].try_into().unwrap()) as usize;
    let pos = u64::from_le_bytes(data[8..16].try_into().unwrap()) as usize;
    let len = u64::from_le_bytes(data[16..24].try_into().unwrap()) as usize;
    assert_focus_cleanup(&[total as u64, pos as u64], Some(len as u64));
    assert_keyboard_focus(
        &[total as u64, pos as u64],
        if data[0] & 1 == 0 {
            None
        } else {
            Some(len as u64)
        },
    );
    assert_focus_selection(&[total as u64, pos as u64], Some(len as u64), len as u64);
    assert_input_retire(
        &[total as u64, pos as u64],
        Some(len as u64),
        total as u64,
        pos as u64,
        len as u64,
        data[0] & 1 != 0,
    );
    assert_hover_lifecycle(
        if data[0] & 1 != 0 {
            Some(total as u64)
        } else {
            None
        },
        if data[1] & 1 != 0 {
            Some(pos as u64)
        } else {
            None
        },
        total as u64,
        pos as u64,
        len as u64,
        data[2] & 1 != 0,
    );
    assert_pointer_order(total as u64, pos as u64, len as u64);
    assert_capture_query(
        total as u64,
        pos as u64,
        len as u64,
        if data[0] & 1 == 0 {
            None
        } else {
            Some(pos as u64)
        },
        if data[1] & 1 == 0 {
            None
        } else {
            Some(total as u64)
        },
    );
    assert_command_modifier(data[0]);
    assert_bytes_slot(&data[24..], total as u64, pos as u64, len as u64);
    if let Some(ch) = char::from_u32(total as u32) {
        assert_text_classes(ch);
    }
    assert_role(data[0]);
    assert_resource(
        total as u64,
        pos as u64,
        len as u64,
        data[0] & 1 != 0,
        total as u64,
        ResourceKey {
            session: total as u64,
            slot: pos as u64,
            generation: len as u64,
        },
    );
    assert_resource(
        total as u64,
        pos as u64,
        len as u64,
        data[0] & 1 != 0,
        (total as u64) ^ 1,
        ResourceKey {
            session: total as u64,
            slot: pos as u64,
            generation: len as u64,
        },
    );
    assert_versions(total as u64, pos as u64, len & 1 == 0);
    assert_versions(total as u64, total as u64, len & 1 == 0);
    assert_surface_span(total, pos, len, data.len());
    assert_surface_span(data.len(), data.len(), data.len(), total);
    assert_rgb_word(data[0], data[1], data[2]);
    assert_canvas_import(&data[24..], total as u32, pos as u32);
    if (data.len() - 24) % 4 == 0 {
        assert_canvas_import(&data[24..], 1, ((data.len() - 24) / 4) as u32);
    }
    if (data.len() - 24) % 4 == 0 && data.len() > 24 {
        let pixels = ((data.len() - 24) / 4) as u32;
        assert_canvas_pixel(&data[24..], 1, pixels, total as u32 % 2, pos as u32 % (pixels + 1));
        assert_canvas_pixel(&data[24..], pixels, 1, len as u32, 0);
    }
    let ids: Vec<u64> = data[24..].iter().take(16).map(|&b| (b % 8) as u64).collect();
    assert_font_order(&ids, len as u64 % 9, total);
    assert_font_order(&ids, len as u64, pos % 20);
    assert_frame_reuse(total % 2 == 1, (len as u32, pos as u32, total), pos as u32, len as u32, total);
    assert_frame_reuse(false, (len as u32, pos as u32, total), len as u32, pos as u32, total);
    assert_damage_span(if total % 2 == 0 { None } else { Some((len as u32, pos as u32)) }, total as u32);
    let pixels = &data[24..];
    let (fw, fh) = ((len % 5) as u32, (pos % 5) as u32);
    let packed = (fw * fh * 4) as usize;
    if pixels.len() >= packed * 2 {
        assert_frame_damage(Some((fw, fh, &pixels[..packed])), fw, fh, &pixels[packed..packed * 2]);
        assert_frame_damage(Some((fw, fh, &pixels[..packed])), fw, fh, &pixels[..packed]);
    }
    assert_frame_damage(Some((fw, fh, pixels)), fw, fh, pixels);
    assert_damage_rect(len as u32, pos as u32, total as u32, (total >> 8) as u32, fw, fh);
    assert_text_offset(len, pos, total, data[2] & 1 == 1);
    assert_text_offset(len % 9, len % 9, pos % 11, data[3] & 1 == 1);
    assert_text_range(len, pos, total, pos, data[2] & 1 == 1, data[3] & 1 == 1);
    assert_text_range(len % 9, len % 9, pos % 11, total % 11, data[2] & 2 == 2, data[3] & 2 == 2);
    assert_access_actions(data[0] & 2 == 2, data[1] & 2 == 2, data[2] & 4 == 4);
    assert_gpu_readback_layout(len as u32, total as u32);
    assert_gpu_readback_layout((len % 70000) as u32, (pos % 70000) as u32);
    assert_frame_tick(total as u64, pos as u64, len as u64);
    assert_frame_tick(pos as u64 % 1000, len as u64 % 17, total as u64 % 5000);
    assert_frame_pace(data[0] & 1 == 1, if data[1] & 1 == 0 { None } else { Some(len as u64) }, total as u64, pos as u64, len as u64);
    assert_frame_pace(true, Some(len as u64 % 5000), pos as u64 % 1000, total as u64 % 17, len as u64 % 5000);
    assert_segment(pixels, len % 9, pos % 4, len % 11, total % 4, pos % 7, &data[..len % 5]);
    if pixels.len() >= packed * 2 {
        assert_damage_convergence(&pixels[..packed], &pixels[packed..packed * 2], fw, fh, total % 3);
    }
    let damage = match total % 3 {
        0 => FrameDamage::Unchanged,
        1 => FrameDamage::Full,
        _ => FrameDamage::Rect { x: (len % 6) as u32, y: (pos % 6) as u32, width: (total % 6) as u32, height: (total / 7 % 6) as u32 },
    };
    if pixels.len() >= packed {
        assert_damage_write(&pixels[..pixels.len() - packed], fw as usize * 4 + pos % 2, fw, fh, &pixels[pixels.len() - packed..], damage);
    }
    assert_pixel_conversion(&data[24..], &[total as u32, pos as u32]);
    let aligned = &data[24..24 + (data.len() - 24) / 4 * 4];
    assert_pixel_conversion(aligned, &vec![0xa5a5a5a5; aligned.len() / 4]);
    assert_payload(&data[24..], pos, total as u32);
    assert_view_copy(
        &data[24..],
        total as u32,
        pos,
        len,
        data[1] as usize,
        &data[..8],
    );
    assert_view_copy(&data[24..], 2, 1, 8, (data[0] & 1) as usize, &data[..8]);
    assert_surface_row(total as u32, pos);
    assert_view(&data[24..], total as u32, pos, len, data[1] as usize);
    assert_view(&data[24..], 1, 1, 4, data[0] as usize);
    assert_endpoint(total, pos, len);
    assert_read(&data[24..], pos, len);
    assert_cursor(&data[24..], pos, len);
    assert_scalars(&data[24..], pos);
    assert_scalars(&data[24..], 0);
    assert_write(&data[24..], total, pos, len, data[1] as usize, &data[..2]);
    assert_write(&data[24..], 2, 2, 4, (data[0] & 1) as usize, &data[..2]);
    assert_borrow(&data[24..], total, pos, len, data[1] as usize);
    assert_borrow(&data[24..], 1, data.len() - 24, 1, data[0] as usize);
});
