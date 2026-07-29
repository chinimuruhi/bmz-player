use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct SkinDrawState {
    pub elapsed_ms: i32,
    /// Lua skin のruntime draw callbackにだけ渡すsidecar context。
    /// JSON skinと通常のcompiled条件では常にNone。
    #[doc(hidden)]
    pub lua_runtime: Option<SkinLuaRuntimeContext>,
    /// Lua callback をロード時に変換した runtime flag。DynamicTimerRuntime が注入する。
    pub runtime_flags: HashMap<i32, bool>,
    /// BMZ logical inputs in E1/E2/E3/E4/UI Left/Right/Up/Down order.
    pub logical_input_held: [bool; SKIN_BMZ_INPUT_COUNT],
    /// Elapsed milliseconds since the latest aggregate false-to-true press edge.
    pub logical_input_press_ms: [Option<i32>; SKIN_BMZ_INPUT_COUNT],
    /// beatoraja TIMER_STARTINPUT (1)。skin.input 待機後からの経過 ms。
    pub start_input_ms: Option<i32>,
    /// beatoraja NUMBER_CURRENT_FPS (20)。
    pub current_fps: u32,
    /// アプリ起動後の経過時間 ms。
    /// beatoraja の NUMBER_OPERATING_TIME_HOUR/MINUTE/SECOND (27..29) に使う。
    pub operating_time_ms: i32,
    pub ready_timer_ms: Option<i32>,
    pub play_timer_ms: Option<i32>,
    pub rhythm_timer_ms: Option<i32>,
    pub quarter_note_elapsed_ms: Option<i32>,
    pub key_mode: KeyMode,
    pub select_bar_elapsed_ms: i32,
    pub select_option_panel_elapsed_ms: i32,
    pub select_option_panel_off_elapsed_ms: [Option<i32>; 6],
    pub select_option_panel: u8,
    pub select_arrange_index: usize,
    pub select_arrange_2p_index: usize,
    pub select_extended_arrange_index: usize,
    pub select_extended_arrange_2p_index: usize,
    pub select_double_option_index: usize,
    pub select_hs_fix_index: usize,
    pub result_arrange_index: usize,
    pub result_arrange_2p_index: usize,
    pub result_extended_arrange_index: usize,
    pub result_extended_arrange_2p_index: usize,
    /// beatoraja RANDOM lane refs 450..469.
    ///
    /// Resultでは互換値、Play/SelectではBMZ拡張として、現在または開始予定の
    /// 固定レーン配置を画面共通で公開する。
    pub random_lane_refs: [u8; SKIN_RANDOM_LANE_REF_COUNT],
    /// Resultで実効譜面にLNが含まれるか。NoneはResult以外。
    pub result_has_long_notes: Option<bool>,
    /// Resultの実効LN種別imageset index (0=LN, 1=CN, 2=HCN)。
    pub result_ln_mode_index: Option<usize>,
    pub select_gauge_index: usize,
    pub select_gauge_auto_shift_index: usize,
    pub select_bottom_shiftable_gauge_index: usize,
    pub select_target_index: usize,
    pub select_bga_index: usize,
    pub select_assist_index: usize,
    pub select_mode_index: usize,
    pub select_sort_index: usize,
    pub select_ln_mode_index: usize,
    pub select_judge_algorithm_index: usize,
    pub mouse_x: Option<f32>,
    pub mouse_y: Option<f32>,
    pub combo: u32,
    pub max_combo: u32,
    pub ex_score: u32,
    pub total_notes: u32,
    pub past_notes: u32,
    pub result_grade_diff_display: ResultGradeDiffDisplay,
    pub result_grade_diff_f_fallback_to_e: bool,
    pub judge_counts: DisplayJudgeCounts,
    pub player_stats: PlayerStatsSnapshot,
    pub course_result: CourseResultSkinSnapshot,
    pub gauge: f32,
    pub gauge_type: i32,
    pub gauge_auto_shift: bool,
    pub gauge_max: f32,
    pub gauge_border: f32,
    pub play_progress: f32,
    pub end_of_note: bool,
    pub end_of_note_ms: Option<i32>,
    /// 各レーンのボムタイマー経過ms。Noneなら非アクティブ。
    pub bomb_ms: [Option<i32>; LANE_COUNT],
    /// 各レーンのkeyon(押下中ビーム)タイマー経過ms。Noneなら非アクティブ。
    pub keyon_ms: [Option<i32>; LANE_COUNT],
    /// 各レーンのkeyoff(離した直後の演出)タイマー経過ms。Noneなら非アクティブ。
    /// beatoraja の TIMER_KEYOFF_1P_KEY1..7 (121..127) / SCRATCH (120) に対応。
    pub keyoff_ms: [Option<i32>; LANE_COUNT],
    /// PeacefulPlay互換キービームの押下中表示状態。renderer runtimeが更新する。
    pub keybeam_hold_active: [bool; LANE_COUNT],
    /// PeacefulPlay互換キービームの解放フェード表示状態。renderer runtimeが更新する。
    pub keybeam_fade_active: [bool; LANE_COUNT],
    /// 各レーンの LN ホールドタイマー経過ms。ホールド中のみ Some。
    /// beatoraja の TIMER_HOLD_1P (70..=77) / TIMER_HOLD_2P (80..=87) に対応。
    pub hold_ms: [Option<i32>; LANE_COUNT],
    /// 各レーンの HCN ACTIVE(回復中) タイマー経過ms。
    /// beatoraja の TIMER_HCN_ACTIVE_1P (250..=257) / 2P (260..=267) に対応。
    pub hcn_active_ms: [Option<i32>; LANE_COUNT],
    /// 各レーンの HCN DAMAGE(減衰中) タイマー経過ms。
    /// beatoraja の TIMER_HCN_DAMAGE_1P (270..=277) / 2P (280..=287) に対応。
    pub hcn_damage_ms: [Option<i32>; LANE_COUNT],
    /// 各レーンの直近判定の画像インデックス (0=PGREAT,1=GREAT,2=GOOD,3=BAD,4=POOR,5=MISS)。
    /// imageset (ボム・キービーム) の画像選択に使う。Noneなら判定なし。
    pub lane_judge: [Option<usize>; LANE_COUNT],
    /// 判定タイマー経過ms。index 0/1/2 = TIMER_JUDGE_1P/2P/3P (46/47/247)。Noneなら非アクティブ。
    pub judge_ms: [Option<i32>; MAX_JUDGE_REGIONS],
    /// Full combo timer elapsed ms (TIMER_FULLCOMBO_1P/2P=48/49)。Noneなら非アクティブ。
    pub full_combo_ms: Option<i32>,
    pub full_combo_2p_ms: Option<i32>,
    pub music_end_ms: Option<i32>,
    /// Gauge increase timer elapsed ms (TIMER_GAUGE_INCLEASE_1P/2P=42/43)。
    pub gauge_increase_ms: Option<i32>,
    pub gauge_increase_2p_ms: Option<i32>,
    /// Gauge max timer elapsed ms (TIMER_GAUGE_MAX_1P/2P=44/45)。
    pub gauge_max_ms: Option<i32>,
    pub gauge_max_2p_ms: Option<i32>,
    pub end_of_note_2p_ms: Option<i32>,
    /// 領域別の判定画像インデックス (0=PGREAT,1=GREAT,2=GOOD,3=BAD,4=POOR,5=MISS)。
    pub judge_index: [Option<usize>; MAX_JUDGE_REGIONS],
    /// 領域別の判定表示用 combo。beatoraja `JudgeManager.judgecombo` 相当。
    pub judge_combo: [u32; MAX_JUDGE_REGIONS],
    /// 領域別の判定タイミング符号。1=EARLY/FAST, -1=LATE/SLOW。
    pub judge_timing_sign: [Option<i8>; MAX_JUDGE_REGIONS],
    /// OFFSET_LIFT (id=3) の y 値 (skin canvas pixel 単位)。リフト量に応じて要素をシフトする。
    pub offset_lift_px: i32,
    /// OFFSET_LANECOVER (id=4) の y 値 (skin canvas pixel 単位)。レーンカバー位置インジケータのシフト。
    pub offset_lanecover_px: i32,
    /// OFFSET_HIDDEN_COVER (id=5) の y 値 (skin canvas pixel 単位)。HIDDEN カバー位置のシフト。
    pub offset_hidden_cover_px: i32,
    /// ユーザーまたは profile で指定された任意の skin offset 値。
    pub skin_offsets: SkinOffsetValues,
    /// 現在のハイスピード倍率 (NUMBER_HISPEED=310, NUMBER_HISPEED_AFTERDOT=311 に使用)。
    pub hispeed: f32,
    /// BMZ extension: current hispeed mode. 0=NHS, 1=FHS.
    pub hispeed_mode_index: i32,
    /// BMZ extension: target green number used by FHS.
    pub target_green_number: u32,
    /// 曲残り時間 ms (NUMBER_TIMELEFT_MINUTE=163, NUMBER_TIMELEFT_SECOND=164 に使用)。
    pub timeleft_ms: i32,
    /// ノーツ表示時間 ms (NUMBER_DURATION=312 に使用)。
    pub total_duration_ms: i32,
    /// 緑数字 ms (NUMBER_DURATION_GREEN=313 に使用)。
    /// 指定されている場合は NUMBER_DURATION=312 をこの値の 5/3 倍として返す。
    pub duration_green_ms: Option<i32>,
    /// Result の曲長 ms (NUMBER_PLAYTIME_MINUTE/SECOND=1163/1164 に使用)。
    pub result_duration_ms: i32,
    /// レーンカバー割合 0.0-1.0 (NUMBER_LANECOVER1=14 は 0..=1000 で返す)。
    pub lane_cover: f32,
    /// リフト量 0.0-1.0 (NUMBER_LIFT1=314 に使用)。
    pub lift: f32,
    /// HIDDEN カバー割合 0.0-1.0。未対応の間は 0.0 で hiddenCover を描画しない。
    pub hidden_cover: f32,
    /// OPTION_LANECOVER1_CHANGING (270)。Start/Select 押下中に true。
    pub lane_cover_changing: bool,
    /// OPTION_LANECOVER1_ON (271)。
    pub lanecover_enabled: bool,
    /// OPTION_LIFT1_ON (272)。
    pub lift_enabled: bool,
    /// OPTION_HIDDEN1_ON (273)。
    pub hidden_enabled: bool,
    /// beatoraja image/index ref 342。
    pub hispeed_auto_adjust: bool,
    /// 現在 BPM (NUMBER_NOWBPM=160 に使用)。
    pub now_bpm: f32,
    /// 最小 BPM (NUMBER_MINBPM=91 に使用)。
    pub min_bpm: f32,
    /// 最大 BPM (NUMBER_MAXBPM=90 に使用)。
    pub max_bpm: f32,
    /// 現在の曲にBGAイベントが含まれるかどうか (OPTION_NO_BGA=170 / OPTION_BGA=171)。
    pub has_bga: bool,
    /// 現在の曲に STOP イベントが含まれるかどうか (OPTION_BPMSTOP=1177)。
    pub has_bpm_stop: bool,
    /// BGA表示設定がONかどうか。曲の有無とは分けて扱う。
    pub bga_enabled: bool,
    /// `#STAGEFILE` 相当の曲画像があるか (OPTION_NO_STAGEFILE=190 / OPTION_STAGEFILE=191)。
    pub has_stagefile: bool,
    /// runtime image 100 (`#STAGEFILE`) のロード済み画像サイズ。
    pub stagefile_image_size: Option<SkinImageSize>,
    /// `#BACKBMP` 相当の背景画像がロード済みか (OPTION_NO_BACKBMP=194 / OPTION_BACKBMP=195)。
    pub has_backbmp: bool,
    /// 現在表示するBGA本体画像。
    pub bga_base: Option<SkinBgaFrame>,
    /// 現在表示するBGAレイヤー画像。
    pub bga_layer: Option<SkinBgaFrame>,
    /// 現在表示するBGAレイヤー2画像 (ch 0A)。
    pub bga_layer2: Option<SkinBgaFrame>,
    /// 直近のBAD/POORで一時表示するミスレイヤー画像。
    pub bga_poor: Option<SkinBgaFrame>,
    /// BGA destination に stretch 指定が無い場合に使う拡大設定。
    pub bga_stretch: i32,
    /// 判定領域別の最後の判定タイミングずれ ms (VALUE_JUDGE_1P/2P/3P_DURATION=525/526/527 に使用)。
    /// 符号は 押下時刻 - note時刻 (FAST=負)。Noneなら非表示。
    pub judge_timing_ms: [Option<i32>; MAX_JUDGE_REGIONS],
    /// DB 上のベスト ex スコア。
    /// Result では保存前ベスト (`previous_best_ex_score`) を MYBEST 表示に優先する。
    pub best_ex_score: Option<u32>,
    /// ghost から現在進行度まで積算した過去ベスト EX。None の場合は final score の線形投影を使う。
    pub projected_best_ex_score: Option<u32>,
    /// 過去ベストのクリアタイプ index (ref 371)。
    pub best_clear_index: Option<i64>,
    /// ターゲットスコアのexスコア (NUMBER_TARGET_SCORE=121, BARGRAPH_TARGETSCORERATE=115 に使用)。
    pub target_ex_score: Option<u32>,
    /// 判定タイミングオフセット設定値 ms (NUMBER_JUDGETIMING=12 に使用、beatoraja の judgetiming 設定)。
    pub judge_timing_offset_ms: i32,
    /// beatoraja IndexType.notesdisplaytimingautoadjust (number/event id 75).
    pub judge_timing_auto_adjust: bool,
    /// 選択中 DirectoryBar に含まれる譜面数 (NUMBER_FOLDER_TOTALSONGS=300)。
    /// 非 DirectoryBar では None。これは現在の表示フォルダー全体の行数ではなく、カーソル行の値。
    pub select_folder_song_count: Option<u32>,
    /// 現在の描画状態が選曲画面かどうか。番号 ref の一部は scene ごとに意味が違う。
    pub select_screen: bool,
    /// 選曲バーのスクロール位置 0.0-1.0。
    pub select_scroll_progress: f32,
    /// 選曲画面の master/key/bgm 音量 0.0-1.0。
    pub select_master_volume: f32,
    pub select_key_volume: f32,
    pub select_bgm_volume: f32,
    /// 選択中バーにバナー画像があるか (OPTION_NO_BANNER=192 / OPTION_BANNER=193)。
    pub select_has_banner: bool,
    /// 選択中 SongData と同じフォルダに `.txt` があるか (OPTION_NO_TEXT/TEXT=174/175)。
    pub select_has_document: bool,
    /// 選択中曲のレベル表記から取り出した数値。
    pub select_play_level: i64,
    /// 現在の曲のレベル表記から取り出した数値 (NUMBER_PLAYLEVEL=96)。
    pub play_level: i64,
    /// beatoraja OPTION_TABLE_SONG (1008).
    pub table_song: bool,
    /// 現在の曲の #DIFFICULTY code。0=OTHER, 1=BEGINNER, 2=NORMAL, 3=HYPER, 4=ANOTHER, 5=INSANE。
    pub difficulty: i64,
    /// 現在の曲の #RANK / 判定ランク。0..4 は VERYHARD..VERYEASY、10 以上は直接倍率。
    pub judge_rank: Option<i32>,
    /// 選択中曲のベストEXスコア。
    pub select_ex_score: Option<u32>,
    /// 選択中曲のリプレイスロット有無。
    pub select_replay_slots: [bool; 4],
    /// 選択中リプレイスロット。Noneなら未選択。
    pub select_replay_index: Option<usize>,
    /// 選択中曲のクリアランプ番号。
    pub select_clear_index: i64,
    /// 選曲画面のお気に入り状態。beatoraja IndexType favorite_song(89) / favorite_chart(90) 用。
    /// value ref 89/90 とは別名前空間。BMZ は invisible を持たず 0/1。
    pub select_favorite_song: bool,
    pub select_favorite_chart: bool,
    /// Result の favorite_chart (image ref 90)。None は Result 以外。
    /// BMZ は invisible を持たないため Some(false/true) の2状態だけを返す。
    pub result_favorite_chart: Option<bool>,
    /// beatoraja IndexType autosave_replay1..4 (321..324) image row indices。
    pub select_replay_slot_rule_indices: [i64; 4],
    /// beatoraja ValueType folder_noplay..folder_max (320..330) 用のランプ別曲数。
    pub select_folder_lamp_counts: [u32; 11],
    /// 選択中バー種別。OPTION_FOLDERBAR / SONGBAR / GRADEBAR の判定に使う。
    pub select_row_kind: SelectRowKind,
    /// 選択中 GradeBar の制約。OPTION_GRADEBAR_* (1002..1017) の判定に使う。
    pub select_course_constraints: CourseConstraintFlags,
    /// 選択中バーがフォルダかどうか。
    pub select_is_folder: bool,
    /// 選択中 SongBar / GradeBar 相当が library.db に登録済みかどうか。
    /// OPTION_PLAYABLEBAR=5 と no-songs SkinBar 表示に使う。
    pub select_in_library: bool,
    /// 選択中曲のノーツ数。
    pub select_total_notes: u32,
    /// beatoraja SongInformation-derived selected chart detail numbers.
    pub select_chart_normal_notes: u32,
    pub select_chart_long_notes: u32,
    pub select_chart_scratch_notes: u32,
    pub select_chart_long_scratch_notes: u32,
    pub select_chart_mine_notes: u32,
    pub select_chart_density: f32,
    pub select_chart_peak_density: f32,
    pub select_chart_end_density: f32,
    pub select_chart_total_gauge: f32,
    pub select_chart_main_bpm: f32,
    /// 選択中曲の代表BPM。
    pub select_bpm: f32,
    /// 選択中曲の最小BPM。
    pub select_min_bpm: f32,
    /// 選択中曲の最大BPM。
    pub select_max_bpm: f32,
    /// 選択中曲の長さ ms。
    pub select_length_ms: i64,
    /// 選択中曲のプレイ回数 / クリア回数 / ミスカウント。
    pub select_play_count: u32,
    pub select_clear_count: u32,
    pub select_bp: Option<u32>,
    pub select_cb: Option<u32>,
    /// Fast/Slow 内訳 (ref 410-419/421-424)。
    /// Play/Result 中は Some、それ以外は None。
    pub fast_slow_counts: Option<crate::snapshot::FastSlowJudgeCounts>,
    /// PeacefulPlay key logger: 直近1秒のPress数。
    pub keylogger_nps: u32,
    /// display lane別の COOL/GREAT/GOOD/BAD 累積数。
    pub keylogger_judge_counts: [[u32; 4]; LANE_COUNT],
    /// display lane別の COOL/FAST/SLOW 累積数。
    pub keylogger_fast_slow_counts: [[u32; 3]; LANE_COUNT],
    /// display lane別、直近16 Pressの開始時刻からの経過ms。
    pub keylogger_event_ms: [[Option<i32>; 16]; LANE_COUNT],
    pub keylogger_event_judge: [[u8; 16]; LANE_COUNT],
    pub keylogger_event_fast_slow: [[u8; 16]; LANE_COUNT],
    pub keylogger_exclude_cool: bool,
    /// 過去ベスト max combo (ref 172)。
    pub best_max_combo: Option<u32>,
    /// ターゲット max combo (ref 173, 175 で使用)。
    pub target_max_combo: Option<u32>,
    /// 過去ベスト min_bp (ref 176, 178 で使用)。
    pub best_bp: Option<u32>,
    /// Result 画面で表示する今回 BP/CB。Failed では未処理ノーツを含む記録用値。
    pub result_bp: Option<u32>,
    pub result_cb: Option<u32>,
    /// Result 画面の IR ランキング状態 (NUMBER_IR_* / OPTION_IR_*)。
    pub ir_ranking: crate::scene::ResultIrSnapshot,
    /// 選曲カーソル譜面の IR ライバルベスト EX (NUMBER_RIVAL_SCORE=271)。
    pub rival_ex_score: Option<i64>,
    /// 同 max combo (NUMBER_RIVAL_MAXCOMBO=275)。
    pub rival_max_combo: Option<i64>,
    /// 同 BP (NUMBER_RIVAL_MISSCOUNT=276)。
    pub rival_bp: Option<i64>,
    /// EXスコア元プレイの PGREAT/GREAT/GOOD/BAD/POOR
    /// (NUMBER/FLOAT_RIVAL_*=280..289)。
    pub rival_judge_counts: Option<[u32; 5]>,
    /// Result update/draw ops 用の保存前ベスト。
    pub previous_best_ex_score: Option<u32>,
    pub previous_best_clear_index: Option<i64>,
    pub previous_best_max_combo: Option<u32>,
    pub previous_best_bp: Option<u32>,
    /// ターゲット min_bp (ref 176, 178 で使用)。
    pub target_bp: Option<u32>,
    /// ターゲットクリアタイプの index (ref 371)。
    pub target_clear_index: Option<i64>,
    /// リザルト画面でクリアしたか (op 90=CLEAR, op 91=FAIL)。
    /// Play 中は None、Result 中は Some(true)=Fail / Some(false)=Clear。
    pub result_failed: Option<bool>,
    /// シーン終了フェードアウトのタイマー経過 ms (TIMER_FADEOUT=2)。
    /// None ならフェードアウト中でない。`timer: 2` の destination はこの値が
    /// Some のときだけ描画され、リザルト画面終了時のアニメーションを駆動する。
    pub fadeout_ms: Option<i32>,
    /// RESULT graph begin/end timers (150/151) and update score timer (152)。
    pub result_graph_begin_ms: Option<i32>,
    pub result_graph_end_ms: Option<i32>,
    pub result_update_score_ms: Option<i32>,
    /// Result gaugegraph display type selected by result key CHANGE_GRAPH.
    pub result_gauge_graph_type: Option<i32>,
    /// Lua Result スキンの展開パネル (0=非表示、1=IR、2=グラフ)。
    pub result_panel: Option<i32>,
    /// RESULT replay slot status for OPTION_REPLAYDATA* / *_SAVED.
    pub result_replay_slots: [bool; 4],
    pub result_saved_replay_slots: [bool; 4],
    /// 閉店/FAILED 演出のタイマー経過 ms (TIMER_FAILED=3)。
    pub failed_ms: Option<i32>,
    /// Result timing distribution average (NUMBER_AVERAGE_TIMING=374).
    pub average_timing_ms: Option<f32>,
    /// Result全ノートの平均絶対判定ずれ us (NUMBER_AVERAGE_DURATION=372/373)。
    /// 未判定ノートは beatoraja と同じく 1,000,000us として集計する。
    pub average_duration_us: Option<i64>,
    /// Result timing distribution standard deviation (NUMBER_STDDEV_TIMING=376).
    pub stddev_timing_ms: Option<f32>,
    /// OPTION_AUTOPLAYON (33) / OPTION_AUTOPLAYOFF (32) 用。
    pub autoplay: bool,
    /// BMSPlayer のプレイ画面か。プレイ専用 op が他 scene で true にならないために使う。
    pub play_screen: bool,
    /// BMSPlayer が replay モードか。
    pub replay_playback: bool,
    /// BMSPlayer が practice モードか。
    pub practice_mode: bool,
    /// beatoraja PlayerResource.updateScore。None は対象外 scene。
    pub score_save_enabled: Option<bool>,
    /// OPTION_NOW_LOADING (80) / OPTION_LOADED (81) 用。
    pub skin_loaded: bool,
    /// NUMBER_LOADING_PROGRESS (165) / RateType load_progress (102) 用。
    pub resource_load_progress: f32,
    /// OPTION_MODE_COURSE (290) とステージ別 op (280..283 / 289) 用。未対応時は None。
    pub course_stage: Option<CourseStageMarker>,
    /// beatoraja `event_index(SKIN_EVENT_HSFIX)`。0=OFF, 1=START, 2=MAX, 3=MAIN, 4=MIN。
    pub hsfix_index: i32,
    /// beatoraja `NUMBER_MAINBPM` (92) 用の代表 BPM (プレイ中)。
    pub main_bpm: f32,
    /// Rm-skin F/S threshold 表示 (ms)。
    pub fs_threshold_ms: i32,
    /// HSFIX 連動の adjusted hidden cover (0..1)。
    pub adjusted_cover_progress: Option<f32>,
    /// HSFIX 連動の BPM 比率 (0..1)。
    pub adjusted_rate: Option<f32>,
    /// HSFIX 連動の BPM 比率 ×100 整数部。
    pub adjusted_rate_adot: Option<i32>,
    /// HitErrorVisualizer 用の直近判定タイミング (ms)。
    pub hit_error_ring: [i64; bmz_gameplay::hit_error::HIT_ERROR_RING_LEN],
    pub hit_error_ring_index: usize,
    /// `dynamicTimer` で定義された observe タイマーの経過 ms。None は timer_off。
    pub dynamic_timer_ms: [Option<i32>; SKIN_DYNAMIC_TIMER_COUNT],
    /// Lua custom timerのうち固定delayとしてIR化できたタイマーの経過ms。
    pub fixed_delay_timer_ms: HashMap<i32, i32>,
    /// 選曲画面の設定フォルダ内。曲メタデータ用の op / text / number を抑制する。
    pub in_settings: bool,
    /// 設定項目の編集モード中 (`in_settings` と併用)。
    pub settings_editing: bool,
    /// 選曲中の曲行キーモード。beatoraja OPTION_MODE_* (160..164) 用。
    pub select_chart_key_mode: Option<KeyMode>,
}

