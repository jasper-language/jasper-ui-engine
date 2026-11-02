//! Configure injected font resources through upstream APIs; no font selection/shaping engine.
//! 仅配置注入字体资源的默认族顺序，复用上游扫描、选择、塑形。
use std::collections::BTreeSet;
use std::sync::Arc;

/// Injected font context plus the blob ids of synthetic bold stand-in faces (see
/// `register_bold_aliases`); the painter emboldens exactly the glyph runs drawn from those blobs.
/// 注入字体上下文及合成粗体替身字面的 Blob id（见 `register_bold_aliases`）；
/// 绘制器只加粗取自这些 Blob 的字形串。
pub(crate) struct InjectedFonts {
    pub(crate) context: blitz_dom::FontContext,
    pub(crate) bold_aliases: BTreeSet<u64>,
}

/// One injected file keeps the earlier lexical family policy; invalid bytes yield no families.
/// 单文件注入沿用字典序族策略；无效字节得到空族表（与旧行为一致）。
pub(crate) fn injected_font_context(bytes: &[u8]) -> InjectedFonts {
    let (fonts, _) = ordered_font_context(&[bytes]);
    fonts
}

/// Host-ordered fallback stack: files fall back in the given order; inside one file
/// (e.g. a regional collection) families keep lexical name order. Rejects an empty or
/// oversized stack and any file that registers no family, instead of silently skipping it.
/// 宿主顺序回退栈：文件间按给定顺序回退，同一文件内（如地区字体集）按族名字典序；
/// 空栈、超限或任一文件未注册出字体族时明确拒绝，不静默跳过。
pub(crate) fn injected_font_stack(fonts: &[&[u8]]) -> Option<InjectedFonts> {
    if !font_stack_count_admits(fonts.len()) {
        return None;
    }
    let (fonts, complete) = ordered_font_context(fonts);
    complete.then_some(fonts)
}

fn ordered_font_context(fonts: &[&[u8]]) -> (InjectedFonts, bool) {
    let mut context = blitz_dom::FontContext {
        source_cache: fontique::SourceCache::new_shared(),
        collection: fontique::Collection::new(fontique::CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
    };
    let mut complete = true;
    let (mut order, mut seen) = (Vec::new(), Vec::new());
    for bytes in fonts {
        let decoded = blitz_dom::decode_font_bytes(bytes).into_owned();
        // Upstream returns families from a HashMap; its iteration order must not become policy.
        // 上游按 HashMap 返回族，迭代顺序不得成为回退策略。
        let registered = context
            .collection
            .register_fonts(fontique::Blob::new(Arc::new(decoded) as _), None);
        let mut named: Vec<(String, fontique::FamilyId)> = registered
            .iter()
            .filter_map(|(id, _)| Some((context.collection.family_name(*id)?.to_owned(), *id)))
            .collect();
        if named.is_empty() {
            complete = false;
        }
        named.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, id) in named {
            if font_order_admits(&seen, id.to_u64()) {
                seen.push(id.to_u64());
                order.push(id);
            }
        }
    }
    let bold_aliases = register_bold_aliases(&mut context.collection, &order);
    for generic in [
        fontique::GenericFamily::SansSerif,
        fontique::GenericFamily::Serif,
        fontique::GenericFamily::Monospace,
        fontique::GenericFamily::SystemUi,
    ] {
        context
            .collection
            .set_generic_families(generic, order.iter().copied());
    }
    (
        InjectedFonts {
            context,
            bold_aliases,
        },
        complete,
    )
}

/// CSS weight given to a synthetic bold stand-in face.
/// 合成粗体替身字面的 CSS 字重。
const BOLD_ALIAS_WEIGHT: f32 = 700.0;

fn css_weight(info: &fontique::FontInfo) -> u16 {
    info.weight().value().round().clamp(1.0, 1000.0) as u16
}

