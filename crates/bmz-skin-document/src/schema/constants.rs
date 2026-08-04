use super::*;

/// Skin value / slider 用の BMZ 組み込み式キー。
pub const SKIN_EXPR_ADJUSTED_COVER: &str = "bmz:adjusted_cover";
pub const SKIN_EXPR_ADJUSTED_RATE: &str = "bmz:adjusted_rate";
pub const SKIN_EXPR_ADJUSTED_RATE_ADOT: &str = "bmz:adjusted_rate_adot";
pub const SKIN_EXPR_FS_THRESHOLD: &str = "bmz:fs_threshold";
pub const SKIN_EXPR_COURSE_TABLE_TEXT: &str = "bmz:course_table_text";
pub const SKIN_EXPR_RESULT_TABLE_TITLE: &str = "bmz:result_table_title";
pub const SKIN_EXPR_DIFFICULTY_NAME: &str = "bmz:difficulty_name";
pub const SKIN_EXPR_FAST_SLOW_BREAKDOWN_HEIGHT: &str = "bmz:fast_slow_breakdown_height";
pub const SKIN_EXPR_DEFAULT_CHART_TOTAL_COUNT: &str = "bmz:default_chart_total_count";
pub const SKIN_EXPR_DEFAULT_CHART_GAUGE: &str = "bmz:default_chart_gauge";
pub const SKIN_EXPR_COURSE_CLEAR_RATE: &str = "bmz:course_clear_rate";
pub const SKIN_EXPR_GAUGE_PERCENT_INTEGER: &str = "bmz:gauge_percent_integer";
pub const SKIN_EXPR_GAUGE_PERCENT_FRACTION: &str = "bmz:gauge_percent_fraction";
pub const SKIN_EXPR_GAUGE_AMOUNT_INTEGER: &str = "bmz:gauge_amount_integer";
pub const SKIN_EXPR_GAUGE_AMOUNT_FRACTION: &str = "bmz:gauge_amount_fraction";

