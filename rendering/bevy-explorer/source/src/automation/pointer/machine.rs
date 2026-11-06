//! The pure side of `/hover_entity` and `/click_entity`: the state machine a job steps through
//! frame by frame, and the refusals and diagnoses it words.

use common::inputs::InputIdentifier;
use scene_runner::update_scene::pointer_results::CURSOR_RAY_MAX_DISTANCE;

pub(super) const NO_COLLIDER_HIT: &str = "no collider hit";
const BEYOND_RAY: &str = "beyond 100 m";
pub(super) const ENTITY_NOT_LIVE: &str = "entity not live";
pub(super) const HUD_HOLDS_INPUT: &str = "input held by the HUD";

/// What a job does once the aim is on the entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickKind {
    /// hover for this many frames
    Hover(u32),
    /// press, then release on a later scene tick
    Click,
    /// press and keep the press held (the aim stays, `Hold::Open`)
    Down,
    /// release a held press
    Up,
}

/// The pointer target a frame saw, as proto entity ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSeen {
    /// the hit entity
    pub entity: u32,
    /// the entity owning the hit collider
    pub container: u32,
    /// the hit container's ancestors that are scene entities, below the scene root
    pub ancestors_hit: Vec<u32>,
}

impl TargetSeen {
    fn matches(&self, id: u32) -> bool {
        self.entity == id || self.container == id || self.ancestors_hit.contains(&id)
    }
}

/// One frame of the pipeline as the job sees it. The default has `entity_live: true` (a manual
/// `Default`), so a test names only what it changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Observed {
    pub target: Option<TargetSeen>,
    pub scene_tick: Option<u32>,
    /// a `PetDown` for the entity is in the log since the job started
    pub down_logged: bool,
    /// a `PetUp` for the entity is in the log since the job started
    pub up_logged: bool,
    /// the scene still maps the entity to the aimed bevy entity
    pub entity_live: bool,
}

impl Default for Observed {
    fn default() -> Self {
        Self {
            target: None,
            scene_tick: None,
            down_logged: false,
            up_logged: false,
            entity_live: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// press the binding
    Press,
    /// release the binding
    Release,
    /// the job is over: `Err` carries the diagnosis
    Finish(Result<(), String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Target,
    Hovering,
    Tick,
    Release,
    Done,
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Stage::Target | Stage::Hovering => "target",
            Stage::Tick => "tick",
            Stage::Release | Stage::Done => "release",
        }
    }
}

/// The steps of a hover or click, decided from one [`Observed`] frame at a time. Nothing is
/// evaluated before `started + 2` (the aim needs a frame to reach the pointer target); past
/// `started + timeout` the job times out naming the stage it waited in. A failure does not
/// release a press itself: [`ClickMachine::holds_press`] tells the caller to.
pub struct ClickMachine {
    kind: ClickKind,
    entity: u32,
    timeout: u32,
    started: u32,
    stage: Stage,
    pressed: bool,
    /// the scene tick the down was first seen at
    down_tick: Option<u32>,
}

impl ClickMachine {
    /// `entity` is the proto id asked for; an `Up` starts holding the press it releases.
    pub fn new(kind: ClickKind, entity: u32, timeout: u32, started: u32) -> Self {
        Self {
            kind,
            entity,
            timeout,
            started,
            stage: Stage::Target,
            pressed: kind == ClickKind::Up,
            down_tick: None,
        }
    }

    pub fn kind(&self) -> ClickKind {
        self.kind
    }

    /// Whether a press is down that the machine has not released.
    pub fn holds_press(&self) -> bool {
        self.pressed
    }

