//! Jasper UI native display facade diagnostic, not a text-core/input-service implementation.
mod native_view;
mod component_contract;
mod native_fonts;
mod pointer_host;
mod native_events;
mod native_keys;
mod native_semantics;
mod view_slot;
use native_view::{ComponentUpdate, ContentFlow, NativeTextView, TextAppearance};
use jasper_ui_engine::{input_identity::{UiInputBinding, UiInputContext}, scene::NodeId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("supply an injected font file")?;
    let bytes = std::fs::read(path)?;
    // Existing scene prototype now consumes the native adapter; text service fixture follows.
    // 现有场景原型经原生适配器执行；下方保留独立的外部文本服务客户端夹具。
    {
        use jasper_ui_blitz_host_probe::{Scene,Node,ScenePatch,Semantics,Role,style::{Style,Length,Color},host::SceneHostView};
        #[cfg(feature="cpu-pending")]
        let render=|view:&mut SceneHostView,version:u64|->Option<Vec<u8>> {
            let mut pixels=vec![0;640*480*4];
            view.render_to_surface(10,version,&mut pixels,640*4,4*1024*1024).then_some(pixels)
        };
        let mut title=Node::text(NodeId(2),"Jasper native scene 中文");
        title.semantics=Semantics::default().role(Role::Text).label("Scene title");
        let mut button=Node::text(NodeId(4),"Native focus target");
        button.semantics=Semantics {role:Role::Button,focusable:true,..Default::default()};
        let mut source=Scene::new(Node::container(NodeId(1)).child(title)
            .child(Node::rect(NodeId(3)).with_style(Style::default().size(Length::Px(120.0),Length::Px(32.0)).background(Color::BLUE))).child(button));
        let mut native=SceneHostView::new(10,&source,&bytes,640,480)
            .ok_or("existing scene native import failed")?;
        let semantics=native.semantics(10,source.version()).ok_or("source semantics unavailable")?;
        if semantics.nodes.len()!=4 || semantics.root!=NodeId(1)
            || semantics.nodes.iter().find(|node|node.node==NodeId(2)).map(|node|node.semantics.label.as_str())!=Some("Scene title") {
            return Err("existing scene identity/semantics projection failed".into());
        }
        #[cfg(feature="cpu-pending")]
        let before=render(&mut native,source.version()).ok_or("source scene render failed")?;
        let expected=source.version();
        let patches=[ScenePatch::UpdateText {id:NodeId(2),text:"Updated native scene 中文".into()},
            ScenePatch::UpdateStyle {id:NodeId(3),style:Style::default().size(Length::Px(160.0),Length::Px(40.0)).background(Color::GREEN)},
            ScenePatch::Insert {parent:NodeId(1),index:1,node:Node::text(NodeId(5),"Inserted through source frame")},
            ScenePatch::Remove {id:NodeId(3)}];
        source.apply_batch_at(expected,&patches).map_err(|_|"source scene batch failed")?;
        if !native.apply(10,expected,source.version(),&patches)
            || native.apply(10,expected,source.version(),&patches) {
            return Err("native scene incremental/stale update failed".into());
        }
        let mut activated=None;
        if native.activate(10,expected,NodeId(4),&mut |_| {})
            || !native.activate(10,source.version(),NodeId(4),&mut |event| {
                activated=Some((event.scene_version,event.node));
            }) || activated!=Some((source.version(),NodeId(4))) {
            return Err("scene prototype semantic action routing failed".into());
        }
        #[cfg(feature="cpu-pending")]
        {
            let after=render(&mut native,source.version()).ok_or("updated source scene render failed")?;
            if after==before {return Err("source scene patch did not change pixels".into());}
            let stride=640*4+16;let mut host_surface=vec![0xa5;479*stride+640*4];
            if !native.render_to_surface(10,source.version(),&mut host_surface,stride,4*1024*1024) {
                return Err("injected software surface adaptation failed".into());
            }
            let mut reference=SceneHostView::new(10,&source,&bytes,640,480)
                .ok_or("updated source reference import failed")?;
            if render(&mut reference,source.version()).as_deref()!=Some(after.as_slice()) {
                return Err("incremental source frame differs from fresh native import".into());
            }
            if !native.set_viewport_scroll(10,source.version(),0.0,24.0)
                || !native.set_viewport_scroll(10,source.version(),0.0,0.0)
                || render(&mut native,source.version()).as_deref()!=Some(after.as_slice()) {
                return Err("scene prototype viewport restoration failed".into());
            }
            println!("existing Scene native CPU frame bytes={}; acceptance pending",after.len());
        }
        let mut focused=false;
        if !native.focus(10,source.version(),NodeId(4),&mut |event| {focused=event.node==NodeId(4) && event.focused;})
            || !focused {return Err("scene prototype focus request failed".into());}
        let mut retired_focus=false;
        if !native.set_viewport(10,0,0,1.0)
            || !native.set_viewport(10,640,480,1.0)
            || !native.close_with_focus_events(10,source.version(),&mut |event| {retired_focus=event.node==NodeId(4) && !event.focused;})
            || !retired_focus || native.semantics(10,source.version()).is_some() {
            return Err("native scene surface/retirement fixture failed".into());
        }
    }
    let context = UiInputContext { target: NodeId(7), target_generation: 1, document: 20, revision: 1 };
    let mut view = NativeTextView::new(10, context, &bytes, "Jasper UI 中文 A😀B", 640, 480);
    if !view.apply_display_snapshot_from_host(10, context, context, "外部服务提供的显示快照") || !view.resize(800, 600) {
        return Err("live view update rejected".into());
    }
    let appearance = TextAppearance {
        font_size_px: 24, padding_px: 12,
        foreground: [28, 70, 110], background: [240, 245, 250],
    };
    if !apply_now(&mut view, 10, context, ComponentUpdate::Appearance(appearance)) { return Err("appearance update rejected".into()); }
    if apply_now(&mut view, 10, context, ComponentUpdate::Appearance(TextAppearance { font_size_px: 0, ..appearance })) {
        return Err("invalid appearance accepted".into());
    }
    if apply_now(&mut view, 11, context, ComponentUpdate::Appearance(appearance))
        || apply_now(&mut view, 10, UiInputContext { target_generation: 2, ..context }, ComponentUpdate::Appearance(appearance)) {
        return Err("foreign/stale component update accepted".into());
    }
    if view.viewport_size() != Some((800, 600)) { return Err("viewport mapping failed".into()); }
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY,
        f32::MAX, f32::MIN_POSITIVE, f32::from_bits(1), 0.0078125, 128.0] {
        if view.set_host_scale(invalid) || view.host_scale() != Some(1.0) {
            return Err("invalid host scale changed the viewport".into());
        }
    }
    if !view.set_host_scale(1.5) || !view.resize(800, 600)
        || view.host_scale() != Some(1.5) || view.viewport_size() != Some((800, 600)) {
        return Err("host DPI/size mapping failed".into());
    }
    // Trusted-host fixture: sessions/context are supplied by the host, not clients.
    let mut binding = UiInputBinding::new(10, 1);
    let ticket = binding.bind(10, context).ok_or("binding failed")?;
    let mut foreign_binding = UiInputBinding::new(11, 1);
    let foreign_ticket = foreign_binding.bind(11, context).ok_or("foreign fixture binding failed")?;
    if set_preedit_now(&mut view, &foreign_binding, 11, foreign_ticket, Some("foreign")) {
        return Err("other application's valid lease changed this view".into());
    }
    let preedit = Some("预编辑：尚未提交");
    let clear = None;
    if binding.admit(11, ticket, &preedit).is_some() { return Err("foreign session admitted".into()); }
    for payload in [&preedit, &clear, &preedit] {
        if !set_preedit_now(&mut view, &binding, 10, ticket, *payload) { return Err("admitted display rejected".into()); }
    }
    #[cfg(feature = "cpu-pending")]
    let before_focus_close = view.render_cpu(4 * 1024 * 1024).ok_or("focus-close baseline failed")?;
    if !view.close_focus_from_host(&mut binding, 10, ticket)
        || set_preedit_now(&mut view, &binding, 10, ticket, preedit) {
        return Err("focus close failed to revoke input".into());
    }
    #[cfg(feature = "cpu-pending")]
    if view.render_cpu(4 * 1024 * 1024).ok_or("focus-close frame failed")? == before_focus_close {
        return Err("focus close did not clear preedit pixels".into());
    }
    let successor = binding.bind(10, context).ok_or("focus reopen failed")?;
    if !set_preedit_now(&mut view, &binding, 10, successor, preedit)
        || view.close_focus_from_host(&mut binding, 10, ticket) {
        return Err("stale focus close affected successor".into());
    }
    #[cfg(feature = "cpu-pending")]
    {
        let before_stale_close = view.render_cpu(4 * 1024 * 1024).ok_or("stale-close baseline failed")?;
        if view.close_focus_from_host(&mut binding, 10, ticket)
            || !binding.accepts(10, successor)
            || view.render_cpu(4 * 1024 * 1024).ok_or("stale-close frame failed")? != before_stale_close {
            return Err("stale focus close changed successor display/binding".into());
        }
    }
    let next = binding.replace(10, successor, UiInputContext { target: NodeId(8), ..context })
        .ok_or("focus replacement failed")?;
    if binding.admit(10, ticket, &preedit).is_some() { return Err("stale focus admitted".into()); }
    if set_preedit_now(&mut view, &binding, 10, next, preedit) { return Err("other target changed this view".into()); }
    let restored = binding.replace(10, next, context).ok_or("focus return failed")?;
    if binding.admit(10, next, &preedit).is_some() { return Err("replaced target admitted".into()); }
    if !set_preedit_now(&mut view, &binding, 10, restored, preedit) {
        return Err("restored target rejected".into());
    }
    let next_revision = UiInputContext { revision: 2, ..context };
    if !view.apply_display_snapshot_from_host(10, context, next_revision, "外部文档修订后的显示快照")
        || view.apply_display_snapshot_from_host(10, context, next_revision, "迟到旧快照")
        || set_preedit_now(&mut view, &binding, 10, restored, preedit) {
        return Err("display revision admission failed".into());
    }
    #[cfg(feature = "cpu-pending")]
    let accepted_snapshot_pixels = view.render_cpu(4 * 1024 * 1024).ok_or("snapshot render rejected")?;
    if view.apply_display_snapshot_from_host(11, next_revision, next_revision, "foreign session")
        || view.apply_display_snapshot_from_host(10, next_revision,
            UiInputContext { document: 21, ..next_revision }, "other document")
        || view.apply_display_snapshot_from_host(10, context, next_revision, "stale snapshot") {
        return Err("invalid display snapshot accepted".into());
    }
    #[cfg(feature = "cpu-pending")]
    if view.render_cpu(4 * 1024 * 1024).ok_or("post-rejection render failed")? != accepted_snapshot_pixels {
        return Err("rejected snapshot changed visible pixels".into());
    }
    let revision_ticket = binding.replace(10, restored, next_revision).ok_or("revision binding failed")?;
    if !set_preedit_now(&mut view, &binding, 10, revision_ticket, preedit) {
        return Err("current revision input rejected".into());
    }
    if !view.set_selection_from_host(&binding, 10, revision_ticket, Some((0, 6)))
        || view.set_selection_from_host(&binding, 10, revision_ticket, Some((1, 6)))
        || view.set_selection_from_host(&binding, 10, revision_ticket, Some((0, usize::MAX)))
        || view.set_selection_from_host(&binding, 10, restored, Some((0, 6))) {
        return Err("external selection admission failed".into());
    }
    #[cfg(feature = "cpu-pending")]
    {
        let selected = view.render_cpu(4 * 1024 * 1024).ok_or("selection render failed")?;
        if view.set_selection_from_host(&binding, 10, revision_ticket, Some((1, 6))) {
            return Err("split codepoint selection accepted".into());
        }
        if view.render_cpu(4 * 1024 * 1024).ok_or("rejection frame failed")? != selected {
            return Err("rejected selection changed pixels".into());
        }
        if !view.set_selection_from_host(&binding, 10, revision_ticket, None) {
            return Err("selection clear rejected".into());
        }
        let cleared = view.render_cpu(4 * 1024 * 1024).ok_or("clear frame failed")?;
        if cleared == selected { return Err("selection clear did not change pixels".into()); }
        if !view.set_selection_from_host(&binding, 10, revision_ticket, Some((6, 0))) {
            return Err("reverse selection rejected".into());
        }
        if view.render_cpu(4 * 1024 * 1024).ok_or("reverse frame failed")? != selected {
            return Err("reverse selection changed highlighted range".into());
        }
    }
    if view.hit_display_text_from_host(&binding, 10, revision_ticket, 30.0, 30.0).is_none()
        || view.hit_display_text_from_host(&binding, 10, revision_ticket, f32::NAN, 30.0).is_some()
        || view.hit_display_text_from_host(&binding, 10, revision_ticket, f32::INFINITY, 30.0).is_some()
        || view.hit_display_text_from_host(&binding, 10, restored, 30.0, 30.0).is_some()
        || view.hit_display_text_from_host(&binding, 10, revision_ticket, 100000.0, 100000.0).is_some() {
        return Err("display text hit admission failed".into());
    }
    #[cfg(feature = "cpu-pending")]
    {
        if !apply_now(&mut view, 10, next_revision, ComponentUpdate::Flow { direction: ContentFlow::Column, gap_px: 8 }) { return Err("column layout rejected".into()); }
        let column = view.render_cpu(4 * 1024 * 1024).ok_or("column render failed")?;
        if !apply_now(&mut view, 10, next_revision, ComponentUpdate::Flow { direction: ContentFlow::Row, gap_px: 8 }) { return Err("row layout rejected".into()); }
        if view.render_cpu(4 * 1024 * 1024).ok_or("row render failed")? == column {
            return Err("typed flow update did not affect layout".into());
        }
        if !apply_now(&mut view, 10, next_revision, ComponentUpdate::Flow { direction: ContentFlow::Column, gap_px: 8 })
            || view.render_cpu(4 * 1024 * 1024).ok_or("restored flow render failed")? != column {
            return Err("typed flow update did not restore layout".into());
        }
    }
    #[cfg(feature = "cpu-pending")]
    {
        let before_theme = view.render_cpu(4 * 1024 * 1024).ok_or("appearance baseline failed")?;
        let alternate = TextAppearance { foreground: [235, 240, 250], background: [24, 30, 40], ..appearance };
        if apply_now(&mut view, 11, next_revision, ComponentUpdate::Appearance(alternate))
            || apply_now(&mut view, 10, context, ComponentUpdate::Appearance(alternate))
            || view.render_cpu(4 * 1024 * 1024).ok_or("rejected component frame failed")? != before_theme {
            return Err("rejected component update changed visible state".into());
        }
        if !apply_now(&mut view, 10, next_revision, ComponentUpdate::Appearance(alternate))
            || !binding.accepts(10, revision_ticket)
            || binding.accepts(10, restored)
            || view.render_cpu(4 * 1024 * 1024).ok_or("alternate appearance render failed")? == before_theme {
            return Err("appearance update lost identity or did not update pixels".into());
        }
        if !apply_now(&mut view, 10, next_revision, ComponentUpdate::Appearance(appearance))
            || view.render_cpu(4 * 1024 * 1024).ok_or("restored appearance render failed")? != before_theme {
            return Err("appearance round trip lost display/selection/preedit state".into());
        }
    }
    #[cfg(feature = "cpu-pending")]
    let before_label = view.render_cpu(4 * 1024 * 1024).ok_or("label baseline failed")?;
    if !apply_now(&mut view, 10, next_revision, ComponentUpdate::AccessibleLabel(Some("文档内容显示")))
        || view.accessible_label() != Some("文档内容显示")
        || apply_now(&mut view, 11, next_revision, ComponentUpdate::AccessibleLabel(Some("foreign")))
        || view.accessible_label() != Some("文档内容显示") {
        return Err("accessibility label mapping/admission failed".into());
    }
    if !apply_now(&mut view, 10, next_revision, ComponentUpdate::AccessibleLabel(Some("")))
        || view.accessible_label() != Some("") {
        return Err("explicit empty label confused with missing label".into());
    }
    if !apply_now(&mut view, 10, next_revision, ComponentUpdate::AccessibleLabel(None))
        || view.accessible_label().is_some() {
        return Err("accessibility label clear failed".into());
    }
    #[cfg(feature = "cpu-pending")]
    if view.render_cpu(4 * 1024 * 1024).ok_or("label round trip frame failed")? != before_label
        || !binding.accepts(10, revision_ticket) {
        return Err("label metadata updates changed display or input binding".into());
    }
    let current_update = view.component_version().ok_or("component sequence absent")?;
    if view.apply_component_update(10, next_revision, current_update.saturating_sub(1), ComponentUpdate::AccessibleLabel(Some("stale")))
        || view.apply_component_update(10, next_revision, u64::MAX, ComponentUpdate::AccessibleLabel(Some("overflow")))
        || view.component_version() != Some(current_update) || view.accessible_label().is_some() {
        return Err("stale component sequence changed state".into());
    }
    if view.selected_display_text_from_host(&binding, 10, revision_ticket).as_deref() != Some("外部")
        || view.selected_display_text_from_host(&binding, 11, revision_ticket).is_some()
        || view.selected_display_text_from_host(&binding, 10, restored).is_some() {
        return Err("selected display text readback/admission failed".into());
    }
    if !view.set_selection_from_host(&binding, 10, revision_ticket, None)
        || view.selected_display_text_from_host(&binding, 10, revision_ticket).is_some()
        || !view.set_selection_from_host(&binding, 10, revision_ticket, Some((6, 0)))
        || view.selected_display_text_from_host(&binding, 10, revision_ticket).as_deref() != Some("外部") {
        return Err("selection clear/readback retained stale content".into());
    }
    let generation2 = UiInputContext { target_generation: 2, ..context };
    let mut reopened = NativeTextView::new(10, generation2, &bytes, "新一代视图", 320, 200);
    if set_preedit_now(&mut reopened, &binding, 10, restored, preedit) {
        return Err("old generation changed reopened view".into());
    }
    let fresh = binding.replace(10, revision_ticket, generation2).ok_or("reopen binding failed")?;
    if set_preedit_now(&mut view, &binding, 10, fresh, preedit)
        || !set_preedit_now(&mut reopened, &binding, 10, fresh, preedit) {
        return Err("view generation isolation failed".into());
    }
    reopened.close();
    if reopened.close_focus_from_host(&mut binding, 10, fresh) || !binding.accepts(10, fresh) {
        return Err("closed view changed external binding".into());
    }
    if set_preedit_now(&mut reopened, &binding, 10, fresh, preedit)
        || reopened.set_selection_from_host(&binding, 10, fresh, Some((0, 3)))
        || reopened.hit_display_text_from_host(&binding, 10, fresh, 30.0, 30.0).is_some()
        || reopened.selected_display_text_from_host(&binding, 10, fresh).is_some() {
        return Err("closed matching generation accepted input".into());
    }
    #[cfg(feature = "cpu-pending")]
    {
        let frame = view.render_cpu(4 * 1024 * 1024).ok_or("CPU frame rejected")?;
        std::fs::create_dir_all("evidence/blitz-cpu-view-pending")?;
        std::fs::write("evidence/blitz-cpu-view-pending/frame.rgba", &frame)?;
        println!("native UI CPU frame bytes={}; production acceptance pending", frame.len());
    }
    #[cfg(not(feature = "cpu-pending"))]
    println!("native UI view constructed; painting and production acceptance pending");
    if !view.close() || view.close() || view.apply_display_snapshot_from_host(10, next_revision, next_revision, "late")
        || apply_now(&mut view, 10, next_revision, ComponentUpdate::Flow { direction: ContentFlow::Row, gap_px: 0 }) || view.resize(10, 10) || view.set_host_scale(2.0) || view.host_scale().is_some()
        || set_preedit_now(&mut view, &binding, 10, restored, preedit)
        || apply_now(&mut view, 10, next_revision, ComponentUpdate::Appearance(appearance))
        || view.apply_component_update(10, next_revision, current_update, ComponentUpdate::AccessibleLabel(Some("late")))
        || view.accessible_label().is_some() || view.viewport_size().is_some() {
        return Err("closed view accepted a late operation".into());
    }
    #[cfg(feature = "cpu-pending")]
    if view.render_cpu(4 * 1024 * 1024).is_some() {
        return Err("closed view rendered".into());
    }
    Ok(())
}

// Diagnostic host fixture only: real producers retain the version of their own update.
fn apply_now(view: &mut NativeTextView, caller: u64, context: UiInputContext, update: ComponentUpdate<'_>) -> bool {
    let Some(version) = view.component_version() else { return false; };
    let applied = view.apply_component_update(caller, context, version, update);
    let expected = if applied { version.checked_add(1) } else { Some(version) };
    assert_eq!(view.component_version(), expected, "dispatch result/version disagreement");
    applied
}

// Diagnostic convenience only: asynchronous producers must retain the original token.
fn set_preedit_now(view: &mut NativeTextView, binding: &UiInputBinding, caller: u64,
    ticket: jasper_ui_engine::input_identity::UiInputTicket, text: Option<&str>) -> bool {
    let Some(token) = view.begin_preedit_from_host(binding, caller, ticket) else { return false; };
    view.set_preedit_from_host(binding, caller, token, text)
}