impl Default for SkinDrawState {
    fn default() -> Self {
        Self {
            elapsed_ms: 0,
            lua_runtime: None,
            runtime_flags: HashMap::new(),
            logical_input_held: [false; SKIN_BMZ_INPUT_COUNT],
            logical_input_press_ms: [None; SKIN_BMZ_INPUT_COUNT],
            start_input_ms: None,
            current_fps: 0,
            operating_time_ms: 0,
            ready_timer_ms: None,
            play_timer_ms: None,
            rhythm_timer_ms: None,
            quarter_note_elapsed_ms: None,
            key_mode: KeyMode::default(),
            select_bar_elapsed_ms: 0,
            select_option_panel_elapsed_ms: 0,
            select_option_panel_off_elapsed_ms: [None; 6],
            select_option_panel: 0,
            select_arrange_index: 0,
            select_arrange_2p_index: 0,
            select_extended_arrange_index: 0,
            select_extended_arrange_2p_index: 0,
            select_double_option_index: 0,
            select_hs_fix_index: 0,
            result_arrange_index: 0,
            result_arrange_2p_index: 0,
            result_extended_arrange_index: 0,
            result_extended_arrange_2p_index: 0,
            random_lane_refs: [0; SKIN_RANDOM_LANE_REF_COUNT],
            result_has_long_notes: None,
            result_ln_mode_index: None,
            select_gauge_index: 2,
            select_gauge_auto_shift_index: 0,
            select_bottom_shiftable_gauge_index: 0,
            select_target_index: 0,
            select_bga_index: 0,
            select_assist_index: 0,
            select_mode_index: 0,
            select_sort_index: 0,
            select_ln_mode_index: 0,
            select_judge_algorithm_index: 0,
            mouse_x: None,
            mouse_y: None,
            combo: 0,
            max_combo: 0,
            ex_score: 0,
            total_notes: 0,
            past_notes: 0,
            result_grade_diff_display: ResultGradeDiffDisplay::default(),
            result_grade_diff_f_fallback_to_e: false,
            judge_counts: DisplayJudgeCounts::default(),
            player_stats: PlayerStatsSnapshot::default(),
            course_result: CourseResultSkinSnapshot::default(),
            gauge: 0.0,
            gauge_type: 2,
            gauge_auto_shift: false,
            gauge_max: 100.0,
            gauge_border: 80.0,
            play_progress: 0.0,
            end_of_note: false,
            end_of_note_ms: None,
            bomb_ms: [None; LANE_COUNT],
            keyon_ms: [None; LANE_COUNT],
            keyoff_ms: [None; LANE_COUNT],
            keybeam_hold_active: [false; LANE_COUNT],
            keybeam_fade_active: [false; LANE_COUNT],
            hold_ms: [None; LANE_COUNT],
            hcn_active_ms: [None; LANE_COUNT],
            hcn_damage_ms: [None; LANE_COUNT],
            lane_judge: [None; LANE_COUNT],
            judge_ms: [None; MAX_JUDGE_REGIONS],
            full_combo_ms: None,
            full_combo_2p_ms: None,
            music_end_ms: None,
            gauge_increase_ms: None,
            gauge_increase_2p_ms: None,
            gauge_max_ms: None,
            gauge_max_2p_ms: None,
            end_of_note_2p_ms: None,
            judge_index: [None; MAX_JUDGE_REGIONS],
            judge_combo: [0; MAX_JUDGE_REGIONS],
            judge_timing_sign: [None; MAX_JUDGE_REGIONS],
            offset_lift_px: 0,
            offset_lanecover_px: 0,
            offset_hidden_cover_px: 0,
            skin_offsets: SkinOffsetValues::default(),
            hispeed: 0.0,
            hispeed_mode_index: 0,
            target_green_number: 0,
            timeleft_ms: 0,
            total_duration_ms: 0,
            duration_green_ms: None,
            result_duration_ms: 0,
            lane_cover: 0.0,
            lift: 0.0,
            hidden_cover: 0.0,
            lane_cover_changing: false,
            lanecover_enabled: true,
            lift_enabled: true,
            hidden_enabled: false,
            hispeed_auto_adjust: false,
            now_bpm: 0.0,
            min_bpm: 0.0,
            max_bpm: 0.0,
            has_bga: false,
            has_bpm_stop: false,
            bga_enabled: true,
            has_stagefile: false,
            stagefile_image_size: None,
            has_backbmp: false,
            bga_base: None,
            bga_layer: None,
            bga_layer2: None,
            bga_poor: None,
            bga_stretch: 1,
            judge_timing_ms: [None; MAX_JUDGE_REGIONS],
            best_ex_score: None,
            projected_best_ex_score: None,
            best_clear_index: None,
            target_ex_score: None,
            judge_timing_offset_ms: 0,
            judge_timing_auto_adjust: false,
            select_folder_song_count: None,
            select_screen: false,
            select_scroll_progress: 0.0,
            select_master_volume: 1.0,
            select_key_volume: 1.0,
            select_bgm_volume: 1.0,
            select_has_banner: false,
            select_has_document: false,
            select_play_level: 0,
            play_level: 0,
            table_song: false,
            difficulty: 0,
            judge_rank: None,
            select_ex_score: None,
            select_replay_slots: [false; 4],
            select_replay_index: None,
            select_clear_index: 0,
            select_favorite_song: false,
            select_favorite_chart: false,
            result_favorite_chart: None,
            select_replay_slot_rule_indices: [0; 4],
            select_folder_lamp_counts: [0; 11],
            select_row_kind: SelectRowKind::Song,
            select_course_constraints: CourseConstraintFlags::default(),
            select_is_folder: false,
            select_in_library: true,
            select_total_notes: 0,
            select_chart_normal_notes: 0,
            select_chart_long_notes: 0,
            select_chart_scratch_notes: 0,
            select_chart_long_scratch_notes: 0,
            select_chart_mine_notes: 0,
            select_chart_density: 0.0,
            select_chart_peak_density: 0.0,
            select_chart_end_density: 0.0,
            select_chart_total_gauge: 0.0,
            select_chart_main_bpm: 0.0,
            select_bpm: 0.0,
            select_min_bpm: 0.0,
            select_max_bpm: 0.0,
            select_length_ms: 0,
            select_play_count: 0,
            select_clear_count: 0,
            select_bp: None,
            select_cb: None,
            fast_slow_counts: None,
            keylogger_nps: 0,
            keylogger_judge_counts: [[0; 4]; LANE_COUNT],
            keylogger_fast_slow_counts: [[0; 3]; LANE_COUNT],
            keylogger_event_ms: [[None; 16]; LANE_COUNT],
            keylogger_event_judge: [[0; 16]; LANE_COUNT],
            keylogger_event_fast_slow: [[0; 16]; LANE_COUNT],
            keylogger_exclude_cool: false,
            best_max_combo: None,
            target_max_combo: None,
            best_bp: None,
            result_bp: None,
            result_cb: None,
            ir_ranking: crate::scene::ResultIrSnapshot::default(),
            rival_ex_score: None,
            rival_max_combo: None,
            rival_bp: None,
            rival_judge_counts: None,
            previous_best_ex_score: None,
            previous_best_clear_index: None,
            previous_best_max_combo: None,
            previous_best_bp: None,
            target_bp: None,
            target_clear_index: None,
            result_failed: None,
            fadeout_ms: None,
            result_graph_begin_ms: None,
            result_graph_end_ms: None,
            result_update_score_ms: None,
            result_gauge_graph_type: None,
            result_panel: None,
            result_replay_slots: [false; 4],
            result_saved_replay_slots: [false; 4],
            failed_ms: None,
            average_timing_ms: None,
            average_duration_us: None,
            stddev_timing_ms: None,
            autoplay: false,
            play_screen: false,
            replay_playback: false,
            practice_mode: false,
            score_save_enabled: None,
            skin_loaded: true,
            resource_load_progress: 1.0,
            course_stage: None,
            hsfix_index: 0,
            main_bpm: 0.0,
            fs_threshold_ms: 25,
            adjusted_cover_progress: None,
            adjusted_rate: None,
            adjusted_rate_adot: None,
            hit_error_ring: [bmz_gameplay::hit_error::HIT_ERROR_EMPTY;
                bmz_gameplay::hit_error::HIT_ERROR_RING_LEN],
            hit_error_ring_index: 0,
            dynamic_timer_ms: [None; SKIN_DYNAMIC_TIMER_COUNT],
            fixed_delay_timer_ms: HashMap::new(),
            in_settings: false,
            settings_editing: false,
            select_chart_key_mode: None,
        }
    }
}