    pub fn step(&mut self, frame: u32, observed: Observed) -> Vec<Effect> {
        if self.stage == Stage::Done || frame < self.started.saturating_add(2) {
            return Vec::new();
        }
        if !observed.entity_live {
            return self.finish(Err(ENTITY_NOT_LIVE.to_owned()));
        }
        if frame >= self.started.saturating_add(self.timeout) {
            let stage = self.stage.name();
            return self.finish(Err(format!("timed out waiting for {stage}")));
        }
        match self.stage {
            Stage::Target if self.kind == ClickKind::Up => {
                self.pressed = false;
                self.stage = Stage::Release;
                vec![Effect::Release]
            }
            Stage::Target => match observed.target {
                None => self.finish(Err(NO_COLLIDER_HIT.to_owned())),
                Some(target) if !target.matches(self.entity) => {
                    self.finish(Err(format!("blocked by entity {}", target.container)))
                }
                Some(_) if matches!(self.kind, ClickKind::Hover(_)) => {
                    self.stage = Stage::Hovering;
                    self.hover(frame)
                }
                Some(_) => {
                    self.pressed = true;
                    self.stage = Stage::Tick;
                    vec![Effect::Press]
                }
            },
            Stage::Hovering => self.hover(frame),
            Stage::Tick => {
                if !observed.down_logged {
                    return Vec::new();
                }
                if self.kind == ClickKind::Down {
                    return self.finish(Ok(()));
                }
                let Some(tick) = observed.scene_tick else {
                    return Vec::new();
                };
                // the up goes on a later tick than the down, so the scene sees both
                if tick > *self.down_tick.get_or_insert(tick) {
                    self.pressed = false;
                    self.stage = Stage::Release;
                    vec![Effect::Release]
                } else {
                    Vec::new()
                }
            }
            Stage::Release if observed.up_logged => self.finish(Ok(())),
            Stage::Release | Stage::Done => Vec::new(),
        }
    }

    fn hover(&mut self, frame: u32) -> Vec<Effect> {
        let ClickKind::Hover(frames) = self.kind else {
            return Vec::new();
        };
        if frame >= self.started.saturating_add(2).saturating_add(frames) {
            self.finish(Ok(()))
        } else {
            Vec::new()
        }
    }