/// beatoraja 予約 ID と衝突しない動的タイマー ID 範囲の先頭。
pub const SKIN_DYNAMIC_TIMER_BASE: i32 = 9000;
/// Play 中 imageset が `main_state.gauge_type()` で選ぶ ref (beatoraja 非予約)。
pub const SKIN_REF_PLAY_GAUGE_TYPE: i32 = 44;
/// beatoraja `BUTTON_HSFIX` (`event_index(55)`)。
pub const SKIN_EVENT_HSFIX: i32 = 55;
/// BMZ extension: exact key mode number (`4`, `5`, `6`, `7`, `8`, `9`, `10`, `14`).
pub const SKIN_REF_BMZ_KEY_MODE: i32 = 1903;
/// BMZ extension: active physical lane count including scratch lanes.
pub const SKIN_REF_BMZ_ACTIVE_LANE_COUNT: i32 = 1904;
/// BMZ extension: exact key mode options in K4/K5/K6/K7/K8/K9/K10/K14 order.
pub const SKIN_OPTION_BMZ_KEY_MODE_BASE: i32 = 1905;
pub const SKIN_OPTION_BMZ_KEY_MODE_COUNT: usize = 8;
pub const SKIN_OPTION_BMZ_KEY_MODE_LAST: i32 = 1912;
/// BMZ extension: scratch layout options.
pub const SKIN_OPTION_BMZ_NO_SCRATCH: i32 = 1913;
pub const SKIN_OPTION_BMZ_SINGLE_PLAY: i32 = 1914;
pub const SKIN_OPTION_BMZ_DOUBLE_PLAY: i32 = 1915;
/// BMZ extension: E1/E2/E3/E4/UI Left/Right/Up/Down held options.
pub const SKIN_OPTION_BMZ_INPUT_BASE: i32 = 1920;
pub const SKIN_OPTION_BMZ_INPUT_LAST: i32 = 1927;
pub const SKIN_BMZ_INPUT_COUNT: usize = 8;
/// BMZ extension: matching press-edge timers.
pub const SKIN_TIMER_BMZ_INPUT_BASE: i32 = 19_000;
pub const SKIN_TIMER_BMZ_INPUT_LAST: i32 = 19_007;
/// BMZ extension: generic daily statistics number refs.
pub const SKIN_REF_BMZ_DAILY_BASE: i32 = 1930;
pub const SKIN_REF_BMZ_DAILY_LAST: i32 = 1946;
/// BMZ extension: daily rank label and recent title text refs.
pub const SKIN_TEXT_BMZ_DAILY_RANK: i32 = 1943;
pub const SKIN_TEXT_BMZ_DAILY_RECENT_BASE: i32 = 1950;
pub const SKIN_TEXT_BMZ_DAILY_RECENT_LAST: i32 = 1959;
/// BMZ extension: select settings row kind (`0=other`, `1=folder`, `2=back`, `3=close`).
pub const SKIN_REF_BMZ_SELECT_SETTINGS_ROW_KIND: i32 = 1960;
/// BMZ extension: select settings folder/back/close row options.
pub const SKIN_OPTION_BMZ_SETTINGS_FOLDER: i32 = 1961;
pub const SKIN_OPTION_BMZ_SETTINGS_BACK: i32 = 1962;
pub const SKIN_OPTION_BMZ_SETTINGS_CLOSE: i32 = 1963;
/// BMZ extension: IR scope index and label (`0=Ranking`, `1=Rival`).
pub const SKIN_REF_BMZ_IR_SCOPE: i32 = 1964;
/// BMZ extension: IR scope selected options.
pub const SKIN_OPTION_BMZ_IR_SCOPE_GLOBAL: i32 = 1965;
pub const SKIN_OPTION_BMZ_IR_SCOPE_RIVAL: i32 = 1966;
/// BMZ extension: IR scope availability options.
pub const SKIN_OPTION_BMZ_IR_SCOPE_GLOBAL_SUPPORTED: i32 = 1967;
pub const SKIN_OPTION_BMZ_IR_SCOPE_RIVAL_SUPPORTED: i32 = 1968;
/// BMZ extension: number of players in the displayed IR scope.
pub const SKIN_REF_BMZ_IR_SCOPE_TOTAL: i32 = 1969;
/// BMZ extension: select session mode (`0=NORMAL`, `1=AUTOPLAY`,
/// `2=AUTO BATTLE`, `3=BATTLE`).
pub const SKIN_REF_BMZ_SELECT_SESSION_MODE: i32 = 1970;
/// Backward-compatible Rust aliases for the initial Result-only names.
pub const SKIN_REF_BMZ_RESULT_IR_SCOPE: i32 = SKIN_REF_BMZ_IR_SCOPE;
pub const SKIN_OPTION_BMZ_RESULT_IR_SCOPE_GLOBAL: i32 = SKIN_OPTION_BMZ_IR_SCOPE_GLOBAL;
pub const SKIN_OPTION_BMZ_RESULT_IR_SCOPE_RIVAL: i32 = SKIN_OPTION_BMZ_IR_SCOPE_RIVAL;
pub const SKIN_OPTION_BMZ_RESULT_IR_SCOPE_GLOBAL_SUPPORTED: i32 =
    SKIN_OPTION_BMZ_IR_SCOPE_GLOBAL_SUPPORTED;
pub const SKIN_OPTION_BMZ_RESULT_IR_SCOPE_RIVAL_SUPPORTED: i32 =
    SKIN_OPTION_BMZ_IR_SCOPE_RIVAL_SUPPORTED;
pub const SKIN_REF_BMZ_RESULT_IR_SCOPE_TOTAL: i32 = SKIN_REF_BMZ_IR_SCOPE_TOTAL;
/// BMZ extension: course result stage count and ten stage slots.
pub const SKIN_REF_BMZ_COURSE_STAGE_COUNT: i32 = 19_100;
pub const SKIN_REF_BMZ_COURSE_STAGE_EX_BASE: i32 = 19_110;
pub const SKIN_REF_BMZ_COURSE_STAGE_GAUGE_BASE: i32 = 19_120;
pub const SKIN_REF_BMZ_COURSE_STAGE_BP_BASE: i32 = 19_130;
pub const SKIN_REF_BMZ_COURSE_STAGE_RATE_BASE: i32 = 19_140;
pub const SKIN_BMZ_COURSE_STAGE_COUNT: usize = 10;
/// Lua result skin の定数 `Expand_op` 代入を宣言的クリックイベントへ変換する ID。
/// beatoraja の正数イベント ID と衝突しない BMZ 内部予約値を使う。
pub const SKIN_EVENT_RESULT_PANEL_IR: i32 = -10_001;
pub const SKIN_EVENT_RESULT_PANEL_GRAPH: i32 = -10_002;
/// BMZ IR scope selection events for compatible skins.
pub const SKIN_EVENT_IR_SCOPE_GLOBAL: i32 = -10_003;
pub const SKIN_EVENT_IR_SCOPE_RIVAL: i32 = -10_004;
pub const SKIN_EVENT_IR_SCOPE_TOGGLE: i32 = -10_005;
/// Backward-compatible Rust aliases for the initial Result-only names.
pub const SKIN_EVENT_RESULT_IR_SCOPE_GLOBAL: i32 = SKIN_EVENT_IR_SCOPE_GLOBAL;
pub const SKIN_EVENT_RESULT_IR_SCOPE_RIVAL: i32 = SKIN_EVENT_IR_SCOPE_RIVAL;
pub const SKIN_EVENT_RESULT_IR_SCOPE_TOGGLE: i32 = SKIN_EVENT_IR_SCOPE_TOGGLE;
/// Clear the visible daily statistics window without deleting score history.
pub const SKIN_EVENT_DAILY_STATISTICS_RESET: i32 = -10_100;
/// Lua callback から変換する runtime event の内部予約 ID 範囲。
/// beatoraja 正数イベント ID と衝突しないよう負数を使う。
pub const SKIN_EVENT_RUNTIME_BASE: i32 = -20_000;
/// beatoraja `NUMBER_RANDOM_1P_1KEY..NUMBER_RANDOM_2P_SCR` (450..469).
/// BMZではResult互換に加え、Play/Selectの確定済み固定配置にも使用する。
pub const SKIN_RANDOM_LANE_REF_BASE: i32 = 450;
pub const SKIN_RANDOM_LANE_REF_COUNT: usize = 20;
/// `SkinDrawState::dynamic_timer_ms` のスロット数。
pub const SKIN_DYNAMIC_TIMER_COUNT: usize = 64;

