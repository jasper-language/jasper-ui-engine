#![no_main]
use jasper_ui_host_surface_verification::{
    SurfaceCoordinator, SurfaceLifecycleError, SurfaceState, SurfaceStepError,
};
use libfuzzer_sys::fuzz_target;

// The engine answer comes from the input. The table is written apart from the production checks;
// generations stay far below exhaustion, which the Verus contract and the unit table cover.
// 引擎的回答来自输入。判定表与生产检查分开写出；代数远低于耗尽，该边界由 Verus 合约和单元判定表覆盖。
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
        let (width, height) = (u32::from(byte), u32::from(byte) + 1);
        let op = byte % 8;
        let mut asked = 0;
        let engine = || {
            asked += 1;
            answer
        };
        let result = match op {
            0 => Some(coordinator.reconfigure(caller, expected, width, height, engine)),
            1 => Some(coordinator.suspend(caller, expected, engine)),
            2 => Some(coordinator.resume(caller, expected, width, height, engine)),
            3 => Some(coordinator.retire(caller, expected, engine).map(|()| 0)),
            4 => {
                let closed = coordinator.close(caller, engine);
                let may_close = !foreign && status.state != SurfaceState::Retired;
                assert_eq!(closed, may_close && answer);
                assert_eq!(asked, u32::from(may_close));
                None
            }
            5 => {
                let marked = coordinator.texture_rendered(caller, expected);
                assert_eq!(marked, !foreign && !stale && status.state == SurfaceState::Active);
                if marked {
                    rendered_at = Some(status.generation);
                }
                None
            }
            6 => {
                let acknowledged = coordinator.presented(caller, expected);
                assert_eq!(
                    acknowledged,
                    !foreign
                        && !stale
                        && status.state == SurfaceState::Active
                        && rendered_at == Some(status.generation)
                );
                if acknowledged {
                    rendered_at = None;
                }
                None
            }
            _ => {
                coordinator.frame_superseded();
                rendered_at = None;
                None
            }
        };
        if let Some(result) = result {
            let admitted = match op {
                0 | 1 => status.state == SurfaceState::Active,
                2 => status.state == SurfaceState::Suspended,
                _ => status.state != SurfaceState::Retired,
            };
            let refusal = if foreign {
                Some(SurfaceLifecycleError::WrongOwner)
            } else if stale {
                Some(SurfaceLifecycleError::StaleGeneration)
            } else if !admitted {
                Some(SurfaceLifecycleError::WrongState)
            } else {
                None
            };
            match (refusal, answer) {
                (Some(error), _) => {
                    assert_eq!(result, Err(SurfaceStepError::Lifecycle(error)));
                    assert_eq!(asked, 0);
                    assert_eq!(coordinator, before);
                }
                (None, false) => {
                    assert_eq!(result, Err(SurfaceStepError::EngineRejected));
                    assert_eq!(asked, 1);
                    assert_eq!(coordinator, before);
                }
                (None, true) => {
                    assert!(result.is_ok());
                    assert_eq!(asked, 1);
                    assert!(!coordinator.present_pending());
                    assert_eq!(
                        coordinator.status(OWNER).unwrap().generation,
                        status.generation + 1
                    );
                }
            }
        }
        let after = coordinator.status(OWNER).unwrap();
        if after.generation != status.generation {
            rendered_at = None;
        }
        assert_eq!(coordinator.present_pending(), rendered_at == Some(after.generation));
        if coordinator.present_pending() {
            assert_eq!(after.state, SurfaceState::Active);
        }
    }
});