    fn finish(&mut self, result: Result<(), String>) -> Vec<Effect> {
        self.stage = Stage::Done;
        vec![Effect::Finish(result)]
    }
}

/// The refusal while a press is held on `held` (a proto id), if one is.
pub fn refuse_while_held(held: Option<u32>, _command: &str) -> Option<String> {
    held.map(|id| {
        format!("a press is held on entity {id}; send /click_entity {id} --event up or /hover_entity --clear")
    })
}

/// The refusal for a binding the input script cannot press: gamepad buttons and axes.
pub fn binding_error(button: &str, binding: &InputIdentifier) -> Option<String> {
    match binding {
        InputIdentifier::Key(_) | InputIdentifier::Mouse(_) => None,
        InputIdentifier::Gamepad(_) | InputIdentifier::Analog(..) => Some(format!(
            "{button} is bound to a gamepad button; bind a key or mouse button"
        )),
    }
}

/// `no collider hit` becomes `beyond 100 m` when the anchor point is past the cursor ray's reach.
pub(super) fn refine_diagnosis(diagnosis: String, anchor_distance: Option<f32>) -> String {
    if diagnosis == NO_COLLIDER_HIT
        && anchor_distance.is_some_and(|distance| distance > CURSOR_RAY_MAX_DISTANCE)
    {
        BEYOND_RAY.to_owned()
    } else {
        diagnosis
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{GamepadButton, KeyCode, MouseButton};
    fn seen(id: u32) -> Option<TargetSeen> {
        Some(TargetSeen {
            entity: id,
            container: id,
            ancestors_hit: vec![id],
        })
    }

    #[test]
    fn click_presses_after_the_target_matches_and_releases_on_a_later_tick() {
        let mut m = ClickMachine::new(ClickKind::Click, 7, 600, 0);
        assert!(m
            .step(
                2,
                Observed {
                    target: seen(7),
                    scene_tick: Some(10),
                    ..Default::default()
                }
            )
            .contains(&Effect::Press));
        assert!(m
            .step(
                3,
                Observed {
                    target: seen(7),
                    scene_tick: Some(10),
                    down_logged: true,
                    ..Default::default()
                }
            )
            .is_empty());
        assert!(m
            .step(
                4,
                Observed {
                    target: seen(7),
                    scene_tick: Some(11),
                    down_logged: true,
                    ..Default::default()
                }
            )
            .contains(&Effect::Release));
        assert_eq!(
            m.step(
                6,
                Observed {
                    target: seen(7),
                    scene_tick: Some(11),
                    down_logged: true,
                    up_logged: true,
                    ..Default::default()
                }
            ),
            vec![Effect::Finish(Ok(()))]
        );
    }

    #[test]
    fn click_fails_when_another_entity_is_hit() {
        let mut m = ClickMachine::new(ClickKind::Click, 7, 600, 0);
        let fx = m.step(
            2,
            Observed {
                target: seen(9),
                scene_tick: Some(1),
                entity_live: true,
                ..Default::default()
            },
        );
        assert_eq!(fx, vec![Effect::Finish(Err("blocked by entity 9".into()))]);
    }

    #[test]
    fn job_fails_when_the_entity_is_gone() {
        let mut m = ClickMachine::new(ClickKind::Hover(30), 7, 600, 0);
        assert_eq!(
            m.step(
                2,
                Observed {
                    entity_live: false,
                    ..Default::default()
                }
            ),
            vec![Effect::Finish(Err("entity not live".into()))]
        );
    }

    #[test]
    fn click_times_out_waiting_for_the_tick() {
        let mut m = ClickMachine::new(ClickKind::Click, 7, 10, 0);
        m.step(
            2,
            Observed {
                target: seen(7),
                scene_tick: Some(10),
                entity_live: true,
                ..Default::default()
            },
        );
        assert_eq!(
            m.step(
                11,
                Observed {
                    target: seen(7),
                    scene_tick: Some(10),
                    down_logged: true,
                    entity_live: true,
                    ..Default::default()
                }
            ),
            vec![Effect::Finish(Err("timed out waiting for tick".into()))]
        );
    }

    #[test]
    fn hover_finishes_after_its_frames() {
        // Hover(n) finishes at started + 2 + n
        let mut m = ClickMachine::new(ClickKind::Hover(1), 7, 600, 0);
        assert!(m
            .step(
                2,
                Observed {
                    target: seen(7),
                    scene_tick: Some(1),
                    entity_live: true,
                    ..Default::default()
                }
            )
            .is_empty());
        assert_eq!(
            m.step(
                3,
                Observed {
                    target: seen(7),
                    scene_tick: Some(1),
                    entity_live: true,
                    ..Default::default()
                }
            ),
            vec![Effect::Finish(Ok(()))]
        );
    }

    #[test]
    fn job_refuses_to_replace_an_open_press() {
        assert_eq!(refuse_while_held(Some(7), "/hover_entity"), Some("a press is held on entity 7; send /click_entity 7 --event up or /hover_entity --clear".to_owned()));
        assert_eq!(refuse_while_held(None, "/hover_entity"), None);
    }

    #[test]
    fn job_refuses_a_gamepad_binding() {
        assert_eq!(
            binding_error("primary", &InputIdentifier::Gamepad(GamepadButton::South)),
            Some("primary is bound to a gamepad button; bind a key or mouse button".to_owned())
        );
        assert_eq!(
            binding_error("pointer", &InputIdentifier::Mouse(MouseButton::Left)),
            None
        );
    }

    #[test]
    fn nothing_is_evaluated_before_the_aim_reaches_the_pointer() {
        let mut m = ClickMachine::new(ClickKind::Click, 7, 600, 10);
        assert!(m
            .step(
                11,
                Observed {
                    entity_live: false,
                    ..Default::default()
                }
            )
            .is_empty());
        assert!(m
            .step(
                12,
                Observed {
                    target: seen(7),
                    ..Default::default()
                }
            )
            .contains(&Effect::Press));
    }

    #[test]
    fn an_ancestor_of_the_hit_matches_and_a_miss_is_no_collider_hit() {
        let child = Some(TargetSeen {
            entity: 9,
            container: 9,
            ancestors_hit: vec![7],
        });
        let mut m = ClickMachine::new(ClickKind::Hover(0), 7, 600, 0);
        assert_eq!(
            m.step(
                2,
                Observed {
                    target: child,
                    ..Default::default()
                }
            ),
            vec![Effect::Finish(Ok(()))]
        );
        let mut m = ClickMachine::new(ClickKind::Click, 7, 600, 0);
        assert_eq!(
            m.step(2, Observed::default()),
            vec![Effect::Finish(Err(NO_COLLIDER_HIT.into()))]
        );
        assert!(!m.holds_press());
    }

    #[test]
    fn down_finishes_holding_the_press_and_up_releases_it() {
        let mut down = ClickMachine::new(ClickKind::Down, 7, 600, 0);
        assert_eq!(
            down.step(
                2,
                Observed {
                    target: seen(7),
                    scene_tick: Some(1),
                    ..Default::default()
                }
            ),
            vec![Effect::Press]
        );
        assert!(down
            .step(
                3,
                Observed {
                    target: seen(7),
                    scene_tick: Some(1),
                    ..Default::default()
                }
            )
            .is_empty());
        assert_eq!(
            down.step(
                4,
                Observed {
                    target: seen(7),
                    scene_tick: Some(1),
                    down_logged: true,
                    ..Default::default()
                }
            ),
            vec![Effect::Finish(Ok(()))]
        );
        assert!(down.holds_press());

        let mut up = ClickMachine::new(ClickKind::Up, 7, 600, 10);
        assert!(up.holds_press());
        assert_eq!(up.step(12, Observed::default()), vec![Effect::Release]);
        assert!(!up.holds_press());
        assert!(up.step(13, Observed::default()).is_empty());
        assert_eq!(
            up.step(
                14,
                Observed {
                    up_logged: true,
                    ..Default::default()
                }
            ),
            vec![Effect::Finish(Ok(()))]
        );
    }

    #[test]
    fn timeouts_name_the_stage_and_a_failed_press_is_left_to_the_caller() {
        let mut target = ClickMachine::new(ClickKind::Click, 7, 2, 0);
        assert_eq!(
            target.step(
                2,
                Observed {
                    target: seen(7),
                    ..Default::default()
                }
            ),
            vec![Effect::Finish(Err("timed out waiting for target".into()))]
        );
        let mut release = ClickMachine::new(ClickKind::Up, 7, 5, 0);
        release.step(2, Observed::default());
        assert_eq!(
            release.step(5, Observed::default()),
            vec![Effect::Finish(Err("timed out waiting for release".into()))]
        );
        let mut gone = ClickMachine::new(ClickKind::Up, 7, 600, 0);
        assert_eq!(
            gone.step(
                2,
                Observed {
                    entity_live: false,
                    ..Default::default()
                }
            ),
            vec![Effect::Finish(Err(ENTITY_NOT_LIVE.into()))]
        );
        assert!(gone.holds_press());
        // a finished machine stays finished
        assert!(gone.step(3, Observed::default()).is_empty());
    }

    #[test]
    fn a_miss_past_the_rays_reach_reads_beyond_100_m() {
        assert_eq!(
            refine_diagnosis(NO_COLLIDER_HIT.into(), Some(150.0)),
            "beyond 100 m"
        );
        assert_eq!(
            refine_diagnosis(NO_COLLIDER_HIT.into(), Some(50.0)),
            NO_COLLIDER_HIT
        );
        assert_eq!(
            refine_diagnosis(NO_COLLIDER_HIT.into(), None),
            NO_COLLIDER_HIT
        );
        assert_eq!(
            refine_diagnosis("blocked by entity 9".into(), Some(150.0)),
            "blocked by entity 9"
        );
        assert_eq!(
            binding_error("primary", &InputIdentifier::Key(KeyCode::KeyE)),
            None
        );
    }
}
