//! Reference gesture recognizer on top of routed pointer events.
//! 宿主中立原语；Jasper UI 层可以替换/覆盖手势语义。

use std::collections::BTreeMap;

use crate::geometry::Point;
use crate::input::{PointerId, PointerPhase};
use crate::pointer::PointerRoute;
use crate::scene::NodeId;

/// Tunable thresholds for the reference recognizer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GestureConfig {
    pub tap_max_duration_ms: u64,
    pub tap_max_distance: f32,
    pub long_press_ms: u64,
    pub pan_threshold: f32,
    pub pinch_threshold: f32,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            tap_max_duration_ms: 250,
            tap_max_distance: 8.0,
            long_press_ms: 500,
            pan_threshold: 8.0,
            pinch_threshold: 8.0,
        }
    }
}

/// Emitted gesture. `target` is the capture/target node from the router.
#[derive(Clone, Debug, PartialEq)]
pub enum Gesture {
    Tap {
        pointer_id: PointerId,
        target: Option<NodeId>,
        position: Point,
    },
    LongPress {
        pointer_id: PointerId,
        target: Option<NodeId>,
        position: Point,
    },
    PanStart {
        pointer_id: PointerId,
        target: Option<NodeId>,
        position: Point,
    },
    Pan {
        pointer_id: PointerId,
        target: Option<NodeId>,
        delta: Point,
        total: Point,
    },
    PanEnd {
        pointer_id: PointerId,
        target: Option<NodeId>,
        position: Point,
        total: Point,
    },
    PinchStart {
        target: Option<NodeId>,
        center: Point,
        distance: f32,
    },
    Pinch {
        target: Option<NodeId>,
        center: Point,
        scale: f32,
        delta: f32,
    },
    PinchEnd {
        target: Option<NodeId>,
        center: Point,
    },
    Cancelled {
        pointer_id: PointerId,
    },
}

#[derive(Clone, Debug)]
struct PointerState {
    start: Point,
    last: Point,
    target: Option<NodeId>,
    started_at: u64,
    panning: bool,
    long_pressed: bool,
}

#[derive(Clone, Debug)]
struct PinchState {
    first: PointerId,
    second: PointerId,
    initial_distance: f32,
    last_distance: f32,
    last_center: Point,
    target: Option<NodeId>,
}

/// Stateful recognizer; feed it `PointerRoute`s and `tick` it for long-press.
#[derive(Clone, Debug)]
pub struct GestureRecognizer {
    config: GestureConfig,
    pointers: BTreeMap<PointerId, PointerState>,
    pinch: Option<PinchState>,
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new(GestureConfig::default())
    }
}

impl GestureRecognizer {
    pub fn new(config: GestureConfig) -> Self {
        Self {
            config,
            pointers: BTreeMap::new(),
            pinch: None,
        }
    }

    pub fn config(&self) -> GestureConfig {
        self.config
    }

