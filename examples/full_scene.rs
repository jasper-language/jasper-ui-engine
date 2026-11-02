//! End-to-end headless caller: scene declaration, versioned update, layout, CPU presentation,
//! pointer activation, external display/IME client lifecycle and retirement.
//! 端到端无头调用方：场景声明、版本化更新、布局、CPU 呈现、指针激活、外部显示/IME 客户端
//! 生命周期及退休。
//! Run: cargo run --features native-engine-cpu --example full_scene -- FONT [OUTPUT].

use jasper_ui_engine::backend::HeadlessBackend;
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::geometry::Edges;
use jasper_ui_engine::native::host::{
    TextDisplayView,UiInputBinding,UiInputContext,
};
use jasper_ui_engine::native::{EventReply,PointerAction,SceneEventKind,SceneUpdate};
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::semantics::{Role,Semantics};
use jasper_ui_engine::style::{Color,FontStyle,Length,Style};

const SCENE_OWNER:u64=10;
const TEXT_OWNER:u64=20;
const W:u32=320;
const H:u32=180;
const BUDGET:usize=4*1024*1024;
const BUTTON:NodeId=NodeId(2);
const TEXT_TARGET:NodeId=NodeId(9);

fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut arguments=std::env::args().skip(1);
    let font_path=arguments.next().ok_or("supply an injected font file")?;
    let output=arguments.next().unwrap_or_else(||"full_scene.ppm".into());
    if arguments.next().is_some() {return Err("expected FONT [OUTPUT]".into());}
    let font=std::fs::read(font_path)?;
    if let Some(parent)=std::path::Path::new(&output).parent().filter(|path|!path.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }

    let button=Node::text(BUTTON,"Activate")
        .with_semantics(Semantics::default().role(Role::Button).label("Activate").focusable(true))
        .with_style(Style::default().size(Length::Px(140.0),Length::Px(44.0))
            .background(Color::rgb(40,130,90)));
    let mut source=Scene::new(Node::container(NodeId(1)).with_style(Style::default()
        .size(Length::Px(W as f32),Length::Px(H as f32)).padding(Edges::all(20.0))
        .background(Color::rgb(18,22,30))).child(button));
    let mut engine=Engine::new(SCENE_OWNER,&source,&font,W,H).ok_or("native scene rejected")?;
    if !engine.focus(SCENE_OWNER,source.version(),BUTTON,&mut |_|{}) {
        return Err("Framework focus request rejected".into());
    }

    let mut activations=Vec::new();
    if !engine.pointer(SCENE_OWNER,source.version(),PointerAction::Down,70.0,42.0,true,
        &mut |_|EventReply::default(),&mut |_|{}) {
        return Err("native pointer down rejected".into());
    }
    let old=source.version();
    let patches=[ScenePatch::UpdateStyle {id:BUTTON,style:Style::default()
        .size(Length::Px(160.0),Length::Px(48.0)).background(Color::rgb(48,160,105))
        .font(700,FontStyle::Normal)}];
    source.apply_batch_at(old,&patches)
        .map_err(|error|format!("source update rejected: {error:?}"))?;
    if !engine.apply_update(SCENE_OWNER,SceneUpdate {
        expected_version:old,next_version:source.version(),patches:&patches}) {
        return Err("visual update rejected".into());
    }
    if engine.focused_node(SCENE_OWNER,source.version())!=Some(BUTTON) {
        return Err("visual update lost focus".into());
    }
    if !engine.pointer(SCENE_OWNER,source.version(),PointerAction::Up,70.0,42.0,false,
        &mut |event|{if event.kind==SceneEventKind::Activate {activations.push(event.node);}EventReply::default()},
        &mut |_|{}) {
        return Err("native pointer up rejected".into());
    }
    if activations!=[BUTTON] {return Err("expected exactly one activation".into());}

    let mut backend=HeadlessBackend::new(1,1);
    engine.render_to_backend(SCENE_OWNER,source.version(),&mut backend,BUDGET)
        .map_err(|error|format!("CPU presentation rejected: {error:?}"))?;
    backend.save_ppm(&output)?;

    let context=UiInputContext {target:TEXT_TARGET,target_generation:1,document:2,revision:3};
    let next_context=UiInputContext {revision:4,..context};
    let mut display=TextDisplayView::new(TEXT_OWNER,context,&font,"External service document",W,H);
    let initial_display=display.render_cpu(TEXT_OWNER,context,BUDGET).ok_or("initial text frame rejected")?;
    let mut binding=UiInputBinding::new(TEXT_OWNER,1);
    let ticket=binding.bind(TEXT_OWNER,context).ok_or("text binding rejected")?;
    if !display.focus(&binding,TEXT_OWNER,ticket,&mut |_|{}) {
        return Err("text focus rejected".into());
    }
    if !display.snapshot(TEXT_OWNER,context,next_context,"External service successor") {
        return Err("external display snapshot rejected".into());
    }
    let successor_display=display.render_cpu(TEXT_OWNER,next_context,BUDGET)
        .ok_or("successor text frame rejected")?;
    if successor_display==initial_display {return Err("external display successor did not repaint".into());}
    let successor=binding.replace(TEXT_OWNER,ticket,next_context).ok_or("binding rotation rejected")?;
    let token=display.begin_preedit(&binding,TEXT_OWNER,successor).ok_or("preedit begin rejected")?;
    if !display.preedit(&binding,TEXT_OWNER,token,Some("compose")) {
        return Err("preedit update rejected".into());
    }
    let composed=display.render_cpu(TEXT_OWNER,next_context,BUDGET).ok_or("preedit frame rejected")?;
    if composed==successor_display {return Err("preedit did not repaint".into());}
    if !display.cancel_preedit(TEXT_OWNER,token) {
        return Err("preedit cancel rejected".into());
    }
    if display.render_cpu(TEXT_OWNER,next_context,BUDGET).as_ref()!=Some(&successor_display) {
        return Err("preedit cancellation did not restore successor display".into());
    }
    if !display.close_focus(&mut binding,TEXT_OWNER,successor,&mut |_|{}) {
        return Err("text focus lease close rejected".into());
    }
    if !display.close(TEXT_OWNER,next_context) {
        return Err("text view retirement rejected".into());
    }

    if !engine.deactivate(SCENE_OWNER,source.version(),&mut |_|{}) ||
        !engine.close(SCENE_OWNER) {
        return Err("scene retirement rejected".into());
    }
    println!("full scene → update → render → input → external text → retire: {output}");
    Ok(())
}