/// Synthetic bold: the pinned painter never consumes Parley's embolden synthesis, so for every
/// (family, style, width) group with no face above weight 500 the heaviest face's bytes are
/// registered again under a fresh blob id with weight 700. CSS font matching then selects the
/// stand-in exactly where a real bold face would be selected (requested weight above 500), the
/// shaping and advances stay those of the source face, and the painter emboldens its outlines.
/// Variable-weight faces get no stand-in (Parley drives their `wght` axis). A blob is aliased
/// only when every face it holds qualifies, since the override applies to all of its faces.
/// 合成粗体：固定版绘制器从不消费 Parley 的加粗合成，因此对没有字重 > 500 字面的每个
/// （族、样式、宽度）组，把其中最重字面的字节以新 Blob id、字重 700 再注册一次。
/// CSS 字体匹配随之恰在真实粗体会被选中处（请求字重 > 500）选中替身，塑形与步进仍为源字面，
/// 绘制器加粗其轮廓。可变字重字面不做替身（由 Parley 驱动 `wght` 轴）。覆盖作用于 Blob
/// 中全部字面，故仅当其中每个字面都合格时才为该 Blob 做替身。
fn register_bold_aliases(
    collection: &mut fontique::Collection,
    families: &[fontique::FamilyId],
) -> BTreeSet<u64> {
    let mut faces = Vec::new();
    for id in families {
        if let Some(family) = collection.family(*id) {
            faces.extend(family.fonts().iter().cloned().map(|face| (*id, face)));
        }
    }
    let group_max = |family: fontique::FamilyId, face: &fontique::FontInfo| {
        faces
            .iter()
            .filter(|(other_family, other)| {
                *other_family == family
                    && other.style() == face.style()
                    && other.width() == face.width()
            })
            .map(|(_, other)| css_weight(other))
            .max()
            .unwrap_or(0)
    };
    let mut blobs: Vec<(fontique::Blob<u8>, Vec<u16>, Vec<u16>, bool)> = Vec::new();
    for (family, face) in &faces {
        let fontique::SourceKind::Memory(blob) = face.source().kind() else {
            continue;
        };
        let slot = match blobs.iter().position(|(known, ..)| known.id() == blob.id()) {
            Some(slot) => slot,
            None => {
                blobs.push((blob.clone(), Vec::new(), Vec::new(), false));
                blobs.len() - 1
            }
        };
        let entry = &mut blobs[slot];
        entry.1.push(css_weight(face));
        entry.2.push(group_max(*family, face));
        entry.3 |= face.has_weight_axis();
    }
    let mut aliases = BTreeSet::new();
    for (blob, weights, maxima, variable) in blobs {
        if variable || !bold_alias_admits(&weights, &maxima) {
            continue;
        }
        let alias = fontique::Blob::new(Arc::new(blob) as _);
        let id = alias.id();
        let registered = collection.register_fonts(
            alias,
            Some(fontique::FontInfoOverride {
                weight: Some(fontique::FontWeight::new(BOLD_ALIAS_WEIGHT)),
                ..Default::default()
            }),
        );
        if !registered.is_empty() {
            aliases.insert(id);
        }
    }
    aliases
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

// BEGIN VERIFIED BOLD ALIAS KERNEL
/// Highest CSS weight that still gets a synthetic bold stand-in: above it CSS matching
/// already selects the face itself for bold requests.
/// 仍需合成粗体替身的最高 CSS 字重：高于它时 CSS 匹配本就为粗体请求选中该字面自身。
pub const BOLD_ALIAS_MAX_SOURCE_WEIGHT: u16 = 500;

/// One blob's faces, in order: `weights[i]` is face i's CSS weight and `group_max[i]` the
/// heaviest weight in face i's (family, style, width) group. The blob gets a stand-in only when
/// it holds at least one face, both lists agree in length, and every face is the heaviest of its
/// group with that weight at most BOLD_ALIAS_MAX_SOURCE_WEIGHT.
/// 一个 Blob 的各字面按序：`weights[i]` 为字面 i 的 CSS 字重，`group_max[i]` 为其
/// （族、样式、宽度）组中最重字重。仅当 Blob 至少含一个字面、两表等长、且每个字面都是
/// 本组最重且字重不超过 BOLD_ALIAS_MAX_SOURCE_WEIGHT 时，才为该 Blob 做替身。
pub fn bold_alias_admits(weights: &[u16], group_max: &[u16]) -> bool {
    if weights.len() < 1 || weights.len() != group_max.len() {
        return false;
    }
    let mut index: usize = 0;
    while index < weights.len() {
        if weights[index] != group_max[index] || group_max[index] > BOLD_ALIAS_MAX_SOURCE_WEIGHT {
            return false;
        }
        index += 1;
    }
    true
}
// END VERIFIED BOLD ALIAS KERNEL