    /// Processes one routed pointer sample.
    pub fn process(&mut self, route: &PointerRoute) -> Vec<Gesture> {
        let event = &route.event;
        let mut out = Vec::new();
        match event.phase {
            PointerPhase::Down => {
                self.pointers.insert(
                    event.pointer_id,
                    PointerState {
                        start: event.position,
                        last: event.position,
                        target: route.target,
                        started_at: event.timestamp_millis,
                        panning: false,
                        long_pressed: false,
                    },
                );
                self.update_pinch(&mut out);
            }
            PointerPhase::Move => {
                if let Some(state) = self.pointers.get_mut(&event.pointer_id) {
                    let total = Point::new(
                        event.position.x - state.start.x,
                        event.position.y - state.start.y,
                    );
                    let moved = (total.x * total.x + total.y * total.y).sqrt();
                    if self.pinch.is_none()
                        && !state.panning
                        && !state.long_pressed
                        && moved > self.config.pan_threshold
                    {
                        state.panning = true;
                        out.push(Gesture::PanStart {
                            pointer_id: event.pointer_id,
                            target: state.target,
                            position: event.position,
                        });
                    }
                    if self.pinch.is_none() && state.panning {
                        let delta = Point::new(
                            event.position.x - state.last.x,
                            event.position.y - state.last.y,
                        );
                        out.push(Gesture::Pan {
                            pointer_id: event.pointer_id,
                            target: state.target,
                            delta,
                            total,
                        });
                    }
                    state.last = event.position;
                }
                self.update_pinch(&mut out);
            }
            PointerPhase::Up => {
                let state = self.pointers.remove(&event.pointer_id);
                if let Some(state) = state {
                    let duration = event.timestamp_millis.saturating_sub(state.started_at);
                    let total = Point::new(
                        event.position.x - state.start.x,
                        event.position.y - state.start.y,
                    );
                    let moved = (total.x * total.x + total.y * total.y).sqrt();
                    if state.panning {
                        out.push(Gesture::PanEnd {
                            pointer_id: event.pointer_id,
                            target: state.target,
                            position: event.position,
                            total,
                        });
                    } else if !state.long_pressed
                        && self.pinch.is_none()
                        && duration <= self.config.tap_max_duration_ms
                        && moved <= self.config.tap_max_distance
                    {
                        out.push(Gesture::Tap {
                            pointer_id: event.pointer_id,
                            target: state.target,
                            position: event.position,
                        });
                    }
                }
                self.update_pinch(&mut out);
            }
            PointerPhase::Cancel => {
                if self.pointers.remove(&event.pointer_id).is_some() {
                    out.push(Gesture::Cancelled {
                        pointer_id: event.pointer_id,
                    });
                }
                self.update_pinch(&mut out);
            }
            PointerPhase::Hover => {}
        }
        out
    }

    /// Advances time for long-press; call with a monotonic millisecond clock.
    pub fn tick(&mut self, now_ms: u64) -> Vec<Gesture> {
        let mut out = Vec::new();
        if self.pinch.is_some() {
            return out;
        }
        for (id, state) in &mut self.pointers {
            if !state.panning
                && !state.long_pressed
                && now_ms.saturating_sub(state.started_at) >= self.config.long_press_ms
            {
                state.long_pressed = true;
                out.push(Gesture::LongPress {
                    pointer_id: *id,
                    target: state.target,
                    position: state.last,
                });
            }
        }
        out
    }

    pub fn active_pointers(&self) -> usize {
        self.pointers.len()
    }

    pub fn is_pinching(&self) -> bool {
        self.pinch.is_some()
    }

