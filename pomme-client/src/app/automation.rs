use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::input::{Action, InputState};
use crate::app::phases::AppPhase;

const SCRIPT_SCHEMA: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Script {
    schema: u32,
    steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Step {
    WaitPhase {
        phase: PhaseName,
        timeout_frames: u32,
    },
    Press {
        action: Action,
    },
    Release {
        action: Action,
    },
    WaitFrames {
        frames: u32,
    },
    Assert {
        condition: Condition,
        equals: bool,
    },
    CapturePosition,
    AssertPositionDelta {
        min: [f64; 3],
        max: [f64; 3],
    },
    CaptureTargetBlock {
        id: String,
    },
    AssertCapturedBlock {
        id: String,
    },
    AssertHeldItem {
        id: String,
    },
    Exit,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum PhaseName {
    Setup,
    Menu,
    Connecting,
    InGame,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Condition {
    InventoryOpen,
    Paused,
    ChatOpen,
}

#[derive(Debug, Serialize)]
struct Trace {
    schema: u32,
    script: PathBuf,
    completed: bool,
    frames: u64,
    events: Vec<TraceEvent>,
    failure: Option<String>,
}

#[derive(Debug, Serialize)]
struct TraceEvent {
    frame: u64,
    step: usize,
    event: String,
}

#[derive(Clone, Copy, Debug)]
struct ObservedState {
    phase: PhaseName,
    inventory_open: bool,
    paused: bool,
    chat_open: bool,
    position: Option<[f64; 3]>,
}

impl ObservedState {
    fn from_phase(phase: &AppPhase) -> Self {
        match phase {
            AppPhase::Setup { .. } => Self::empty(PhaseName::Setup),
            AppPhase::InMenu { .. } => Self::empty(PhaseName::Menu),
            AppPhase::Connecting { .. } => Self::empty(PhaseName::Connecting),
            AppPhase::InGame { game, .. } => Self {
                phase: PhaseName::InGame,
                inventory_open: game.inventory_open || game.creative_inventory_open,
                paused: game.paused,
                chat_open: game.chat.is_open(),
                position: Some([
                    game.player.position.x,
                    game.player.position.y,
                    game.player.position.z,
                ]),
            },
        }
    }

    const fn empty(phase: PhaseName) -> Self {
        Self {
            phase,
            inventory_open: false,
            paused: false,
            chat_open: false,
            position: None,
        }
    }

    const fn condition(self, condition: Condition) -> bool {
        match condition {
            Condition::InventoryOpen => self.inventory_open,
            Condition::Paused => self.paused,
            Condition::ChatOpen => self.chat_open,
        }
    }
}

pub struct Automation {
    script_path: PathBuf,
    trace_path: PathBuf,
    script: Script,
    step: usize,
    frame: u64,
    wait_frames_remaining: Option<u32>,
    phase_wait_frames: u32,
    position_anchor: Option<[f64; 3]>,
    block_target: Option<[i32; 3]>,
    events: Vec<TraceEvent>,
    exit_requested: bool,
    failure: Option<String>,
}

impl Automation {
    pub fn load(
        script_path: impl AsRef<Path>,
        trace_path: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let script_path = script_path.as_ref().to_path_buf();
        let bytes = fs::read(&script_path)
            .map_err(|error| format!("failed to read {}: {error}", script_path.display()))?;
        let script: Script = serde_json::from_slice(&bytes)
            .map_err(|error| format!("failed to parse {}: {error}", script_path.display()))?;
        if script.schema != SCRIPT_SCHEMA {
            return Err(format!(
                "{} uses schema {}, expected {SCRIPT_SCHEMA}",
                script_path.display(),
                script.schema
            ));
        }
        if script.steps.is_empty() {
            return Err(format!("{} contains no steps", script_path.display()));
        }

        Ok(Self {
            script_path,
            trace_path: trace_path.as_ref().to_path_buf(),
            script,
            step: 0,
            frame: 0,
            wait_frames_remaining: None,
            phase_wait_frames: 0,
            position_anchor: None,
            block_target: None,
            events: Vec::new(),
            exit_requested: false,
            failure: None,
        })
    }

    /// Dispatch at most one input mutation per frame. Assertions run at the
    /// beginning of the next frame, after gameplay consumed the prior action.
    pub fn before_input(&mut self, phase: &AppPhase, input: &mut InputState) {
        if self.exit_requested || self.failure.is_some() {
            return;
        }
        self.frame += 1;
        let observed = ObservedState::from_phase(phase);

        loop {
            let Some(step) = self.script.steps.get(self.step) else {
                self.fail("script ended without an explicit exit step".to_owned());
                return;
            };

            match step {
                Step::WaitPhase {
                    phase,
                    timeout_frames,
                } => {
                    if observed.phase == *phase {
                        self.record(format!("phase={phase:?}"));
                        self.step += 1;
                        self.phase_wait_frames = 0;
                        continue;
                    }
                    self.phase_wait_frames += 1;
                    if self.phase_wait_frames > *timeout_frames {
                        self.fail(format!(
                            "step {} timed out waiting for phase {phase:?}; observed {:?}",
                            self.step, observed.phase
                        ));
                    }
                    return;
                }
                Step::Press { action } => {
                    let action = *action;
                    input.inject_action_press(action);
                    self.record(format!("press={action:?}"));
                    self.step += 1;
                    return;
                }
                Step::Release { action } => {
                    let action = *action;
                    input.inject_action_release(action);
                    self.record(format!("release={action:?}"));
                    self.step += 1;
                    return;
                }
                Step::WaitFrames { frames } => {
                    let remaining = self.wait_frames_remaining.get_or_insert(*frames);
                    if *remaining == 0 {
                        self.wait_frames_remaining = None;
                        self.record(format!("waited_frames={frames}"));
                        self.step += 1;
                        continue;
                    }
                    *remaining -= 1;
                    return;
                }
                Step::Assert { condition, equals } => {
                    let actual = observed.condition(*condition);
                    if actual != *equals {
                        self.fail(format!(
                            "step {} assertion failed: {condition:?} was {actual}, expected {equals}",
                            self.step
                        ));
                        return;
                    }
                    self.record(format!("assert={condition:?}:{actual}"));
                    self.step += 1;
                }
                Step::CapturePosition => {
                    let Some(position) = observed.position else {
                        self.fail(format!(
                            "step {} cannot capture position outside the in-game phase",
                            self.step
                        ));
                        return;
                    };
                    self.position_anchor = Some(position);
                    self.record(format!("position_anchor={position:?}"));
                    self.step += 1;
                }
                Step::AssertPositionDelta { min, max } => {
                    let (Some(anchor), Some(position)) = (self.position_anchor, observed.position)
                    else {
                        self.fail(format!(
                            "step {} requires an in-game position and a captured anchor",
                            self.step
                        ));
                        return;
                    };
                    let delta = std::array::from_fn(|axis| position[axis] - anchor[axis]);
                    if !position_delta_within(delta, *min, *max) {
                        self.fail(format!(
                            "step {} position delta {delta:?} was outside {min:?}..={max:?}",
                            self.step
                        ));
                        return;
                    }
                    self.record(format!("assert_position_delta={delta:?}"));
                    self.step += 1;
                }
                Step::CaptureTargetBlock { id } => {
                    let AppPhase::InGame { game, .. } = phase else {
                        self.fail(format!(
                            "step {} cannot capture a block outside the in-game phase",
                            self.step
                        ));
                        return;
                    };
                    let Some(pos) = game.interaction.block_target_pos() else {
                        self.fail(format!(
                            "step {} has no block under the crosshair",
                            self.step
                        ));
                        return;
                    };
                    let state = game.chunk_store.get_block_state(pos[0], pos[1], pos[2]);
                    let actual = crate::world::block::block_id(state);
                    if actual != id {
                        self.fail(format!(
                            "step {} targeted block at {pos:?} was {actual:?}, expected {id:?}",
                            self.step
                        ));
                        return;
                    }
                    self.block_target = Some(pos);
                    self.record(format!("capture_target_block={pos:?}:{actual}"));
                    self.step += 1;
                }
                Step::AssertCapturedBlock { id } => {
                    let AppPhase::InGame { game, .. } = phase else {
                        self.fail(format!(
                            "step {} cannot inspect a block outside the in-game phase",
                            self.step
                        ));
                        return;
                    };
                    let Some(pos) = self.block_target else {
                        self.fail(format!(
                            "step {} requires a captured block target",
                            self.step
                        ));
                        return;
                    };
                    let state = game.chunk_store.get_block_state(pos[0], pos[1], pos[2]);
                    let actual = crate::world::block::block_id(state);
                    if actual != id {
                        self.fail(format!(
                            "step {} captured block at {pos:?} was {actual:?}, expected {id:?}",
                            self.step
                        ));
                        return;
                    }
                    self.record(format!("assert_captured_block={pos:?}:{actual}"));
                    self.step += 1;
                }
                Step::AssertHeldItem { id } => {
                    let AppPhase::InGame { game, .. } = phase else {
                        self.fail(format!(
                            "step {} cannot inspect an item outside the in-game phase",
                            self.step
                        ));
                        return;
                    };
                    let actual = game
                        .player
                        .inventory
                        .held_stack(input.selected_slot())
                        .map(|stack| crate::player::inventory::item_resource_name(stack.kind))
                        .unwrap_or_else(|| "air".to_owned());
                    if actual != *id {
                        self.fail(format!(
                            "step {} held item was {actual:?}, expected {id:?}",
                            self.step
                        ));
                        return;
                    }
                    self.record(format!("assert_held_item={actual}"));
                    self.step += 1;
                }
                Step::Exit => {
                    self.record("exit".to_owned());
                    self.step += 1;
                    self.exit_requested = true;
                    return;
                }
            }
        }
    }

    pub const fn should_exit(&self) -> bool {
        self.exit_requested || self.failure.is_some()
    }

    pub fn finish(&mut self) -> Result<(), String> {
        if !self.exit_requested && self.failure.is_none() {
            self.fail(format!(
                "application exited before automation completed step {}",
                self.step
            ));
        }

        let trace = Trace {
            schema: SCRIPT_SCHEMA,
            script: self.script_path.clone(),
            completed: self.failure.is_none() && self.exit_requested,
            frames: self.frame,
            events: std::mem::take(&mut self.events),
            failure: self.failure.clone(),
        };
        if let Some(parent) = self.trace_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create automation trace directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let bytes = serde_json::to_vec_pretty(&trace)
            .map_err(|error| format!("failed to encode automation trace: {error}"))?;
        fs::write(&self.trace_path, bytes).map_err(|error| {
            format!(
                "failed to write automation trace {}: {error}",
                self.trace_path.display()
            )
        })?;

        match self.failure.take() {
            Some(failure) => Err(failure),
            None => {
                tracing::info!(
                    "Automation completed in {} frames; trace: {}",
                    self.frame,
                    self.trace_path.display()
                );
                Ok(())
            }
        }
    }

    fn record(&mut self, event: String) {
        self.events.push(TraceEvent {
            frame: self.frame,
            step: self.step,
            event,
        });
    }

    fn fail(&mut self, failure: String) {
        tracing::error!("Automation failed: {failure}");
        self.failure = Some(failure);
    }
}

fn position_delta_within(delta: [f64; 3], min: [f64; 3], max: [f64; 3]) -> bool {
    (0..3).all(|axis| {
        min[axis].is_finite()
            && max[axis].is_finite()
            && min[axis] <= max[axis]
            && delta[axis] >= min[axis]
            && delta[axis] <= max[axis]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_script(contents: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("pomme-automation-test-{}", fastrand::u64(..)));
        fs::create_dir_all(&dir).unwrap();
        let script = dir.join("script.json");
        let trace = dir.join("trace.json");
        fs::write(&script, contents).unwrap();
        (script, trace)
    }

    #[test]
    fn rejects_unknown_schema() {
        let (script, trace) = write_script(r#"{"schema":2,"steps":[{"type":"exit"}]}"#);
        let error = Automation::load(script, trace).err().unwrap();
        assert!(error.contains("uses schema 2"));
    }

    #[test]
    fn writes_failure_trace_for_unmet_assertion() {
        let (script, trace) = write_script(
            r#"{"schema":1,"steps":[{"type":"assert","condition":"inventory_open","equals":true},{"type":"exit"}]}"#,
        );
        let mut automation = Automation::load(&script, &trace).unwrap();
        let mut input = InputState::released();
        let phase = AppPhase::Setup {
            quick_access_multiplayer: None,
            quick_access_singleplayer: false,
            pending_skin_uuid: None,
        };
        automation.before_input(&phase, &mut input);
        assert!(automation.should_exit());
        assert!(automation.finish().is_err());
        let value: serde_json::Value = serde_json::from_slice(&fs::read(trace).unwrap()).unwrap();
        assert_eq!(value["completed"], false);
        assert!(
            value["failure"]
                .as_str()
                .unwrap()
                .contains("assertion failed")
        );
    }

    #[test]
    fn position_delta_requires_each_axis_inside_the_inclusive_bounds() {
        assert!(position_delta_within(
            [0.25, -0.5, 2.0],
            [-1.0, -1.0, 1.0],
            [1.0, 1.0, 3.0]
        ));
        assert!(!position_delta_within(
            [0.25, -0.5, 4.0],
            [-1.0, -1.0, 1.0],
            [1.0, 1.0, 3.0]
        ));
        assert!(!position_delta_within(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0]
        ));
    }
}
