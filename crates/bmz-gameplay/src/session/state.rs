#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Ready,
    Playing,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HispeedMode {
    Normal,
    Floating,
}

#[derive(Debug, Clone, Copy)]
pub struct PlayOffsets {
    pub input_offset_us: i64,
    pub visual_offset_us: i64,
}

pub const INPUT_OFFSET_AUTO_ADJUST_MIN_US: i64 = -500_000;
pub const INPUT_OFFSET_AUTO_ADJUST_MAX_US: i64 = 500_000;
const INPUT_OFFSET_AUTO_ADJUST_STEP_US: i64 = 1_000;
const INPUT_OFFSET_AUTO_ADJUST_BATCH: u32 = 10;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputOffsetAutoAdjustState {
    pub sum_delta_us: i64,
    pub count: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct PlayAudioMix {
    pub master_volume: f32,
    /// 譜面の loudness 解析から導出した正規化ゲイン。
    ///
    /// 正規化のON/OFFでは上書きせず、`effective_normalization_gain`で適用値を決める。
    pub chart_normalization_gain: f32,
    pub normalize_chart_volume: bool,
    pub key_volume: f32,
    pub bgm_volume: f32,
}

impl PlayAudioMix {
    pub fn effective_normalization_gain(self) -> f32 {
        if self.normalize_chart_volume && self.chart_normalization_gain.is_finite() {
            self.chart_normalization_gain.clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlaySkinOffset {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub r: i32,
    pub a: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameTimes {
    pub audio_now: TimeUs,
    pub audio_schedule_until: TimeUs,
}

#[derive(Debug, Clone, Default)]
pub struct BgmScheduler {
    pub next_index: usize,
}

pub struct GameSession {
    pub chart: Arc<PlayableChart>,
    /// 判定窓・スコア・ゲージ規則に使う元譜面側のキーモード。
    /// battle 表示では `chart.metadata.key_mode` が K10/K14 でも K5/K7 を保持する。
    pub primary_key_mode: KeyMode,
    /// LN policy / course override 適用後の譜面で実際にスコア対象となるノート数。
    /// Tap と LongStart に加えて、CN / HCN の LongEnd を数える。
    pub scored_total_notes: u32,
    /// `chart` の BPM 変化と STOP を取り込んだ tick<->time マップ。
    /// スクロール位置を BPM に追従させるために使う。
    pub timing_map: TimingMap,
    pub audio_clock: AudioClock,
    pub input_system: InputSystem,
    pub judge: JudgeEngine,
    /// プロファイル既定の判定窓。`#RANK` / `#EXRANK` 倍率の基準値。
    pub base_judge_window: JudgeWindow,
    pub base_judge_windows: JudgeWindows,
    pub rule_mode: RuleMode,
    pub score: ScoreState,
    /// display-only 2P opponent score. None outside battle sessions.
    pub opponent_score: Option<ScoreState>,
    /// Course-mode display combo carry. ScoreState remains per-chart for
    /// storage; these fields only affect the rendered combo/max combo.
    pub course_combo_carry: u32,
    pub course_combo_carry_active: bool,
    pub course_max_combo: u32,
    pub gauge: GaugeState,
    /// display-only 2P opponent gauge. None outside battle sessions.
    pub opponent_gauge: Option<GaugeState>,
    pub replay_recorder: ReplayRecorder,
    pub replay_player: Option<ReplayPlayer>,
    /// Some の場合、リプレイが担当するレーンだけ true。
    /// 通常リプレイの None は従来通り全レーンをリプレイが占有する。
    pub replay_lane_mask: Option<[bool; LANE_COUNT]>,
    /// 対戦ゴーストなど、描画・判定は行うが主プレイヤーのスコア／ゲージ／音声へ
    /// 反映しないレーン。
    pub display_only_lane_mask: [bool; LANE_COUNT],
    pub autoplay: Option<AutoplayController>,
    pub recent_inputs: Vec<InputEvent>,
    /// 各レーンのキー押下開始時刻。押下中のみ Some。skin の keyon タイマー(100..=107)。
    pub lane_keyon_started_at: [Option<TimeUs>; LANE_COUNT],
    /// 各レーンのキー解放時刻。離した直後のみ Some(次の Press でクリア)。skin の keyoff タイマー(120..=127)。
    pub lane_keyoff_started_at: [Option<TimeUs>; LANE_COUNT],
    /// Scratch lane rotation direction while the scratch input is held.
    pub lane_scratch_direction: [Option<ScratchDirection>; LANE_COUNT],
    /// Extra turntable phase accumulated by scratch input, in beatoraja scratch-angle ms.
    pub lane_scratch_angle_delta_ms: [i64; LANE_COUNT],
    pub scratch_angle_last_render_at: Option<TimeUs>,
    /// オートプレイで Press したレーンの自動 Release 予定時刻。
    /// `audio_now` がこの時刻を超えたら keyon → keyoff へ遷移する。
    pub lane_auto_release_at: [Option<TimeUs>; LANE_COUNT],
    pub recent_judgements: Vec<JudgementEvent>,
    /// Skin runtime がフレーム間で入力・判定履歴を構築するためのイベント列。
    /// `advance_session_frame` の終端で drain し、長時間プレイでもセッション側に
    /// 無制限に蓄積しない。sequence は同一時刻のイベント順も一意にする。
    pub pending_skin_events: Vec<SkinRuntimeEvent>,
    pub next_skin_event_sequence: u64,
    /// Result 統計グラフ用の最終ノート判定詳細。
    /// beatoraja の Result は譜面上の Note.state / playTime を走査してグラフ化するため、
    /// BMZ でも score 集計とは別に note_id 単位の判定差分を保持する。
    pub result_judgements: HashMap<NoteId, ResultJudgementDetail>,
    /// HitErrorVisualizer 用の直近判定タイミング (ms)。beatoraja `recentJudges` 相当。
    pub hit_error_ring: HitErrorRing,
    /// Gauge increase animation start time. Skin timer 42/43 uses elapsed time from here.
    pub gauge_increase_started_at: Option<TimeUs>,
    pub opponent_gauge_increase_started_at: Option<TimeUs>,
    /// Gauge max animation start time. Skin timer 44/45 is active while the current gauge is maxed.
    pub gauge_max_started_at: Option<TimeUs>,
    pub opponent_gauge_max_started_at: Option<TimeUs>,
    /// Full combo animation start time. Set once when all notes have been judged
    /// and the combo still matches the total note count.
    pub full_combo_started_at: Option<TimeUs>,
    pub opponent_full_combo_started_at: Option<TimeUs>,
    pub bgm_scheduler: BgmScheduler,
    pub offsets: PlayOffsets,
    pub input_offset_auto_adjust_enabled: bool,
    pub input_offset_auto_adjust: Option<InputOffsetAutoAdjustState>,
    pub audio_mix: PlayAudioMix,
    pub hispeed: f32,
    pub hispeed_mode: HispeedMode,
    pub target_green_number: u32,
    /// Floating hispeed の曲開始前基準 BPM。曲開始後は現在 BPM で再計算する。
    pub hsfix_base_bpm: f64,
    pub lift: f32,
    pub lane_cover: f32,
    /// レーンカバー(SUDDEN+)の表示有無。
    /// false のときは描画/判定の visible_max 計算で lane_cover が 0 として扱われる。
    pub lane_cover_visible: bool,
    /// beatoraja `OPTION_LANECOVER1_ON` (271): SUDDEN+ 機能が有効か。
    pub lanecover_enabled: bool,
    /// beatoraja `OPTION_LIFT1_ON` (272): LIFT 機能が有効か。
    pub lift_enabled: bool,
    /// beatoraja `OPTION_HIDDEN1_ON` (273): HIDDEN 機能が有効か。
    pub hidden_enabled: bool,
    /// beatoraja `PlayConfig.enableHispeedAutoAdjust`。
    pub hispeed_auto_adjust: bool,
    /// Start/Select 押下中など、レーンカバー数値表示を出す状態。
    pub lane_cover_changing: bool,
    pub hidden_cover: f32,
    pub skin_offsets: Vec<PlaySkinOffset>,
    pub bga_enabled: bool,
    pub poor_bga_duration_us: i64,
    pub bga_stretch: i32,
    /// LN モードでも終端 (tail) キャップを描画するか。既定 OFF (beatoraja 準拠)。
    pub show_ln_tail_cap: bool,
    /// HCN passing 中のレーン状態。beatoraja の TIMER_HCN_ACTIVE / TIMER_HCN_DAMAGE 相当。
    /// HCN の区間内 (始端ノート判定済み) のレーンのみ Some。
    pub lane_hcn_timer: [Option<HcnLaneTimer>; LANE_COUNT],
    /// HCN キー音のミュート状態。終端を BAD 以下で判定済みの passing HCN のみ Some。
    pub lane_hcn_keysound_muted: [Option<bool>; LANE_COUNT],
    /// 判定処理で発火したキー音。EmptyPoor や LN 終端のように、スコア対象
    /// note_id とキー音対象 note_id が一致しないケースがあるため別に持つ。
    pub pending_keysounds: Vec<KeySoundEvent>,
    /// 当該フレームで適用するキー音音量変更。`advance_session_frame` の終端で
    /// `SessionFrame.keysound_volumes` に吸い出される (app 層が audio engine に反映)。
    pub pending_keysound_volumes: Vec<(SoundId, f32)>,
    /// beatoraja `event_index(BUTTON_HSFIX=55)`。
    pub hsfix_index: i32,
    pub input_timestamp_anchor: Option<InputTimestampAnchor>,
    /// 当該フレーム中に発火した Mine ヒット。`advance_session_frame` の終端で
    /// `SessionFrame.mine_hits` に吸い出される（app 層が地雷 SE を鳴らす）。
    pub pending_mine_hits: Vec<MineHitEvent>,
    pub state: PlayState,
    /// HCN ゲージ増減の前回更新時刻。
    pub last_hcn_gauge_at: Option<TimeUs>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultJudgementDetail {
    pub judge: Judge,
    pub side: TimingSide,
    /// BMZ 判定エンジンの差分。正が入力遅れ、負が入力早め。
    pub delta: TimeUs,
    pub time: TimeUs,
}

#[derive(Debug, Clone)]
pub struct FrameOutput<TSnapshot> {
    pub render_snapshot: TSnapshot,
    /// 当該フレームで発生した判定。result graph など、app 層がフレーム単位で
    /// 蓄積する副作用に使う。
    pub judgements: Vec<JudgementEvent>,
    /// 当該フレームで踏んだ Mine ノーツ。app 層が地雷 SE を鳴らすのに使う。
    pub mine_hits: Vec<MineHitEvent>,
    /// 当該フレームで適用するキー音音量変更 (HCN 早離し時のミュート/復帰)。
    pub keysound_volumes: Vec<(SoundId, f32)>,
    /// 当該フレームで発生した、skin runtime 向けの順序付き入力・判定イベント。
    pub skin_events: Vec<SkinRuntimeEvent>,
    pub state: PlayState,
}

#[derive(Debug, Clone)]
pub struct SessionFrame {
    pub times: FrameTimes,
    pub judgements: Vec<JudgementEvent>,
    /// 当該フレームで踏んだ Mine ノーツ。地雷 SE 再生など、UI / audio 側の
    /// 副作用処理に使う。`apply_judge_outcome` の中ですでにゲージは削っている。
    pub mine_hits: Vec<MineHitEvent>,
    /// 当該フレームで適用するキー音音量変更 (HCN 早離し時のミュート/復帰)。
    /// app 層が audio engine の `set_volume_for_sound` に反映する。
    pub keysound_volumes: Vec<(SoundId, f32)>,
    /// 当該フレームで発生した、skin runtime 向けの順序付き入力・判定イベント。
    pub skin_events: Vec<SkinRuntimeEvent>,
    pub state: PlayState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinRuntimeEvent {
    pub sequence: u64,
    pub kind: SkinRuntimeEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkinRuntimeEventKind {
    Input(InputEvent),
    Judgement(JudgementEvent),
}

impl BgmScheduler {
    pub fn schedule_until(
        &mut self,
        chart: &PlayableChart,
        clock: &AudioClock,
        until: TimeUs,
        volume: f32,
        audio: &mut dyn AudioScheduler,
    ) {
        while let Some(event) = chart.bgm_events.get(self.next_index) {
            if event.time > until {
                break;
            }

            let chart_volume = bmz_chart::volume::chart_channel_volume_factor(
                bmz_chart::volume::chart_volume_at_time(&chart.bgm_volume_events, event.time),
            );
            audio.schedule(ScheduledSound {
                start_frame: clock.time_to_output_frame(event.time),
                sound_id: event.sound,
                volume: (volume * chart_volume).clamp(0.0, 1.0),
                pan: 0.0,
                loop_playback: false,
                fade_in_frames: 0,
                catch_up: true,
                restart_policy: RestartPolicy::StopSameSound,
            });

            self.next_index += 1;
        }
    }

    pub fn is_done(&self, chart: &PlayableChart) -> bool {
        self.next_index >= chart.bgm_events.len()
    }
}

pub fn compute_frame_times(session: &GameSession) -> FrameTimes {
    let audio_now = session.audio_clock.now();
    let audio_schedule_until = TimeUs(audio_now.0 + AUDIO_SCHEDULE_AHEAD_US);
    FrameTimes { audio_now, audio_schedule_until }
}