    fn update_pinch(&mut self, out: &mut Vec<Gesture>) {
        let mut ids: Vec<PointerId> = self.pointers.keys().copied().take(2).collect();
        if ids.len() < 2 {
            if let Some(pinch) = self.pinch.take() {
                out.push(Gesture::PinchEnd {
                    target: pinch.target,
                    center: pinch.last_center,
                });
            }
            return;
        }
        ids.sort();
        let first = ids[0];
        let second = ids[1];
        let p1 = &self.pointers[&first];
        let p2 = &self.pointers[&second];
        let center = Point::new((p1.last.x + p2.last.x) / 2.0, (p1.last.y + p2.last.y) / 2.0);
        let distance = ((p1.last.x - p2.last.x).powi(2) + (p1.last.y - p2.last.y).powi(2)).sqrt();

        let replace = match &self.pinch {
            Some(pinch) => pinch.first != first || pinch.second != second,
            None => true,
        };
        if replace {
            if let Some(pinch) = self.pinch.take() {
                out.push(Gesture::PinchEnd {
                    target: pinch.target,
                    center: pinch.last_center,
                });
            }
            let initial = distance.max(1.0);
            self.pinch = Some(PinchState {
                first,
                second,
                initial_distance: initial,
                last_distance: distance,
                last_center: center,
                target: p1.target.or(p2.target),
            });
            out.push(Gesture::PinchStart {
                target: p1.target.or(p2.target),
                center,
                distance,
            });
            return;
        }

        let pinch = self.pinch.as_mut().expect("pinch exists");
        let delta = distance - pinch.last_distance;
        if delta.abs() >= self.config.pinch_threshold
            || (center.x, center.y) != (pinch.last_center.x, pinch.last_center.y)
        {
            pinch.last_distance = distance;
            pinch.last_center = center;
            out.push(Gesture::Pinch {
                target: pinch.target,
                center,
                scale: distance / pinch.initial_distance,
                delta,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::input::{PointerEvent, PointerKind, PointerPhase};
    use crate::pointer::PointerRoute;

    fn route(id: u64, phase: PointerPhase, x: f32, y: f32, t: u64) -> PointerRoute {
        PointerRoute {
            event: PointerEvent {
                pointer_id: PointerId(id),
                kind: PointerKind::Touch,
                phase,
                position: Point::new(x, y),
                timestamp_millis: t,
                ..Default::default()
            },
            target: Some(NodeId(1)),
            path: vec![NodeId(1)],
            captured: true,
        }
    }

    #[test]
    fn recognizes_tap() {
        let mut recognizer = GestureRecognizer::default();
        assert!(recognizer
            .process(&route(1, PointerPhase::Down, 10.0, 10.0, 0))
            .is_empty());
        let gestures = recognizer.process(&route(1, PointerPhase::Up, 11.0, 10.0, 100));
        assert_eq!(
            gestures,
            vec![Gesture::Tap {
                pointer_id: PointerId(1),
                target: Some(NodeId(1)),
                position: Point::new(11.0, 10.0)
            }]
        );
    }

    #[test]
    fn recognizes_long_press_and_suppresses_tap() {
        let mut recognizer = GestureRecognizer::default();
        recognizer.process(&route(1, PointerPhase::Down, 10.0, 10.0, 0));
        assert_eq!(
            recognizer.tick(600),
            vec![Gesture::LongPress {
                pointer_id: PointerId(1),
                target: Some(NodeId(1)),
                position: Point::new(10.0, 10.0)
            }]
        );
        assert!(recognizer
            .process(&route(1, PointerPhase::Up, 10.0, 10.0, 700))
            .is_empty());
    }

    #[test]
    fn recognizes_pan() {
        let mut recognizer = GestureRecognizer::default();
        recognizer.process(&route(1, PointerPhase::Down, 10.0, 10.0, 0));
        let start = recognizer.process(&route(1, PointerPhase::Move, 30.0, 10.0, 50));
        assert!(matches!(start[0], Gesture::PanStart { .. }));
        let moved = recognizer.process(&route(1, PointerPhase::Move, 40.0, 10.0, 60));
        assert!(matches!(moved[0], Gesture::Pan { .. }));
        let end = recognizer.process(&route(1, PointerPhase::Up, 40.0, 10.0, 70));
        assert!(matches!(end[0], Gesture::PanEnd { .. }));
    }

    #[test]
    fn recognizes_pinch() {
        let mut recognizer = GestureRecognizer::default();
        recognizer.process(&route(1, PointerPhase::Down, 0.0, 0.0, 0));
        let start = recognizer.process(&route(2, PointerPhase::Down, 10.0, 0.0, 0));
        assert!(
            matches!(start[0], Gesture::PinchStart { distance, .. } if (distance - 10.0).abs() < 0.01)
        );
        let moved = recognizer.process(&route(2, PointerPhase::Move, 20.0, 0.0, 10));
        assert!(matches!(moved[0], Gesture::Pinch { scale, .. } if (scale - 2.0).abs() < 0.01));
        let end = recognizer.process(&route(2, PointerPhase::Up, 20.0, 0.0, 20));
        assert!(matches!(end[0], Gesture::PinchEnd { .. }));
    }

    #[test]
    fn cancel_emits_cancelled() {
        let mut recognizer = GestureRecognizer::default();
        recognizer.process(&route(1, PointerPhase::Down, 1.0, 1.0, 0));
        assert_eq!(
            recognizer.process(&route(1, PointerPhase::Cancel, 1.0, 1.0, 1)),
            vec![Gesture::Cancelled {
                pointer_id: PointerId(1)
            }]
        );
    }
}
