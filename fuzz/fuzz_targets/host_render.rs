#![no_main]
use jasper_ui_host_surface_verification::{RenderedPixels, SurfaceCoordinator, SurfaceState};
use libfuzzer_sys::fuzz_target;

// The engine answer comes from the input. What may run and what stays acknowledgeable is written
// apart from the production checks; generations stay far below exhaustion, which the Verus contract
// and the unit table cover.
// 引擎的回答来自输入。可运行与可确认的判定与生产检查分开写出；代数远低于耗尽，该边界由 Verus 合约和单元判定表覆盖。
fuzz_target!(|data: &[u8]| {
    const OWNER: u64 = 41;
    let mut coordinator = SurfaceCoordinator::new(OWNER, 320, 200);
    let mut rendered_at = None;
    for byte in data.iter().copied() {
        let before = coordinator;
        let status = coordinator.status(OWNER).unwrap();
        let foreign = byte & 0x10 != 0;
        let stale = byte & 0x20 != 0;
        let answer = byte & 0x40 != 0;
        let caller = if foreign { OWNER + 1 } else { OWNER };
        let expected = status.generation + u64::from(stale);
        let live = !foreign && !stale && status.state == SurfaceState::Active;
        match byte % 6 {
            op @ (0 | 1) => {
                let pixels = if op == 0 {
                    RenderedPixels::AwaitingPresent
                } else {
                    RenderedPixels::Delivered
                };
                let mut asked = 0;
                let result = coordinator.render(caller, expected, pixels, || {
                    asked += 1;
                    if answer { Ok(byte) } else { Err(!byte) }
                });
                assert_eq!(asked, u32::from(live));
                assert_eq!(coordinator.status(OWNER), Some(status));
                if live {
                    assert_eq!(result, Some(if answer { Ok(byte) } else { Err(!byte) }));
                    let waits = answer && op == 0;
                    rendered_at = waits.then_some(status.generation);
                } else {
                    assert_eq!(result, None);
                    assert_eq!(coordinator, before);
                }
            }
            2 => {
                let acknowledged = coordinator.presented(caller, expected);
                assert_eq!(acknowledged, live && rendered_at == Some(status.generation));
                if acknowledged {
                    rendered_at = None;
                }
            }
            3 => {
                let _ = coordinator.reconfigure(caller, expected, 3, 5, || answer);
            }
            4 => {
                let _ = coordinator.suspend(caller, expected, || answer);
            }
            _ => {
                let _ = coordinator.resume(caller, expected, 3, 5, || answer);
            }
        }
        let after = coordinator.status(OWNER).unwrap();
        if after.generation != status.generation {
            rendered_at = None;
        }
        assert_eq!(coordinator.present_pending(), rendered_at.is_some());
        if coordinator.present_pending() {
            assert_eq!(after.state, SurfaceState::Active);
        }
    }
});