/// `dynamicTimer` observe 条件のエッジ検出用ランタイム。Renderer が保持する。
#[derive(Debug, Clone)]
pub struct DynamicTimerRuntime {
    runtime_flags: HashMap<i32, bool>,
    runtime_flags_initialized: bool,
    starts: [Option<i32>; SKIN_DYNAMIC_TIMER_COUNT],
    logical_input_initialized: bool,
    logical_input_held: [bool; SKIN_BMZ_INPUT_COUNT],
    logical_input_starts: [Option<i32>; SKIN_BMZ_INPUT_COUNT],
    keybeam_keyon_starts: [Option<i32>; LANE_COUNT],
    keybeam_keyoff_starts: [Option<i32>; LANE_COUNT],
    keybeam_suppressed: [bool; LANE_COUNT],
    keybeam_fade_allowed: [bool; LANE_COUNT],
    key_logger: KeyLoggerRuntime,
}

#[derive(Debug, Clone, Default)]
pub(super) struct KeyLoggerRuntime {
    last_sequence: Option<u64>,
    last_now_us: Option<i64>,
    press_history_us: VecDeque<i64>,
    judge_counts: [[u32; 4]; LANE_COUNT],
    fast_slow_counts: [[u32; 3]; LANE_COUNT],
    event_started_ms: [[Option<i32>; 16]; LANE_COUNT],
    event_started_us: [[Option<i64>; 16]; LANE_COUNT],
    event_judge: [[u8; 16]; LANE_COUNT],
    event_fast_slow: [[u8; 16]; LANE_COUNT],
    next_event_slot: [usize; LANE_COUNT],
}

