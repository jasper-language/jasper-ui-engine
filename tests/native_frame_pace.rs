//! Headless acceptance of refresh-clock frame pacing on the root native CPU path.
//! 根原生 CPU 路径按刷新时钟定帧节拍的无头验收。
#![cfg(feature="native-blitz-cpu-pending")]
use jasper_ui_engine::engine::Engine;
use jasper_ui_engine::native::host::FramePace;
use jasper_ui_engine::scene::{Node,NodeId,Scene,ScenePatch};
use jasper_ui_engine::style::{Color,Length,Style};

const W:u32=120;
const H:u32=80;
const OWNER:u64=21;
const BUDGET:usize=1024*1024;
/// 60 Hz in nanoseconds, rounded; the host chooses the unit.
const PERIOD:u64=16_666_667;
const ORIGIN:u64=1_000_000_000;

fn scene()->Scene {
    Scene::new(Node::container(NodeId(1)).with_style(Style::default()
            .size(Length::Px(W as f32),Length::Px(H as f32)).background(Color::rgb(10,20,30)))
        .child(Node::rect(NodeId(2)).with_style(Style::default()
            .size(Length::Px(30.0),Length::Px(20.0)).background(Color::rgb(200,40,40)))))
}

fn font()->Vec<u8> {
    let path=std::env::var("JASPER_UI_TEST_FONT").expect("JASPER_UI_TEST_FONT must name an injected Latin font");
    std::fs::read(path).expect("injected font readable")
}

fn render(engine:&mut Engine,version:u64) {
    let mut bytes=vec![0;(W*H*4) as usize];
    assert!(engine.render_to_surface(OWNER,version,&mut bytes,(W*4) as usize,BUDGET));
}

fn recolor(engine:&mut Engine,expected:u64,color:Color)->u64 {
    let style=Style::default().size(Length::Px(30.0),Length::Px(20.0)).background(color);
    assert!(engine.apply(OWNER,expected,expected+1,&[ScenePatch::UpdateStyle {id:NodeId(2),style}]));
    expected+1
}

#[test]
fn one_frame_per_period_and_idle_when_clean() {
    let source=scene();let font=font();let mut v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    // First frame is due; before the first tick the host waits for it.
    assert_eq!(engine.frame_pace(OWNER,None,ORIGIN,PERIOD,ORIGIN-1),Some(FramePace::WaitUntil(ORIGIN)));
    let now=ORIGIN+3*PERIOD+5;let tick=ORIGIN+3*PERIOD;
    assert_eq!(engine.frame_pace(OWNER,None,ORIGIN,PERIOD,now),Some(FramePace::RenderNow(tick)));
    render(&mut engine,v);
    // Painted and nothing changed: no wake-up at all, whatever the clock says.
    assert_eq!(engine.frame_pace(OWNER,Some(tick),ORIGIN,PERIOD,now),Some(FramePace::Idle));
    assert_eq!(engine.frame_pace(OWNER,Some(tick),ORIGIN,PERIOD,now+100*PERIOD),Some(FramePace::Idle));
    // A change inside the same period waits for the next tick instead of a second frame.
    v=recolor(&mut engine,v,Color::rgb(40,200,40));
    assert_eq!(engine.frame_pace(OWNER,Some(tick),ORIGIN,PERIOD,now+1),Some(FramePace::WaitUntil(tick+PERIOD)));
    assert_eq!(engine.frame_pace(OWNER,Some(tick),ORIGIN,PERIOD,tick+PERIOD),Some(FramePace::RenderNow(tick+PERIOD)));
    render(&mut engine,v);
    assert_eq!(engine.needs_frame(OWNER),Some(false));
}

#[test]
fn missed_periods_are_skipped_not_caught_up() {
    let source=scene();let font=font();let mut v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    render(&mut engine,v);
    v=recolor(&mut engine,v,Color::rgb(40,40,200));
    // Host stalled for ~10 periods: exactly one frame for the current period.
    let late=ORIGIN+13*PERIOD+PERIOD/2;
    assert_eq!(engine.frame_pace(OWNER,Some(ORIGIN+2*PERIOD),ORIGIN,PERIOD,late),
        Some(FramePace::RenderNow(ORIGIN+13*PERIOD)));
    render(&mut engine,v);
    assert_eq!(engine.frame_pace(OWNER,Some(ORIGIN+13*PERIOD),ORIGIN,PERIOD,late),Some(FramePace::Idle));
}

#[test]
fn foreign_closed_and_invalid_clock_are_rejected() {
    let source=scene();let font=font();let v=source.version();
    let mut engine=Engine::new(OWNER,&source,&font,W,H).expect("native import");
    assert_eq!(engine.frame_pace(OWNER+1,None,ORIGIN,PERIOD,ORIGIN),None);
    assert_eq!(engine.frame_pace(OWNER,None,ORIGIN,0,ORIGIN),None,"zero interval");
    // Next tick beyond the clock range: no decision rather than a wrapped deadline.
    assert_eq!(engine.frame_pace(OWNER,Some(u64::MAX-1),0,u64::MAX-1,u64::MAX-1),None);
    // Host invalidation (lost surface) makes a frame due within the current period too.
    render(&mut engine,v);
    assert!(engine.invalidate_frame(OWNER));
    assert_eq!(engine.frame_pace(OWNER,None,ORIGIN,PERIOD,ORIGIN),Some(FramePace::RenderNow(ORIGIN)));
    assert!(engine.close_with_focus_events(OWNER,v,&mut |_|{}));
    assert_eq!(engine.frame_pace(OWNER,None,ORIGIN,PERIOD,ORIGIN),None,"closed view");
}
