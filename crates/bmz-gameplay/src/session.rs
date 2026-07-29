use std::collections::HashMap;
use std::sync::Arc;

use bmz_audio::clock::AudioClock;
use bmz_audio::queue::{AudioScheduler, RestartPolicy, ScheduledSound};
use bmz_chart::model::{LongNoteMode, NoteEvent, NoteKind, PlayableChart};
use bmz_chart::timing::TimingMap;
use bmz_core::ids::{NoteId, SoundId};
use bmz_core::input::{InputEvent, InputKind, InputSource, ScratchDirection};
use bmz_core::judge::{Judge, TimingSide};
use bmz_core::lane::{KeyMode, LANE_COUNT, Lane};
use bmz_core::time::TimeUs;

use crate::autoplay::AutoplayController;
use crate::gauge::GaugeState;
use crate::hit_error::HitErrorRing;
use crate::input::backend::monotonic_timestamp_ns;
use crate::input::system::InputSystem;
use crate::input::translator::{InputTimestampAnchor, InputTimingContext};
use crate::judge::engine::JudgeEngine;
use crate::judge::model::{
    JudgeOutcome, JudgeWindow, JudgeWindows, JudgementEvent, KeySoundEvent, MineHitEvent,
};
use crate::judge::window::{
    judge_percent_at_time_for_keymode, judge_windows_for_rule_mode_and_keymode,
};
use crate::replay::{ReplayPlayer, ReplayRecorder};
use crate::rule::RuleMode;
use crate::score::ScoreState;

pub const AUDIO_SCHEDULE_AHEAD_US: i64 = 100_000;
pub const SESSION_END_MARGIN_US: i64 = 5_000_000;
pub const JUDGEMENT_DISPLAY_US: i64 = 800_000;
pub const INPUT_DISPLAY_US: i64 = 160_000;
/// オートプレイ時のキー押下を「離す」までの時間。beatoraja の `auto_minduration` (80ms) と揃える。
/// この時間が経過すると lane_keyon → lane_keyoff へ遷移し、skin の KEYOFF タイマー演出が走る。
pub const AUTO_KEYBEAM_DURATION_US: i64 = 80_000;
const SCRATCH_ANGLE_PERIOD_MS: i64 = 2_160;
/// pre-ready の wall-clock key 時刻と chart 時刻を区別する閾値。
const CHART_KEY_FUTURE_SLACK_US: i64 = 1_000_000;

include!("session/state.rs");
include!("session/judgement.rs");
include!("session/audio.rs");
include!("session/input.rs");
include!("session/hcn.rs");
include!("session/frame.rs");

#[cfg(test)]
#[path = "session/tests.rs"]
mod tests;