impl Default for DynamicTimerRuntime {
    fn default() -> Self {
        Self {
            runtime_flags: HashMap::new(),
            runtime_flags_initialized: false,
            starts: [None; SKIN_DYNAMIC_TIMER_COUNT],
            logical_input_initialized: false,
            logical_input_held: [false; SKIN_BMZ_INPUT_COUNT],
            logical_input_starts: [None; SKIN_BMZ_INPUT_COUNT],
            keybeam_keyon_starts: [None; LANE_COUNT],
            keybeam_keyoff_starts: [None; LANE_COUNT],
            keybeam_suppressed: [false; LANE_COUNT],
            keybeam_fade_allowed: [false; LANE_COUNT],
            key_logger: KeyLoggerRuntime::default(),
        }
    }
}

impl DynamicTimerRuntime {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// スキン install / scene 再入場時に、timer と runtime flag を初期状態へ戻す。
    pub fn reset_for_document(&mut self, document: Option<&SkinDocument>) {
        self.reset();
        if let Some(document) = document {
            self.initialize_runtime_flags(document);
        }
    }

    /// 宣言済み runtime event を dispatch する。対象 event がなければ false。
    pub fn dispatch_runtime_event(&mut self, document: &SkinDocument, event_id: i32) -> bool {
        self.ensure_runtime_flags(document);
        let mut handled = false;
        for event in document.runtime_events.iter().filter(|event| event.id == event_id) {
            handled = true;
            for flag_id in &event.toggle_flags {
                let flag = self.runtime_flags.entry(*flag_id).or_insert(false);
                *flag = !*flag;
            }
        }
        handled
    }

    /// observe 条件を評価し、`state.dynamic_timer_ms` を更新する。
    pub fn advance(&mut self, document: &SkinDocument, state: &mut SkinDrawState, now_ms: i32) {
        self.ensure_runtime_flags(document);
        self.advance_logical_inputs(document, state.logical_input_held, now_ms);
        state.runtime_flags.clone_from(&self.runtime_flags);
        state.logical_input_press_ms =
            self.logical_input_starts.map(|start| start.map(|start| now_ms.saturating_sub(start)));
        self.advance_keybeam(state, now_ms);
        self.key_logger.write_state(state, now_ms);
        state.keylogger_exclude_cool = !document.graph.iter().any(|graph| {
            graph.id.starts_with("keylogger-graph-judge-") && graph.id.ends_with("-cool")
        });
        state.fixed_delay_timer_ms.clear();
        for def in &document.fixed_delay_timers {
            let Some(source_elapsed) = skin_timer_elapsed_ms(Some(def.source_timer), state) else {
                continue;
            };
            if source_elapsed >= def.delay_ms {
                state
                    .fixed_delay_timer_ms
                    .insert(def.id, source_elapsed.saturating_sub(def.delay_ms));
            }
        }
        for def in &document.dynamic_timers {
            let idx = def.id.saturating_sub(SKIN_DYNAMIC_TIMER_BASE) as usize;
            if idx >= SKIN_DYNAMIC_TIMER_COUNT {
                continue;
            }
            if eval_skin_draw_condition(&def.observe, state) {
                let start = self.starts[idx].get_or_insert(now_ms);
                state.dynamic_timer_ms[idx] = Some(now_ms.saturating_sub(*start));
            } else {
                self.starts[idx] = None;
                state.dynamic_timer_ms[idx] = None;
            }
        }
    }

    fn ensure_runtime_flags(&mut self, document: &SkinDocument) {
        if !self.runtime_flags_initialized {
            self.initialize_runtime_flags(document);
        }
    }

    fn advance_logical_inputs(
        &mut self,
        document: &SkinDocument,
        held: [bool; SKIN_BMZ_INPUT_COUNT],
        now_ms: i32,
    ) {
        if !self.logical_input_initialized {
            self.logical_input_initialized = true;
            self.logical_input_held = held;
            return;
        }
        for (index, &is_held) in held.iter().enumerate() {
            if is_held && !self.logical_input_held[index] {
                self.logical_input_starts[index] = Some(now_ms);
                for event in document.runtime_events.iter().filter(|event| {
                    event.trigger_action.is_some_and(|action| action.index() == index)
                }) {
                    for flag_id in &event.toggle_flags {
                        let flag = self.runtime_flags.entry(*flag_id).or_insert(false);
                        *flag = !*flag;
                    }
                }
            }
        }
        self.logical_input_held = held;
    }

    fn initialize_runtime_flags(&mut self, document: &SkinDocument) {
        self.runtime_flags =
            document.runtime_flags.iter().map(|flag| (flag.id, flag.initial)).collect();
        self.runtime_flags_initialized = true;
    }

