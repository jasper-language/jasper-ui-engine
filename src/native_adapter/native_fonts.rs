//! Configure injected font resources through upstream APIs; no font selection/shaping engine.
//! 仅配置注入字体资源的默认族顺序，复用上游扫描、选择、塑形。
use std::sync::Arc;

/// One injected file keeps the earlier lexical family policy; invalid bytes yield no families.
/// 单文件注入沿用字典序族策略；无效字节得到空族表（与旧行为一致）。
pub(crate) fn injected_font_context(bytes: &[u8]) -> blitz_dom::FontContext {
    let (context,_)=ordered_font_context(&[bytes]);
    context
}

/// Host-ordered fallback stack: files fall back in the given order; inside one file
/// (e.g. a regional collection) families keep lexical name order. Rejects an empty or
/// oversized stack and any file that registers no family, instead of silently skipping it.
/// 宿主顺序回退栈：文件间按给定顺序回退，同一文件内（如地区字体集）按族名字典序；
/// 空栈、超限或任一文件未注册出字体族时明确拒绝，不静默跳过。
pub(crate) fn injected_font_stack(fonts: &[&[u8]]) -> Option<blitz_dom::FontContext> {
    if !font_stack_count_admits(fonts.len()) {return None;}
    let (context,complete)=ordered_font_context(fonts);
    complete.then_some(context)
}

fn ordered_font_context(fonts: &[&[u8]]) -> (blitz_dom::FontContext,bool) {
    let mut context=blitz_dom::FontContext {
        source_cache:fontique::SourceCache::new_shared(),
        collection:fontique::Collection::new(fontique::CollectionOptions {shared:false,system_fonts:false}),
    };
    let mut complete=true;
    let (mut order,mut seen)=(Vec::new(),Vec::new());
    for bytes in fonts {
        let decoded=blitz_dom::decode_font_bytes(bytes).into_owned();
        // Upstream returns families from a HashMap; its iteration order must not become policy.
        // 上游按 HashMap 返回族，迭代顺序不得成为回退策略。
        let registered=context.collection.register_fonts(fontique::Blob::new(Arc::new(decoded) as _),None);
        let mut named:Vec<(String,fontique::FamilyId)>=registered.iter()
            .filter_map(|(id,_)|Some((context.collection.family_name(*id)?.to_owned(),*id))).collect();
        if named.is_empty() {complete=false;}
        named.sort_by(|a,b|a.0.cmp(&b.0));
        for (_,id) in named {
            if font_order_admits(&seen,id.to_u64()) {seen.push(id.to_u64());order.push(id);}
        }
    }
    for generic in [fontique::GenericFamily::SansSerif,fontique::GenericFamily::Serif,
        fontique::GenericFamily::Monospace,fontique::GenericFamily::SystemUi] {
        context.collection.set_generic_families(generic,order.iter().copied());
    }
    (context,complete)
}

// BEGIN VERIFIED FONT ORDER KERNEL
/// Upper bound on host-injected font files per view.
/// 每个视图宿主注入字体文件数上限。
pub const MAX_INJECTED_FONTS: usize = 16;

/// A font stack holds at least one and at most MAX_INJECTED_FONTS files.
/// 字体栈至少一个、至多 MAX_INJECTED_FONTS 个文件。
pub fn font_stack_count_admits(count: usize) -> bool {
    count >= 1 && count <= MAX_INJECTED_FONTS
}

/// True only for the first occurrence of `id`: earlier files keep fallback priority.
/// 仅 `id` 首次出现时为真：先注入的文件保持回退优先级。
pub fn font_order_admits(seen: &[u64], id: u64) -> bool {
    let mut index: usize = 0;
    while index < seen.len() {
        if seen[index] == id {
            return false;
        }
        index += 1;
    }
    true
}
// END VERIFIED FONT ORDER KERNEL

// BEGIN VERIFIED FONT WEIGHT KERNEL
/// CSS numeric font weights span 1..=1000; anything else is rejected, never clamped.
/// CSS 数值字重范围为 1..=1000；其余值拒绝，不钳制。
pub fn font_weight_admits(weight: u16) -> bool {
    weight >= 1 && weight <= 1000
}
// END VERIFIED FONT WEIGHT KERNEL

// BEGIN VERIFIED FONT FAMILY KERNEL
/// Upper bound on one declared font family name, in UTF-8 bytes.
/// 单个声明字体族名的 UTF-8 字节数上限。
pub const MAX_FONT_FAMILY_BYTES: usize = 128;

/// A family name is 1..=MAX_FONT_FAMILY_BYTES bytes with no control byte, DEL, quote or
/// backslash, so it is carried verbatim inside one CSS quoted string; rejected, never escaped.
/// 族名为 1..=MAX_FONT_FAMILY_BYTES 字节，且不含控制字节、DEL、双引号、反斜杠，
/// 因而可原样放入一个 CSS 引号串；不合规即拒绝，不转义。
pub fn font_family_admits(name: &[u8]) -> bool {
    if name.len() < 1 || name.len() > MAX_FONT_FAMILY_BYTES {
        return false;
    }
    let mut index: usize = 0;
    while index < name.len() {
        let byte = name[index];
        if byte < 0x20 || byte == 0x22 || byte == 0x5c || byte == 0x7f {
            return false;
        }
        index += 1;
    }
    true
}
// END VERIFIED FONT FAMILY KERNEL