/// Which IR ranking supplies standard IR refs for this skin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrScopeBinding {
    /// Preserve beatoraja-compatible global ranking behavior.
    #[default]
    Global,
    /// Bind standard IR refs to the scope currently selected by the player.
    Active,
}

/// Backward-compatible type alias for the initial Result-only name.
pub type ResultIrScopeBinding = IrScopeBinding;

/// Optional Result IR scope switch input declared by a BMZ-compatible skin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultIrScopeToggle {
    #[default]
    None,
    E1Press,
}

/// Optional Select IR scope switch input declared by a BMZ-compatible skin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectIrScopeToggle {
    #[default]
    None,
    E3Press,
}

pub fn string_array_refs(values: &[String; 10]) -> [&str; 10] {
    std::array::from_fn(|index| values[index].as_str())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SkinDynamicTimerDef {
    pub id: i32,
    pub observe: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SkinFixedDelayTimerDef {
    pub id: i32,
    #[serde(rename = "sourceTimer")]
    pub source_timer: i32,
    #[serde(rename = "delayMs")]
    pub delay_ms: i32,
}

/// 描画ランタイムで保持する bool フラグの初期値。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SkinRuntimeFlagDef {
    pub id: i32,
    #[serde(default)]
    pub initial: bool,
}

/// event ID を受けて複数の runtime flag を反転する宣言的イベント。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SkinRuntimeEventDef {
    pub id: i32,
    #[serde(default, rename = "toggleFlags")]
    pub toggle_flags: Vec<i32>,
    /// BMZ extension: logical input press edge that dispatches this runtime event.
    #[serde(default, rename = "triggerAction")]
    pub trigger_action: Option<SkinRuntimeTriggerAction>,
}

/// Runtime event trigger that does not require Lua-side input or file access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkinRuntimeTriggerAction {
    E1Press,
    E2Press,
    E3Press,
    E4Press,
    UiLeftPress,
    UiRightPress,
    UiUpPress,
    UiDownPress,
}

impl SkinRuntimeTriggerAction {
    pub const fn index(self) -> usize {
        match self {
            Self::E1Press => 0,
            Self::E2Press => 1,
            Self::E3Press => 2,
            Self::E4Press => 3,
            Self::UiLeftPress => 4,
            Self::UiRightPress => 5,
            Self::UiUpPress => 6,
            Self::UiDownPress => 7,
        }
    }
}

/// スキン音声に対する宣言的な再生・停止命令。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SkinAudioActionDef {
    pub action: SkinAudioActionKind,
    pub path: String,
    #[serde(default = "default_skin_audio_volume")]
    pub volume: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkinAudioActionKind {
    Play,
    Loop,
    Stop,
}

/// 条件が単一 timer の ON へ落とせる Lua `customEvents` 定義。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SkinCustomEventDef {
    pub id: i32,
    #[serde(default)]
    pub timer: i32,
    #[serde(default)]
    pub once: bool,
    #[serde(default, rename = "audioActions")]
    pub audio_actions: Vec<SkinAudioActionDef>,
}

fn default_skin_audio_volume() -> f32 {
    1.0
}