    pub fn ingest_skin_events(
        &mut self,
        events: &[SkinRuntimeEvent],
        key_mode: KeyMode,
        now_us: i64,
    ) {
        self.key_logger.ingest(events, key_mode, now_us);
    }

    fn advance_keybeam(&mut self, state: &mut SkinDrawState, now_ms: i32) {
        for lane in 0..LANE_COUNT {
            let keyon_start = state.keyon_ms[lane].map(|elapsed| now_ms.saturating_sub(elapsed));
            let keyoff_start = state.keyoff_ms[lane].map(|elapsed| now_ms.saturating_sub(elapsed));

            if keyon_start.is_some() && keyon_start != self.keybeam_keyon_starts[lane] {
                self.keybeam_suppressed[lane] = false;
                self.keybeam_fade_allowed[lane] = false;
            }
            if state.hold_ms[lane].is_some() && state.keyon_ms[lane].is_some() {
                self.keybeam_suppressed[lane] = true;
            }

            state.keybeam_hold_active[lane] =
                state.keyon_ms[lane].is_some() && !self.keybeam_suppressed[lane];
            if keyoff_start.is_some() && keyoff_start != self.keybeam_keyoff_starts[lane] {
                self.keybeam_fade_allowed[lane] = !self.keybeam_suppressed[lane];
                self.keybeam_suppressed[lane] = false;
            }
            state.keybeam_fade_active[lane] =
                keyoff_start.is_some() && self.keybeam_fade_allowed[lane];
            self.keybeam_keyon_starts[lane] = keyon_start;
            self.keybeam_keyoff_starts[lane] = keyoff_start;
        }
    }
}

