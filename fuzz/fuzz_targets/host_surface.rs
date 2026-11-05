#![no_main]
use jasper_ui_host_surface_verification::{
    retire_current, SurfaceLifecycle, SurfaceLifecycleError, SurfaceState, SurfaceStatus,
};
use libfuzzer_sys::fuzz_target;

// Generations stay far below exhaustion here: the public API starts at one and advances by one, so
// that edge is left to the Verus contract and the module's own table test.
// 这里的代数远低于耗尽：公开 API 从一开始、每次加一，该边界交给 Verus 合约和模块自身的判定表测试。
fuzz_target!(|data: &[u8]| {
    let owner = data
        .get(..8)
        .map_or(0, |bytes| u64::from_le_bytes(bytes.try_into().unwrap()));
    let other = owner.wrapping_add(1);
    let mut surface = SurfaceLifecycle::new(owner, 320, 200);
    let mut seen: Vec<SurfaceStatus> = Vec::new();
    for byte in data.iter().copied() {
        let before = surface.status(owner).unwrap();
        let foreign = byte & 0x10 != 0;
        let stale = byte & 0x20 != 0;
        let caller = if foreign { other } else { owner };
        let expected = before.generation + u64::from(stale);
        let (width, height) = (u32::from(byte), u32::from(byte) + 1);
        let op = byte % 5;
        let admitted = match op {
            0 | 1 => before.state == SurfaceState::Active,
            2 => before.state == SurfaceState::Suspended,
            _ => before.state != SurfaceState::Retired,
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
        let transition = match op {
            0 => surface.reconfigure(caller, expected, width, height),
            1 => surface.suspend(caller, expected),
            2 => surface.resume(caller, expected, width, height),
            3 => surface.retire(caller, expected).map(|_| expected + 1),
            _ => {
                let accepted = retire_current(&mut surface, caller);
                assert_eq!(accepted, !foreign && before.state != SurfaceState::Retired);
                if accepted {
                    Ok(before.generation + 1)
                } else {
                    Err(SurfaceLifecycleError::WrongState)
                }
            }
        };
        let after = surface.status(owner).unwrap();
        assert_eq!(surface.status(other), None);
        if op != 4 {
            match refusal {
                Some(error) => assert_eq!(transition, Err(error)),
                None => assert_eq!(transition, Ok(before.generation + 1)),
            }
        }
        match transition {
            Ok(generation) => {
                assert_eq!(after.generation, generation);
                assert_eq!(after.generation, before.generation + 1);
                let expected_state = match op {
                    0 | 2 => SurfaceState::Active,
                    1 => SurfaceState::Suspended,
                    _ => SurfaceState::Retired,
                };
                assert_eq!(after.state, expected_state);
                let extent = if op == 0 || op == 2 {
                    (width, height)
                } else {
                    (before.width, before.height)
                };
                assert_eq!((after.width, after.height), extent);
            }
            Err(_) => assert_eq!(after, before),
        }
        assert_eq!(
            surface.render_allowed(owner, after.generation),
            after.state == SurfaceState::Active
        );
        assert!(!surface.render_allowed(other, after.generation));
        // A generation that may render again names the state it first named.
        // 再次可渲染的代数，对应的是它第一次对应的状态。
        for earlier in &seen {
            if surface.render_allowed(owner, earlier.generation) {
                assert_eq!(after, *earlier);
            }
        }
        if after.state == SurfaceState::Active {
            seen.push(after);
        }
    }
});
