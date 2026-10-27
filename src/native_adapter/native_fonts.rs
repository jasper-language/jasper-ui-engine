//! Configure injected font resources through upstream APIs; no font selection/shaping engine.
//! 仅配置注入字体资源的默认族顺序，复用上游扫描、选择、塑形。
pub(crate) fn injected_font_context(bytes: &[u8]) -> blitz_dom::FontContext {
    let mut context=blitz_dom::build_single_font_ctx(bytes);
    // Font collections can contain several regional families. Upstream registration
    // collects a HashMap: its iteration order must not become per-view fallback policy.
    // Candidate default is lexical family-name order, not a locale preference contract.
    let mut names:Vec<String>=context.collection.family_names().map(str::to_owned).collect();
    names.sort();
    let ids:Vec<_>=names.iter().filter_map(|name|context.collection.family_id(name)).collect();
    for generic in [fontique::GenericFamily::SansSerif,fontique::GenericFamily::Serif,
        fontique::GenericFamily::Monospace,fontique::GenericFamily::SystemUi] {
        context.collection.set_generic_families(generic,ids.iter().copied());
    }
    context
}