impl KeyLoggerRuntime {
    pub(super) fn ingest(&mut self, events: &[SkinRuntimeEvent], key_mode: KeyMode, now_us: i64) {
        if self.last_now_us.is_some_and(|last| now_us < last) {
            *self = Self::default();
        }
        self.last_now_us = Some(now_us);
        let active_lanes = key_mode.active_lanes();
        for event in events {
            if self.last_sequence.is_some_and(|last| event.sequence <= last) {
                continue;
            }
            self.last_sequence = Some(event.sequence);
            match &event.kind {
                SkinRuntimeEventKind::Input(input) if input.kind == InputKind::Press => {
                    let Some(lane) =
                        active_lanes.iter().position(|candidate| *candidate == input.lane)
                    else {
                        continue;
                    };
                    self.press_history_us.push_back(input.time.0);
                    let slot = self.next_event_slot[lane];
                    self.event_started_ms[lane][slot] =
                        Some((input.time.0 / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32);
                    self.event_started_us[lane][slot] = Some(input.time.0);
                    self.event_judge[lane][slot] = 0;
                    self.event_fast_slow[lane][slot] = 0;
                    self.next_event_slot[lane] = (slot + 1) % 16;
                }
                SkinRuntimeEventKind::Judgement(judgement) => {
                    let Some(lane) =
                        active_lanes.iter().position(|candidate| *candidate == judgement.lane)
                    else {
                        continue;
                    };
                    let judge = match judgement.judge {
                        Judge::PGreat => 0,
                        Judge::Great => 1,
                        Judge::Good => 2,
                        Judge::Bad | Judge::Poor | Judge::EmptyPoor => 3,
                    };
                    self.judge_counts[lane][judge] =
                        self.judge_counts[lane][judge].saturating_add(1);
                    let side = match judgement.judge {
                        Judge::PGreat => Some(0),
                        _ => match judgement.side {
                            TimingSide::Fast => Some(1),
                            TimingSide::Slow => Some(2),
                        },
                    };
                    if let Some(side) = side {
                        self.fast_slow_counts[lane][side] =
                            self.fast_slow_counts[lane][side].saturating_add(1);
                    }
                    let slot = (self.next_event_slot[lane] + 15) % 16;
                    if self.event_started_us[lane][slot] == Some(judgement.time.0) {
                        self.event_judge[lane][slot] = (judge + 1) as u8;
                        self.event_fast_slow[lane][slot] = side.map_or(0, |side| (side + 1) as u8);
                    }
                }
                _ => {}
            }
        }
        let keep_from = now_us.saturating_sub(1_000_000);
        while self.press_history_us.front().is_some_and(|time| *time < keep_from) {
            self.press_history_us.pop_front();
        }
    }

    pub(super) fn write_state(&self, state: &mut SkinDrawState, now_ms: i32) {
        state.keylogger_nps = self.press_history_us.len().min(999) as u32;
        state.keylogger_judge_counts = self.judge_counts;
        state.keylogger_fast_slow_counts = self.fast_slow_counts;
        state.keylogger_event_judge = self.event_judge;
        state.keylogger_event_fast_slow = self.event_fast_slow;
        for lane in 0..LANE_COUNT {
            for slot in 0..16 {
                state.keylogger_event_ms[lane][slot] =
                    self.event_started_ms[lane][slot].map(|started| now_ms.saturating_sub(started));
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct SkinRuntimeGraphs<'a> {
    pub(super) play_judge_graph_density: &'a [u8],
    pub(super) play_bpm_graph_segments: &'a [crate::chart_graph::BpmGraphSegment],
    pub(super) result_gauge_graph_points: &'a [crate::snapshot::ResultGaugeGraphPoint],
    pub(super) result_timing_points: &'a [crate::snapshot::ResultTimingPoint],
    pub(super) result_judge_graph_buckets: &'a [crate::snapshot::ResultJudgeGraphBucket],
    pub(super) result_early_late_graph_buckets: &'a [crate::snapshot::ResultEarlyLateGraphBucket],
    pub(super) result_timing_distribution: &'a crate::snapshot::ResultTimingDistribution,
}

impl<'a> SkinRuntimeGraphs<'a> {
    pub(super) fn from_document(document: &'a SkinDocument) -> Self {
        Self {
            play_judge_graph_density: &document.play_judge_graph_density,
            play_bpm_graph_segments: &document.play_bpm_graph_segments,
            result_gauge_graph_points: &document.result_gauge_graph_points,
            result_timing_points: &document.result_timing_points,
            result_judge_graph_buckets: &document.result_judge_graph_buckets,
            result_early_late_graph_buckets: &document.result_early_late_graph_buckets,
            result_timing_distribution: &document.result_timing_distribution,
        }
    }

    pub(super) fn from_document_with_play_graphs(
        document: &'a SkinDocument,
        play_judge_graph_density: &'a [u8],
        play_bpm_graph_segments: &'a [crate::chart_graph::BpmGraphSegment],
    ) -> Self {
        Self {
            play_judge_graph_density,
            play_bpm_graph_segments,
            result_gauge_graph_points: &document.result_gauge_graph_points,
            result_timing_points: &document.result_timing_points,
            result_judge_graph_buckets: &document.result_judge_graph_buckets,
            result_early_late_graph_buckets: &document.result_early_late_graph_buckets,
            result_timing_distribution: &document.result_timing_distribution,
        }
    }

    pub(super) fn from_result_graph(graph: &'a crate::snapshot::ResultGraphSnapshot) -> Self {
        Self {
            play_judge_graph_density: &graph.judge_graph_density,
            play_bpm_graph_segments: &graph.bpm_graph_segments,
            result_gauge_graph_points: &graph.gauge_points,
            result_timing_points: &graph.timing_points,
            result_judge_graph_buckets: &graph.judge_graph_buckets,
            result_early_late_graph_buckets: &graph.early_late_graph_buckets,
            result_timing_distribution: &graph.timing_distribution,
        }
    }
}

pub(super) struct DestinationResolveContext<'a, 'text> {
    pub(super) images: &'a HashMap<&'a str, &'a SkinImageDef>,
    pub(super) values: &'a HashMap<&'a str, &'a SkinValueDef>,
    pub(super) enabled_options: &'a [i32],
    pub(super) state: &'a SkinDrawState,
    pub(super) text_state: &'a SkinTextState<'text>,
    pub(super) sources: &'a HashMap<String, SkinDocumentTexture>,
    pub(super) runtime_graphs: SkinRuntimeGraphs<'a>,
    pub(super) has_nearest_f_diff_rank_destination: bool,
    pub(super) cache: Option<&'a mut ResultRenderCache>,
}

/// beatoraja `PlaySkin.judgeregion` 上限 (TIMER_JUDGE_1P/2P/3P = 46/47/247)。
pub const MAX_JUDGE_REGIONS: usize = 3;
pub(super) const LUA_DRAW_CALLBACK_PREFIX: &str = "bmz:lua_draw_callback:";

/// Renderer-facing interface for Lua draw sidecars. Implementations own the VM
/// outside `SkinDocument`; the renderer only supplies a read-only frame state.
pub trait SkinLuaDrawRuntime: std::fmt::Debug + Send + Sync {
    fn evaluate_draw(
        &self,
        callback_id: usize,
        state: &SkinDrawState,
        enabled_options: &[i32],
        text_values: &BTreeMap<i32, String>,
    ) -> bool;
}

#[derive(Clone)]
pub struct SkinLuaRuntimeContext {
    pub(super) runtime: Arc<dyn SkinLuaDrawRuntime>,
    pub(super) enabled_options: Arc<[i32]>,
    pub(super) text_values: Arc<BTreeMap<i32, String>>,
}

impl std::fmt::Debug for SkinLuaRuntimeContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SkinLuaRuntimeContext")
            .field("enabled_options", &self.enabled_options)
            .field("text_value_count", &self.text_values.len())
            .finish_non_exhaustive()
    }
}

impl PartialEq for SkinLuaRuntimeContext {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.runtime, &other.runtime)
            && self.enabled_options == other.enabled_options
            && self.text_values == other.text_values
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JudgeRegionState {
    pub judge_ms: [Option<i32>; MAX_JUDGE_REGIONS],
    pub judge_index: [Option<usize>; MAX_JUDGE_REGIONS],
    pub judge_combo: [u32; MAX_JUDGE_REGIONS],
    pub judge_timing_sign: [Option<i8>; MAX_JUDGE_REGIONS],
    /// 領域別の最新判定タイミングずれ ms (VALUE_JUDGE_1P/2P/3P_DURATION=525/526/527 に使用)。
    /// 符号は 押下時刻 - note時刻 (FAST=負)。None なら非表示。
    pub judge_timing_ms: [Option<i32>; MAX_JUDGE_REGIONS],
}

/// レーン index から判定領域 index へ (beatoraja `JudgeManager.updateMicro` 同式)。
pub fn lane_judge_region(lane_index: usize, lane_count: usize, region_count: usize) -> usize {
    if lane_count == 0 || region_count == 0 {
        return 0;
    }
    let region = lane_index * region_count / lane_count;
    region.min(region_count.saturating_sub(1))
}

/// `recent_judgements` から領域別の判定 timer / 画像 index を構築する。
pub fn build_judge_region_state(
    recent_judgements: &[crate::snapshot::DisplayJudgement],
    render_now_us: i64,
    region_count: usize,
) -> JudgeRegionState {
    let mut judge_ms = [None; MAX_JUDGE_REGIONS];
    let mut judge_index = [None; MAX_JUDGE_REGIONS];
    let mut judge_combo = [0; MAX_JUDGE_REGIONS];
    let mut judge_timing_sign = [None; MAX_JUDGE_REGIONS];
    let mut judge_timing_ms = [None; MAX_JUDGE_REGIONS];
    let region_count = region_count.min(MAX_JUDGE_REGIONS);
    for judgement in recent_judgements.iter().rev() {
        let region = lane_judge_region(judgement.lane.index(), LANE_COUNT, region_count);
        if judge_ms[region].is_some() {
            continue;
        }
        judge_ms[region] = Some(
            ((render_now_us - judgement.time.0) / 1_000).clamp(i32::MIN as i64, i32::MAX as i64)
                as i32,
        );
        judge_index[region] = Some(judge_image_index_for_judge(judgement.judge));
        judge_combo[region] = judgement.combo;
        judge_timing_sign[region] = judgement.side.map(|side| match side {
            TimingSide::Fast => 1,
            TimingSide::Slow => -1,
        });
        if !judgement.timing_ms_suppressed {
            judge_timing_ms[region] =
                Some((judgement.delta_us / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32);
        }
    }
    JudgeRegionState { judge_ms, judge_index, judge_combo, judge_timing_sign, judge_timing_ms }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinClickTarget {
    Event { event_id: i32, click: i32 },
    SelectRow { row_index: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinClickHit {
    pub target: SkinClickTarget,
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinSliderHit {
    pub slider_type: i32,
    pub value: f32,
}

#[derive(Debug, Clone)]
pub struct SkinContext {
    manifest: SkinManifest,
    document: Option<SkinDocument>,
    lua_draw_runtime: Option<Arc<dyn SkinLuaDrawRuntime>>,
    document_sources: HashMap<String, SkinDocumentTexture>,
    select_settings_dest_index: Arc<crate::select_settings_dest::SelectSettingsDestIndex>,
    result_render_cache: Arc<Mutex<ResultRenderCache>>,
}

impl PartialEq for SkinContext {
    fn eq(&self, other: &Self) -> bool {
        self.manifest == other.manifest
            && self.document == other.document
            && match (&self.lua_draw_runtime, &other.lua_draw_runtime) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
            && self.document_sources == other.document_sources
            && self.select_settings_dest_index == other.select_settings_dest_index
    }
}

pub(super) const RESULT_RENDER_CACHE_MAX_ENTRIES: usize = 64;
static NEXT_RESULT_GAUGE_GRAPH_REVISION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Default)]
pub(super) struct ResultRenderCache {
    planning: Option<ResultPlanningCache>,
    rect_batches: HashMap<ResultRectBatchCacheKey, Arc<[RectCommand]>>,
    gauge_graph: Option<ResultGaugeGraphCache>,
    gauge_rect_batches: HashMap<ResultGaugeGraphRectBatchCacheKey, Arc<[RectCommand]>>,
}

impl ResultRenderCache {
    pub(super) fn cached_planning(&mut self, document: &SkinDocument) -> ResultPlanningCache {
        if let Some(planning) = &self.planning {
            return planning.clone();
        }
        let enabled_options = Arc::<[i32]>::from(document.enabled_options());
        let mut destinations = Vec::new();
        for (entry_index, entry) in document.destination.iter().enumerate() {
            match entry {
                DestinationListEntry::Single(_) => {
                    destinations.push(ResultDestinationRef::Single { entry_index });
                }
                DestinationListEntry::Conditional { if_ops, destinations: entries } => {
                    if test_skin_dst_if(if_ops, &enabled_options) {
                        destinations.extend(entries.iter().enumerate().map(
                            |(destination_index, _)| ResultDestinationRef::Conditional {
                                entry_index,
                                destination_index,
                            },
                        ));
                    }
                }
            }
        }
        let has_nearest_f_diff_rank_destination = destinations
            .iter()
            .filter_map(|destination| destination.resolve(document))
            .any(|destination| destination.id == "RANK_s_F");
        let planning = ResultPlanningCache {
            enabled_options,
            destinations: Arc::from(destinations),
            has_nearest_f_diff_rank_destination,
        };
        self.planning = Some(planning.clone());
        planning
    }

    pub(super) fn cached_rect_batch(
        &mut self,
        key: ResultRectBatchCacheKey,
        build: impl FnOnce() -> Arc<[RectCommand]>,
    ) -> Arc<[RectCommand]> {
        if let Some(rects) = self.rect_batches.get(&key) {
            return Arc::clone(rects);
        }
        let rects = build();
        if self.rect_batches.len() >= RESULT_RENDER_CACHE_MAX_ENTRIES {
            self.rect_batches.clear();
        }
        self.rect_batches.insert(key, Arc::clone(&rects));
        rects
    }

    pub(super) fn prepare_gauge_graph(
        &mut self,
        graph: &Arc<crate::snapshot::ResultGraphSnapshot>,
    ) {
        if self.gauge_graph.as_ref().is_some_and(|cached| Arc::ptr_eq(&cached.graph, graph)) {
            return;
        }
        let revision = NEXT_RESULT_GAUGE_GRAPH_REVISION.fetch_add(1, Ordering::Relaxed);
        self.gauge_graph = Some(ResultGaugeGraphCache {
            graph: Arc::clone(graph),
            revision,
            points_by_type: HashMap::new(),
        });
        if self.gauge_rect_batches.len() >= RESULT_RENDER_CACHE_MAX_ENTRIES {
            self.gauge_rect_batches.clear();
        }
    }

    pub(super) fn cached_gauge_points(
        &mut self,
        gauge_type: i32,
    ) -> Option<(u64, Arc<[crate::snapshot::ResultGaugeGraphPoint]>)> {
        let cached = self.gauge_graph.as_mut()?;
        let points = cached
            .points_by_type
            .entry(gauge_type)
            .or_insert_with(|| {
                let filtered = cached
                    .graph
                    .gauge_points
                    .iter()
                    .copied()
                    .filter(|point| point.gauge_type == gauge_type)
                    .collect::<Vec<_>>();
                if filtered.is_empty() {
                    Arc::from(cached.graph.gauge_points.as_slice())
                } else {
                    Arc::from(filtered)
                }
            })
            .clone();
        Some((cached.revision, points))
    }

    pub(super) fn gauge_graph_revision(&self) -> Option<u64> {
        self.gauge_graph.as_ref().map(|cached| cached.revision)
    }

    pub(super) fn cached_gauge_rect_batch(
        &mut self,
        key: ResultGaugeGraphRectBatchCacheKey,
        build: impl FnOnce() -> Arc<[RectCommand]>,
    ) -> Arc<[RectCommand]> {
        if let Some(rects) = self.gauge_rect_batches.get(&key) {
            return Arc::clone(rects);
        }
        let rects = build();
        if self.gauge_rect_batches.len() >= RESULT_RENDER_CACHE_MAX_ENTRIES {
            self.gauge_rect_batches.clear();
        }
        self.gauge_rect_batches.insert(key, Arc::clone(&rects));
        rects
    }
}

#[derive(Debug)]
pub(super) struct ResultGaugeGraphCache {
    graph: Arc<crate::snapshot::ResultGraphSnapshot>,
    revision: u64,
    points_by_type: HashMap<i32, Arc<[crate::snapshot::ResultGaugeGraphPoint]>>,
}

#[derive(Debug, Clone)]
pub(super) struct ResultPlanningCache {
    pub(super) enabled_options: Arc<[i32]>,
    pub(super) destinations: Arc<[ResultDestinationRef]>,
    pub(super) has_nearest_f_diff_rank_destination: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ResultDestinationRef {
    Single { entry_index: usize },
    Conditional { entry_index: usize, destination_index: usize },
}

impl ResultDestinationRef {
    pub(super) fn resolve(self, document: &SkinDocument) -> Option<&SkinDestinationDef> {
        match (self, document.destination.get(self.entry_index())) {
            (
                ResultDestinationRef::Single { .. },
                Some(DestinationListEntry::Single(destination)),
            ) => Some(destination),
            (
                ResultDestinationRef::Conditional { destination_index, .. },
                Some(DestinationListEntry::Conditional { destinations, .. }),
            ) => destinations.get(destination_index),
            _ => None,
        }
    }

    fn entry_index(self) -> usize {
        match self {
            ResultDestinationRef::Single { entry_index }
            | ResultDestinationRef::Conditional { entry_index, .. } => entry_index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ResultRectBatchCacheKey {
    pub(super) destination_index: usize,
    pub(super) kind: ResultRectBatchKind,
    pub(super) frame: ResolvedSkinFrame,
    pub(super) key_mode: KeyMode,
    pub(super) judge_rank: Option<i32>,
    pub(super) visible_len: usize,
    pub(super) data_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum ResultRectBatchKind {
    Judge,
    EarlyLate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ResultGaugeGraphRectBatchCacheKey {
    pub(super) destination_index: usize,
    pub(super) frame: ResolvedSkinFrame,
    pub(super) graph_revision: u64,
    pub(super) display_gauge_type: i32,
    pub(super) gauge_max_bits: u32,
    pub(super) gauge_border_bits: u32,
}

impl Default for SkinContext {
    fn default() -> Self {
        Self {
            manifest: default_skin_manifest(),
            document: None,
            lua_draw_runtime: None,
            document_sources: HashMap::new(),
            select_settings_dest_index: Arc::new(
                crate::select_settings_dest::SelectSettingsDestIndex::default(),
            ),
            result_render_cache: Arc::new(Mutex::new(ResultRenderCache::default())),
        }
    }
}

impl SkinContext {
    pub fn from_manifest(manifest: SkinManifest) -> Self {
        Self {
            manifest,
            document: None,
            lua_draw_runtime: None,
            document_sources: HashMap::new(),
            select_settings_dest_index: Arc::new(
                crate::select_settings_dest::SelectSettingsDestIndex::default(),
            ),
            result_render_cache: Arc::new(Mutex::new(ResultRenderCache::default())),
        }
    }

    pub fn from_manifest_and_document(
        manifest: SkinManifest,
        document: SkinDocument,
        document_sources: impl IntoIterator<Item = SkinDocumentTexture>,
    ) -> Self {
        let select_settings_dest_index =
            Arc::new(crate::select_settings_dest::build_select_settings_dest_index(&document));
        Self {
            manifest,
            document: Some(document),
            lua_draw_runtime: None,
            document_sources: document_sources
                .into_iter()
                .map(|source| (source.source_id.clone(), source))
                .collect(),
            select_settings_dest_index,
            result_render_cache: Arc::new(Mutex::new(ResultRenderCache::default())),
        }
    }

    pub fn manifest(&self) -> &SkinManifest {
        &self.manifest
    }

    pub fn document(&self) -> Option<&SkinDocument> {
        self.document.as_ref()
    }

    pub fn set_lua_draw_runtime(&mut self, runtime: Option<Arc<dyn SkinLuaDrawRuntime>>) {
        self.lua_draw_runtime = runtime;
    }

    fn state_with_lua_runtime(
        &self,
        state: &SkinDrawState,
        text: &SkinTextState<'_>,
    ) -> SkinDrawState {
        let mut state = state.clone();
        let Some(runtime) = self.lua_draw_runtime.as_ref() else {
            return state;
        };
        let enabled_options: Arc<[i32]> = self
            .document
            .as_ref()
            .map(|document| Arc::from(document.enabled_options()))
            .unwrap_or_else(|| Arc::from([]));
        state.lua_runtime = Some(SkinLuaRuntimeContext {
            runtime: Arc::clone(runtime),
            enabled_options,
            text_values: Arc::new(lua_main_state_text_values(&state, text)),
        });
        state
    }

    pub fn set_user_selected_options(&mut self, enabled_options: Vec<i32>) -> bool {
        let Some(document) = &mut self.document else {
            return false;
        };
        document.user_selected_options = Some(enabled_options);
        true
    }

    pub fn with_play_graphs(
        &self,
        judge_graph_density: Vec<u8>,
        bpm_graph_segments: Vec<crate::chart_graph::BpmGraphSegment>,
    ) -> Self {
        let mut cloned = self.clone();
        if let Some(document) = &mut cloned.document {
            document.play_judge_graph_density = judge_graph_density;
            document.play_bpm_graph_segments = bpm_graph_segments;
        }
        cloned
    }

    pub fn with_result_graphs(&self, graph: &crate::snapshot::ResultGraphSnapshot) -> Self {
        let mut cloned = self.clone();
        if let Some(document) = &mut cloned.document {
            document.play_judge_graph_density = graph.judge_graph_density.clone();
            document.play_bpm_graph_segments = graph.bpm_graph_segments.clone();
            document.result_gauge_graph_points = graph.gauge_points.clone();
            document.result_timing_points = graph.timing_points.clone();
            document.result_judge_graph_buckets = graph.judge_graph_buckets.clone();
            document.result_early_late_graph_buckets = graph.early_late_graph_buckets.clone();
            document.result_timing_distribution = graph.timing_distribution.clone();
        }
        cloned
    }

    pub fn static_document_items(&self) -> Vec<SkinRenderItem> {
        self.static_document_items_for_state(&SkinDrawState::default())
    }

    pub fn static_document_items_for_state(&self, state: &SkinDrawState) -> Vec<SkinRenderItem> {
        self.static_document_items_for_state_and_text(state, &SkinTextState::default())
    }

    pub fn static_document_items_for_state_and_text(
        &self,
        state: &SkinDrawState,
        text: &SkinTextState<'_>,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = &self.document else {
            return Vec::new();
        };
        let runtime_sources = static_runtime_document_sources(&self.document_sources, state);
        let state = self.state_with_lua_runtime(state, text);
        document.static_render_items(&runtime_sources, &state, text)
    }

    pub fn static_document_items_for_result_state_and_text(
        &self,
        graph: &Arc<crate::snapshot::ResultGraphSnapshot>,
        state: &SkinDrawState,
        text: &SkinTextState<'_>,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = &self.document else {
            return Vec::new();
        };
        let runtime_sources = static_runtime_document_sources(&self.document_sources, state);
        let state = self.state_with_lua_runtime(state, text);
        // A runtime callback may execute arbitrary bounded Lua. Do not hold the
        // result cache lock across that call.
        if self.lua_draw_runtime.is_none()
            && let Ok(mut cache) = self.result_render_cache.lock()
        {
            cache.prepare_gauge_graph(graph);
            return document.static_render_items_with_graphs_cached(
                &runtime_sources,
                &state,
                text,
                SkinRuntimeGraphs::from_result_graph(graph.as_ref()),
                Some(&mut cache),
            );
        }
        document.static_render_items_with_graphs(
            &runtime_sources,
            &state,
            text,
            SkinRuntimeGraphs::from_result_graph(graph.as_ref()),
        )
    }

    pub fn select_document_items(&self, snapshot: &SelectSnapshot) -> Vec<SkinRenderItem> {
        self.select_document_items_with_dynamic_timers(snapshot, None)
    }

    pub fn select_document_items_with_dynamic_timers(
        &self,
        snapshot: &SelectSnapshot,
        dynamic_timers: Option<&mut DynamicTimerRuntime>,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = &self.document else {
            return Vec::new();
        };
        let runtime_sources = select_runtime_document_sources(&self.document_sources, snapshot);
        document.select_render_items_with_dynamic_timers(
            &runtime_sources,
            snapshot,
            dynamic_timers,
            &self.select_settings_dest_index,
            self.lua_draw_runtime.clone(),
        )
    }

    pub fn select_click_hit(
        &self,
        snapshot: &SelectSnapshot,
        x: f32,
        y: f32,
    ) -> Option<SkinClickHit> {
        let document = self.document.as_ref()?;
        document.select_click_hit(
            &self.document_sources,
            snapshot,
            &self.select_settings_dest_index,
            x,
            y,
        )
    }

    pub fn result_click_hit(&self, state: &SkinDrawState, x: f32, y: f32) -> Option<SkinClickHit> {
        self.document.as_ref()?.result_click_hit(state, x, y)
    }

    pub fn result_slider_hit(
        &self,
        state: &SkinDrawState,
        x: f32,
        y: f32,
    ) -> Option<SkinSliderHit> {
        self.document.as_ref()?.result_slider_hit(state, x, y)
    }

    pub fn select_slider_hit(
        &self,
        snapshot: &SelectSnapshot,
        x: f32,
        y: f32,
    ) -> Option<SkinSliderHit> {
        let document = self.document.as_ref()?;
        document.select_slider_hit(snapshot, &self.select_settings_dest_index, x, y)
    }

    /// 静的 destination を `{"id":"notes"}` マーカーと `timer: 3` (FAILED) で分割して返す。
    /// `.0` はノーツ背面、`.1` はノーツ前面、`.2` は閉店/暗転オーバーレイ（最前面）。
    pub fn static_document_items_split_for_state_and_text(
        &self,
        state: &SkinDrawState,
        text: &SkinTextState<'_>,
    ) -> (Vec<SkinRenderItem>, Vec<SkinRenderItem>, Vec<SkinRenderItem>) {
        let Some(document) = &self.document else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        let runtime_sources = static_runtime_document_sources(&self.document_sources, state);
        let state = self.state_with_lua_runtime(state, text);
        document.static_render_items_split(&runtime_sources, &state, text)
    }

    pub fn static_document_play_items_split_for_state_and_text(
        &self,
        state: &SkinDrawState,
        text: &SkinTextState<'_>,
        play_judge_graph_density: &[u8],
        play_bpm_graph_segments: &[crate::chart_graph::BpmGraphSegment],
    ) -> (Vec<SkinRenderItem>, Vec<SkinRenderItem>, Vec<SkinRenderItem>) {
        let Some(document) = &self.document else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        let runtime_sources = static_runtime_document_sources(&self.document_sources, state);
        let state = self.state_with_lua_runtime(state, text);
        document.static_render_items_split_with_graphs(
            &runtime_sources,
            &state,
            text,
            SkinRuntimeGraphs::from_document_with_play_graphs(
                document,
                play_judge_graph_density,
                play_bpm_graph_segments,
            ),
            None,
        )
    }

    pub fn document_note_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
    ) -> Option<SkinRenderItem> {
        let document = self.document.as_ref()?;
        document.note_image_render_item(lane, key_mode, rect, &self.document_sources)
    }

    pub fn document_ln_start_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
    ) -> Option<SkinRenderItem> {
        let document = self.document.as_ref()?;
        document.note_ln_start_render_item(lane, key_mode, rect, mode, &self.document_sources)
    }

    pub fn document_ln_end_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
    ) -> Option<SkinRenderItem> {
        let document = self.document.as_ref()?;
        document.note_ln_end_render_item(lane, key_mode, rect, mode, &self.document_sources)
    }

    /// ロングノート胴体（`note.lnbody` 系 / `note.hcnbody` 系）を指定矩形に伸縮描画する。
    pub fn document_long_body_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
        state: LongBodyState,
        draw_state: &SkinDrawState,
    ) -> Option<SkinRenderItem> {
        let document = self.document.as_ref()?;
        document.note_long_body_render_item(
            lane,
            key_mode,
            rect,
            mode,
            state,
            draw_state,
            &self.document_sources,
        )
    }

    /// Mine ノート（`note.mine`）を指定矩形に描画する。スキン側に定義が無ければ
    /// `None` を返すため、呼び出し側はデフォルトテクスチャ等のフォールバックへ
    /// 落ちる。
    pub fn document_mine_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
    ) -> Option<SkinRenderItem> {
        let document = self.document.as_ref()?;
        document.note_mine_render_item(lane, key_mode, rect, &self.document_sources)
    }

    pub fn document_note_height(&self, lane: Lane, key_mode: KeyMode) -> Option<f32> {
        let document = self.document.as_ref()?;
        document.note_height_for_lane(lane, key_mode)
    }

    pub fn document_note_expansion_scale(&self, state: &SkinDrawState) -> (f32, f32) {
        let Some(note) = self.document.as_ref().and_then(|document| document.note.as_ref()) else {
            return (1.0, 1.0);
        };
        let elapsed = state.quarter_note_elapsed_ms.unwrap_or(i32::MAX).max(0) as f32;
        let pulse = if elapsed < 9.0 {
            elapsed / 9.0
        } else if elapsed <= 159.0 {
            (159.0 - elapsed) / 150.0
        } else {
            0.0
        };
        let width = note.expansionrate.first().copied().unwrap_or(100) as f32 / 100.0;
        let height = note.expansionrate.get(1).copied().unwrap_or(100) as f32 / 100.0;
        (1.0 + (width - 1.0) * pulse, 1.0 + (height - 1.0) * pulse)
    }

    pub fn document_bar_line_items(
        &self,
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = self.document.as_ref() else {
            return Vec::new();
        };
        let state = self.state_with_lua_runtime(state, &SkinTextState::default());
        document.note_group_render_items(note_y, key_mode, &state, &self.document_sources)
    }

    pub fn document_bpm_line_items(
        &self,
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = self.document.as_ref() else {
            return Vec::new();
        };
        let Some(note) = document.note.as_ref() else {
            return Vec::new();
        };
        let state = self.state_with_lua_runtime(state, &SkinTextState::default());
        document.note_line_render_items(&note.bpm, note_y, key_mode, &state, &self.document_sources)
    }

    pub fn document_stop_line_items(
        &self,
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = self.document.as_ref() else {
            return Vec::new();
        };
        let Some(note) = document.note.as_ref() else {
            return Vec::new();
        };
        let state = self.state_with_lua_runtime(state, &SkinTextState::default());
        document.note_line_render_items(
            &note.stop,
            note_y,
            key_mode,
            &state,
            &self.document_sources,
        )
    }

    pub fn document_time_line_items(
        &self,
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = self.document.as_ref() else {
            return Vec::new();
        };
        let Some(note) = document.note.as_ref() else {
            return Vec::new();
        };
        let state = self.state_with_lua_runtime(state, &SkinTextState::default());
        document.note_line_render_items(
            &note.time,
            note_y,
            key_mode,
            &state,
            &self.document_sources,
        )
    }

    pub fn document_gauge_items(&self, gauge: f32, elapsed_ms: i32) -> Option<Vec<SkinRenderItem>> {
        let document = self.document.as_ref()?;
        document.gauge_render_items(gauge, elapsed_ms, &self.document_sources)
    }

    pub fn timer_animation_duration_ms(&self, timer: i32) -> i32 {
        self.document.as_ref().map_or(0, |document| {
            let enabled_options = document.enabled_options();
            document
                .all_destinations(&enabled_options)
                .into_iter()
                .filter(|destination| destination.timer == Some(timer))
                .filter_map(|destination| {
                    flatten_dst_entries(&destination.dst, &enabled_options)
                        .into_iter()
                        .map(|frame| frame.time.unwrap_or(0))
                        .max()
                })
                .max()
                .unwrap_or(0)
                .max(0)
        })
    }

    pub fn document_judge_items(
        &self,
        judge: &str,
        combo: u32,
        elapsed_ms: i32,
        skin_offsets: &SkinOffsetValues,
        region: usize,
    ) -> Option<Vec<SkinRenderItem>> {
        let document = self.document.as_ref()?;
        let judge_image_index = judge_image_index(judge)?;
        let judge_def = document
            .judge
            .iter()
            .find(|j| j.index == region as i32)
            .or_else(|| document.judge.first())?;
        let state = SkinDrawState { skin_offsets: *skin_offsets, ..SkinDrawState::default() };
        document.judge_render_items_for_def(
            judge_def,
            judge_image_index,
            combo,
            elapsed_ms,
            &self.document_sources,
            &state,
        )
    }

    pub fn apply_play_skin_global_offset(
        &self,
        items: Vec<SkinRenderItem>,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        if self.document.is_none() {
            return items;
        }
        items.into_iter().map(|item| apply_all_offset_to_render_item(item, state)).collect()
    }

    pub fn apply_play_skin_global_offset_to_item(
        &self,
        item: SkinRenderItem,
        state: &SkinDrawState,
    ) -> SkinRenderItem {
        if self.document.is_none() {
            return item;
        }
        apply_all_offset_to_render_item(item, state)
    }

    /// beatoraja スキンの `note.dst` からレーンのノートエリアを取得し、
    /// `note_y`（0.0=判定ライン, 1.0=最上部）に対応するノート矩形を返す。
    /// `note_height` は正規化座標での高さ。ドキュメントスキンが無い場合は `None`。
    pub fn note_rect_for_progress(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        note_y: f32,
        note_height: f32,
        state: &SkinDrawState,
    ) -> Option<Rect> {
        let document = self.document.as_ref()?;
        let enabled_options = document.enabled_options();
        let area = document.note_lane_area(lane, key_mode, &enabled_options)?;
        let canvas_h = document.h.max(1) as f32;
        let bottom_y = note_progress_to_y(area, note_y, state, canvas_h);
        let rect =
            Rect { x: area.x, y: bottom_y - note_height, width: area.width, height: note_height };
        Some(document.apply_notes_offset_to_rect(rect, state))
    }

    pub fn missed_note_rect_for_fall(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        fall: f32,
        note_height: f32,
        state: &SkinDrawState,
    ) -> Option<Rect> {
        let document = self.document.as_ref()?;
        let note = document.note.as_ref()?;
        if note.dst2 == i32::MIN {
            return None;
        }
        let enabled_options = document.enabled_options();
        let area = document.note_lane_area(lane, key_mode, &enabled_options)?;
        let canvas_h = document.h.max(1) as f32;
        let judge_bottom = note_judge_bottom_y(area, state, canvas_h);
        let target_bottom = (canvas_h - note.dst2 as f32) / canvas_h;
        let bottom_y = judge_bottom + (target_bottom - judge_bottom) * fall.clamp(0.0, 1.0);
        let rect =
            Rect { x: area.x, y: bottom_y - note_height, width: area.width, height: note_height };
        Some(document.apply_notes_offset_to_rect(rect, state))
    }

    /// ロングノート胴体の矩形を計算する。`head_y`/`tail_y` は `VisibleNote::y` と同じ
    /// 正規化座標（0.0=判定ライン, 1.0=最奥）。
    pub fn note_body_rect(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        head_y: f32,
        tail_y: f32,
        state: &SkinDrawState,
    ) -> Option<Rect> {
        let document = self.document.as_ref()?;
        let enabled_options = document.enabled_options();
        let area = document.note_lane_area(lane, key_mode, &enabled_options)?;
        let canvas_h = document.h.max(1) as f32;
        let note_height = document.note_height_for_lane(lane, key_mode)?;
        let head_bottom = note_progress_to_y(area, head_y, state, canvas_h);
        let tail_bottom = note_progress_to_y(area, tail_y, state, canvas_h);
        // beatoraja の drawLongNote に合わせる:
        //   body = [dsty+scale, dsty+dy]  (LibGDX y-up)
        //       = [tail_bottom, head_bottom - note_height]  (y-down)
        // 胴体は tail キャップの下端から head キャップの上端まで、キャップと重ならない。
        let top = head_bottom.min(tail_bottom);
        let bottom = head_bottom.max(tail_bottom) - note_height;
        Some(document.apply_notes_offset_to_rect(
            Rect { x: area.x, y: top, width: area.width, height: bottom - top },
            state,
        ))
    }
}

pub(super) fn select_runtime_document_sources(
    base_sources: &HashMap<String, SkinDocumentTexture>,
    snapshot: &SelectSnapshot,
) -> HashMap<String, SkinDocumentTexture> {
    let mut sources = base_sources.clone();
    if snapshot.stage_background
        && let Some(source_size) = snapshot.stage_image_size
    {
        insert_runtime_document_source(&mut sources, "100", SELECT_STAGE_TEXTURE, source_size);
    }
    if snapshot.backbmp_image
        && let Some(source_size) = snapshot.backbmp_image_size
    {
        insert_runtime_document_source(&mut sources, "101", PLAY_BACKBMP_TEXTURE, source_size);
    }
    if snapshot.banner_image
        && let Some(source_size) = snapshot.banner_image_size
    {
        insert_runtime_document_source(&mut sources, "102", SELECT_BANNER_TEXTURE, source_size);
    }
    sources
}

pub(super) fn static_runtime_document_sources(
    base_sources: &HashMap<String, SkinDocumentTexture>,
    state: &SkinDrawState,
) -> HashMap<String, SkinDocumentTexture> {
    let mut sources = base_sources.clone();
    if state.has_stagefile
        && let Some(source_size) = state.stagefile_image_size
    {
        insert_runtime_document_source(&mut sources, "100", SELECT_STAGE_TEXTURE, source_size);
    }
    if state.has_backbmp {
        insert_runtime_document_source(
            &mut sources,
            "101",
            PLAY_BACKBMP_TEXTURE,
            SkinImageSize { width: 1.0, height: 1.0 },
        );
    }
    sources
}

pub(super) fn insert_runtime_document_source(
    sources: &mut HashMap<String, SkinDocumentTexture>,
    source_id: &str,
    texture: TextureId,
    source_size: SkinImageSize,
) {
    sources.insert(
        source_id.to_string(),
        SkinDocumentTexture {
            source_id: source_id.to_string(),
            texture: SkinTextureId(texture.0),
            source_size,
        },
    );
}
