use super::*;

impl SkinPlacement {
    pub(super) fn resolve(&self, elapsed_ms: i32) -> ResolvedPlacement {
        let Some(frame) = self.animation.sample(elapsed_ms) else {
            return ResolvedPlacement { rect: self.rect, alpha: self.alpha, blend: self.blend };
        };

        ResolvedPlacement { rect: frame.rect, alpha: self.alpha * frame.alpha, blend: self.blend }
    }
}

impl Animation {
    pub fn none() -> Self {
        Self { keyframes: Vec::new() }
    }

    fn sample(&self, elapsed_ms: i32) -> Option<Keyframe> {
        self.keyframes
            .iter()
            .filter(|frame| frame.time_ms <= elapsed_ms)
            .max_by_key(|frame| frame.time_ms)
            .copied()
    }
}

impl TextStyle {
    pub(super) fn with_alpha(self, alpha: f32) -> Self {
        Self {
            color: self.color.with_alpha(self.color.a * alpha),
            outline: self.outline.map(|outline| TextOutline {
                color: outline.color.with_alpha(outline.color.a * alpha),
                ..outline
            }),
            shadow: self.shadow.map(|shadow| TextShadow {
                color: shadow.color.with_alpha(shadow.color.a * alpha),
                ..shadow
            }),
            ..self
        }
    }
}

impl Color {
    pub(super) fn with_alpha(self, alpha: f32) -> Self {
        Self { a: alpha.clamp(0.0, 1.0), ..self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedPlacement {
    pub(super) rect: Rect,
    pub(super) alpha: f32,
    pub(super) blend: BlendMode,
}

pub(super) fn format_number(value: i64, digits: u8) -> String {
    if digits == 0 {
        value.to_string()
    } else {
        format!("{:0width$}", value.max(0), width = digits as usize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NumberPadding {
    None,
    Zero,
    Blank,
}

impl NumberPadding {
    pub(super) fn is_zero_padding(self) -> bool {
        matches!(self, Self::Zero)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SignedNumberRowOrder {
    PositiveFirst,
    NegativeFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SignedNumberRender {
    Unsigned,
    Signed(SignedNumberRowOrder),
}

pub(super) fn number_padding(value: &SkinValueDef) -> NumberPadding {
    if value.zeropadding == 2 || value.padding == 2 {
        return NumberPadding::Blank;
    }
    if value.zeropadding != 0 || value.padding != 0 {
        return NumberPadding::Zero;
    }
    let image_cells = value.divx.max(1).saturating_mul(value.divy.max(1));
    if !value_layout_is_signed(value) && !ref_id_is_signed(value.ref_id) && image_cells % 10 != 0 {
        return NumberPadding::Blank;
    }
    NumberPadding::None
}

pub(super) fn signed_value_padding(value: &SkinValueDef, padding: NumberPadding) -> NumberPadding {
    if matches!(value.ref_id, 152 | 153 | 172 | 175 | 178) {
        return NumberPadding::None;
    }
    padding
}

pub(super) fn display_number_digits(
    value: i64,
    max_digits: usize,
    padding: NumberPadding,
) -> Vec<u8> {
    let value = value.saturating_abs();
    let mut text = if padding.is_zero_padding() && max_digits > 0 {
        format!("{value:0width$}", width = max_digits)
    } else {
        value.to_string()
    };
    if max_digits > 0 && text.len() > max_digits {
        text = text[text.len() - max_digits..].to_string();
    }
    let mut digits: Vec<u8> =
        text.bytes().filter(|byte| byte.is_ascii_digit()).map(|byte| byte - b'0').collect();
    if matches!(padding, NumberPadding::Blank) && max_digits > digits.len() {
        let mut padded = vec![10; max_digits - digits.len()];
        padded.extend(digits);
        digits = padded;
    }
    digits
}

/// 符号付き数値（beatoraja の mimage 慣習）用に、divx 列のテクスチャセル index を返す。
///
/// レイアウト (`divx`>=12, `divy`>=2):
/// - 各行は `[0,1,2,3,4,5,6,7,8,9, blank, sign]`
/// - 行0: 正数用 (sign cell = `+`)
/// - 行1: 負数用 (sign cell = `-`)
///
/// 返り値の各 byte は `digit_index % divx` が列、`digit_index / divx` が行になる。
/// 先頭要素は符号セル (index 11)、続けて絶対値の右寄せ桁が並ぶ。
pub(super) fn display_signed_number_digits(
    value: i64,
    max_digits: usize,
    padding: NumberPadding,
    divx: u32,
) -> Vec<u8> {
    display_signed_number_digits_with_row_order(
        value,
        max_digits,
        padding,
        divx,
        SignedNumberRowOrder::PositiveFirst,
    )
}

pub(super) fn display_signed_number_digits_with_row_order(
    value: i64,
    max_digits: usize,
    padding: NumberPadding,
    divx: u32,
    row_order: SignedNumberRowOrder,
) -> Vec<u8> {
    if max_digits == 0 {
        return Vec::new();
    }
    let negative_row = match row_order {
        SignedNumberRowOrder::PositiveFirst => divx as u8,
        SignedNumberRowOrder::NegativeFirst => 0,
    };
    let positive_row = match row_order {
        SignedNumberRowOrder::PositiveFirst => 0,
        SignedNumberRowOrder::NegativeFirst => divx as u8,
    };
    let row_offset = if value < 0 { negative_row } else { positive_row };
    let abs = value.unsigned_abs();
    let abs_text = abs.to_string();
    let numeric_width = max_digits.saturating_sub(1);
    let trimmed = if matches!(padding, NumberPadding::None) {
        if abs_text.len() > max_digits {
            abs_text[abs_text.len() - max_digits..].to_string()
        } else {
            abs_text
        }
    } else if abs_text.len() > numeric_width {
        abs_text[abs_text.len() - numeric_width..].to_string()
    } else {
        abs_text
    };
    let sign_visible = !matches!(padding, NumberPadding::None) || trimmed.len() < max_digits;
    let mut digits = Vec::with_capacity(max_digits);
    if sign_visible {
        // beatoraja の `keta` (`digit`) は符号セルを含む総枠数。
        digits.push(11u8 + row_offset);
    }
    if sign_visible && numeric_width > trimmed.len() {
        let fill = match padding {
            NumberPadding::Zero => 0,
            NumberPadding::Blank => 10,
            NumberPadding::None => u8::MAX,
        };
        if fill != u8::MAX {
            digits.extend(std::iter::repeat_n(fill + row_offset, numeric_width - trimmed.len()));
        }
    }
    for byte in trimmed.bytes() {
        if byte.is_ascii_digit() {
            digits.push((byte - b'0') + row_offset);
        }
    }
    digits
}

/// `ref_id` が符号付き表示を要求する Result 系 ref か。
/// beatoraja の `NUMBER_DIFF_*` 系と次 DJ LEVEL までの差分を対象とする。
pub(super) fn ref_id_is_signed(ref_id: i32) -> bool {
    matches!(ref_id, 152 | 153 | 154 | 172 | 175 | 178)
}

pub(super) fn value_ref_is_signed_for_state(ref_id: i32, state: &SkinDrawState) -> bool {
    ref_id_is_signed(ref_id)
        || (ref_id == 12 && state.select_screen && state.select_option_panel == 3)
}

/// beatoraja `JsonSkinObjectLoader` は value 画像のセル数 (`divx*divy`) が
/// 24 の倍数のとき +側/-側の別 image (mimage) を持つ符号付き数値として扱う。
/// ref に依らず画像レイアウトで決まる (例: Starseeker の ±ms 表示 ref=525, 12x2)。
pub(super) fn value_layout_is_signed(value: &SkinValueDef) -> bool {
    let cells = value.divx.max(1).saturating_mul(value.divy.max(1));
    cells >= 24 && cells % 24 == 0
}

pub(super) fn value_is_signed_for_state(value: &SkinValueDef, state: &SkinDrawState) -> bool {
    value_ref_is_signed_for_state(value.ref_id, state) || value_layout_is_signed(value)
}

pub(super) fn signed_number_render_for_value(
    value: &SkinValueDef,
    state: &SkinDrawState,
) -> SignedNumberRender {
    if value_is_signed_for_state(value, state) {
        SignedNumberRender::Signed(signed_number_row_order_for_value(value, state))
    } else {
        SignedNumberRender::Unsigned
    }
}

pub(super) fn signed_number_row_order_for_value(
    value: &SkinValueDef,
    state: &SkinDrawState,
) -> SignedNumberRowOrder {
    if state.select_screen
        && value.ref_id == 154
        && value.id == "RANK_Diff_Exscore"
        && value.divx >= 12
        && value.divy >= 2
    {
        SignedNumberRowOrder::NegativeFirst
    } else {
        SignedNumberRowOrder::PositiveFirst
    }
}

pub(super) fn lookup_text(values: &[(TextSlot, String)], slot: TextSlot) -> String {
    values
        .iter()
        .find(|(candidate, _)| *candidate == slot)
        .map(|(_, value)| value.clone())
        .unwrap_or_default()
}

pub(super) fn lookup_number(values: &[(NumberSlot, i64)], slot: NumberSlot) -> i64 {
    values
        .iter()
        .find(|(candidate, _)| *candidate == slot)
        .map(|(_, value)| *value)
        .unwrap_or_default()
}

pub(super) fn current_datetime_number(ref_id: i32) -> Option<i64> {
    let seconds =
        SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs().min(i64::MAX as u64) as i64;
    let date = unix_seconds_to_local_datetime(seconds)
        .unwrap_or_else(|| unix_seconds_to_utc_datetime(seconds));
    match ref_id {
        21 => Some(date.year as i64),
        22 => Some(date.month as i64),
        23 => Some(date.day as i64),
        24 => Some(date.hour as i64),
        25 => Some(date.minute as i64),
        26 => Some(date.second as i64),
        _ => None,
    }
}

pub(super) fn skin_judge_region_text(state: &SkinDrawState, region: usize) -> Option<String> {
    if region >= MAX_JUDGE_REGIONS || state.judge_ms[region].is_none() {
        return None;
    }
    judge_index_text(state.judge_index[region]?).map(str::to_string)
}

pub(super) fn skin_judge_timing_text(state: &SkinDrawState, region: usize) -> Option<&'static str> {
    if region >= MAX_JUDGE_REGIONS || state.judge_ms[region].is_none() {
        return None;
    }
    match state.judge_timing_sign[region] {
        Some(1) => Some("FAST"),
        Some(-1) => Some("SLOW"),
        _ => None,
    }
}

pub(super) fn judge_index_text(index: usize) -> Option<&'static str> {
    Some(match index {
        0 => "PGREAT",
        1 => "GREAT",
        2 => "GOOD",
        3 => "BAD",
        4 => "POOR",
        5 => "EMPTY POOR",
        _ => return None,
    })
}

pub(super) fn skin_judge_region_color(
    state: &SkinDrawState,
    region: usize,
    alpha: f32,
) -> Option<Color> {
    if region >= MAX_JUDGE_REGIONS || state.judge_ms[region].is_none() {
        return None;
    }
    Some(match state.judge_index[region]? {
        0 => Color::rgba(112.0 / 255.0, 224.0 / 255.0, 1.0, alpha),
        1 | 2 => Color::rgba(1.0, 224.0 / 255.0, 80.0 / 255.0, alpha),
        3..=5 => Color::rgba(1.0, 88.0 / 255.0, 82.0 / 255.0, alpha),
        _ => return None,
    })
}

pub(super) fn skin_judge_timing_color(
    state: &SkinDrawState,
    region: usize,
    alpha: f32,
) -> Option<Color> {
    if region >= MAX_JUDGE_REGIONS || state.judge_ms[region].is_none() {
        return None;
    }
    Some(match state.judge_timing_sign[region]? {
        1 => Color::rgba(72.0 / 255.0, 176.0 / 255.0, 1.0, alpha),
        -1 => Color::rgba(1.0, 88.0 / 255.0, 82.0 / 255.0, alpha),
        _ => return None,
    })
}

#[cfg(unix)]
pub(super) fn unix_seconds_to_local_datetime(seconds: i64) -> Option<SkinDateTime> {
    let raw_time = seconds as libc::time_t;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `raw_time` and `tm` are valid pointers for the duration of the call.
    // `localtime_r` initializes `tm` on success and returns null on failure.
    let result = unsafe { libc::localtime_r(&raw_time, tm.as_mut_ptr()) };
    if result.is_null() {
        return None;
    }
    // SAFETY: The non-null result means `tm` has been fully initialized.
    let tm = unsafe { tm.assume_init() };
    Some(datetime_from_tm(tm))
}

#[cfg(windows)]
pub(super) fn unix_seconds_to_local_datetime(seconds: i64) -> Option<SkinDateTime> {
    let raw_time = seconds as libc::time_t;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `raw_time` and `tm` are valid pointers for the duration of the call.
    // `localtime_s` initializes `tm` when it returns zero.
    let result = unsafe { libc::localtime_s(tm.as_mut_ptr(), &raw_time) };
    if result != 0 {
        return None;
    }
    // SAFETY: A zero return value means `tm` has been fully initialized.
    let tm = unsafe { tm.assume_init() };
    Some(datetime_from_tm(tm))
}

#[cfg(not(any(unix, windows)))]
pub(super) fn unix_seconds_to_local_datetime(_seconds: i64) -> Option<SkinDateTime> {
    None
}

pub(super) fn datetime_from_tm(tm: libc::tm) -> SkinDateTime {
    SkinDateTime {
        year: tm.tm_year + 1900,
        month: (tm.tm_mon + 1).clamp(1, 12) as u32,
        day: tm.tm_mday.clamp(1, 31) as u32,
        hour: tm.tm_hour.clamp(0, 23) as u32,
        minute: tm.tm_min.clamp(0, 59) as u32,
        second: tm.tm_sec.clamp(0, 59) as u32,
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SkinDateTime {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

pub(super) fn unix_seconds_to_utc_datetime(seconds: i64) -> SkinDateTime {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400) as u32;
    let (year, month, day) = civil_from_days(days);
    SkinDateTime {
        year,
        month,
        day,
        hour: seconds_of_day / 3_600,
        minute: (seconds_of_day % 3_600) / 60,
        second: seconds_of_day % 60,
    }
}

pub(super) fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

pub(super) fn best_rank_op_matches(op: i32, state: &SkinDrawState) -> bool {
    if state.in_settings {
        return false;
    }
    let Some(rank) = rank_index(result_mybest_ex_score(state), state.total_notes) else {
        return false;
    };
    op == 320 + rank as i32
}

/// 現在のランク判定の基準値 (ex_score, notes)。
/// Play 画面では beatoraja の `qualifyNowRank` と同じく past notes を分母にする。
pub(super) fn current_rank_inputs(state: &SkinDrawState) -> (Option<u32>, u32) {
    if state.result_failed.is_some() {
        (Some(state.ex_score), state.total_notes)
    } else if state.select_screen {
        (state.select_ex_score, state.select_total_notes)
    } else if let Some(notes) = current_score_rate_notes(state) {
        (Some(state.ex_score), notes)
    } else {
        (Some(state.ex_score), state.total_notes)
    }
}

pub(super) fn current_rank_index(state: &SkinDrawState) -> Option<usize> {
    let (ex_score, total_notes) = current_rank_inputs(state);
    if !state.select_screen
        && state.result_failed.is_none()
        && total_notes == 0
        && current_score_rate_notes(state) == Some(0)
    {
        return rank_index(Some(2), 1);
    }
    rank_index(ex_score, total_notes)
}

pub(super) fn rank_index(ex_score: Option<u32>, total_notes: u32) -> Option<usize> {
    let ex_score = ex_score?;
    let max_score = total_notes.saturating_mul(2);
    if max_score == 0 {
        return None;
    }
    let score = ex_score.min(max_score) as u64;
    let max = max_score as u64;
    let rank = if score * 9 >= max * 8 {
        0
    } else if score * 9 >= max * 7 {
        1
    } else if score * 9 >= max * 6 {
        2
    } else if score * 9 >= max * 5 {
        3
    } else if score * 9 >= max * 4 {
        4
    } else if score * 9 >= max * 3 {
        5
    } else if score * 9 >= max * 2 {
        6
    } else {
        7
    };
    Some(rank)
}

pub fn select_arrange_index(arrange: &str) -> usize {
    match arrange {
        "MIRROR" => 1,
        "RANDOM" | "F-RANDOM" | "MF-RANDOM" => 2,
        "R-RANDOM" => 3,
        "S-RANDOM" => 4,
        "SPIRAL" => 5,
        "H-RANDOM" => 6,
        "ALL-SCR" => 7,
        "RANDOM-EX" => 8,
        "S-RANDOM-EX" => 9,
        _ => 0,
    }
}

pub fn extended_arrange_index(arrange: &str) -> usize {
    match arrange {
        "F-RANDOM" => 10,
        "MF-RANDOM" => 11,
        _ => select_arrange_index(arrange),
    }
}

pub fn select_double_option_index(double_option: &str) -> usize {
    match double_option {
        "FLIP" => 1,
        "BATTLE" => 2,
        "BATTLE AS" => 3,
        _ => 0,
    }
}

pub(super) fn select_hs_fix_index(hs_fix: &str) -> usize {
    match hs_fix {
        "START BPM" => 1,
        "MAX BPM" => 2,
        "MAIN BPM" => 3,
        "MIN BPM" => 4,
        _ => 0,
    }
}

pub(crate) fn random_lane_refs(
    pattern: &[u8],
    key_mode: KeyMode,
) -> [u8; SKIN_RANDOM_LANE_REF_COUNT] {
    let mut refs = [0; SKIN_RANDOM_LANE_REF_COUNT];
    if pattern.is_empty() {
        return refs;
    }

    let mut p1_slot = 0;
    let mut p2_slot = 0;
    for &lane in key_mode.active_lanes() {
        if is_p1_random_key_lane(key_mode, lane) {
            if p1_slot < 9 {
                refs[p1_slot] = random_lane_display_value(pattern, lane, key_mode, false);
            }
            p1_slot += 1;
        } else if is_p2_random_key_lane(lane) {
            if p2_slot < 9 {
                refs[10 + p2_slot] = random_lane_display_value(pattern, lane, key_mode, true);
            }
            p2_slot += 1;
        }
    }

    if key_mode.active_lanes().contains(&Lane::Scratch) {
        refs[9] = random_lane_display_value(pattern, Lane::Scratch, key_mode, false);
    }
    if key_mode.active_lanes().contains(&Lane::Scratch2) {
        refs[19] = random_lane_display_value(pattern, Lane::Scratch2, key_mode, true);
    }

    refs
}

pub(crate) fn fixed_random_lane_refs(
    pattern: &[u8],
    key_mode: KeyMode,
    arrange: &str,
    arrange_2p: &str,
) -> [u8; SKIN_RANDOM_LANE_REF_COUNT] {
    let mut refs = random_lane_refs(pattern, key_mode);
    let arrange_index = select_arrange_index(arrange);
    let arrange_2p_index = select_arrange_index(arrange_2p);
    for (slot, value) in refs.iter_mut().enumerate() {
        let side_arrange_index = if slot < 10 { arrange_index } else { arrange_2p_index };
        let scratch_ref = matches!(slot, 9 | 19);
        let displayable_arrange = if scratch_ref {
            side_arrange_index == 8
        } else {
            matches!(side_arrange_index, 2 | 3 | 8)
        };
        if !displayable_arrange {
            *value = 0;
        }
    }
    refs
}

pub(super) fn random_lane_display_value(
    pattern: &[u8],
    display_lane: Lane,
    key_mode: KeyMode,
    is_2p_side: bool,
) -> u8 {
    let Some(source) = pattern.get(display_lane.index()).copied().map(usize::from) else {
        return 0;
    };
    if source >= LANE_COUNT {
        return 0;
    }
    if is_2p_side {
        p2_random_lane_number(source, key_mode)
    } else {
        p1_random_lane_number(source, key_mode)
    }
}

pub(super) fn is_p1_random_key_lane(key_mode: KeyMode, lane: Lane) -> bool {
    matches!(
        lane,
        Lane::Key1 | Lane::Key2 | Lane::Key3 | Lane::Key4 | Lane::Key5 | Lane::Key6 | Lane::Key7
    ) || (key_mode == KeyMode::K9 && matches!(lane, Lane::Key8 | Lane::Key9))
}

pub(super) fn is_p2_random_key_lane(lane: Lane) -> bool {
    matches!(
        lane,
        Lane::Key8
            | Lane::Key9
            | Lane::Key10
            | Lane::Key11
            | Lane::Key12
            | Lane::Key13
            | Lane::Key14
    )
}

pub(super) fn p1_random_lane_number(source: usize, key_mode: KeyMode) -> u8 {
    match Lane::ALL[source] {
        Lane::Scratch => p1_random_side_key_count(key_mode),
        Lane::Key1 => 1,
        Lane::Key2 => 2,
        Lane::Key3 => 3,
        Lane::Key4 => 4,
        Lane::Key5 => 5,
        Lane::Key6 => 6,
        Lane::Key7 => 7,
        Lane::Key8 if key_mode == KeyMode::K9 => 8,
        Lane::Key9 if key_mode == KeyMode::K9 => 9,
        _ => 0,
    }
}

pub(super) fn p2_random_lane_number(source: usize, key_mode: KeyMode) -> u8 {
    match Lane::ALL[source] {
        Lane::Key8 => 1,
        Lane::Key9 => 2,
        Lane::Key10 => 3,
        Lane::Key11 => 4,
        Lane::Key12 => 5,
        Lane::Key13 => 6,
        Lane::Key14 => 7,
        Lane::Scratch2 => p2_random_side_key_count(key_mode),
        _ => 0,
    }
}

pub(super) fn p1_random_side_key_count(key_mode: KeyMode) -> u8 {
    match key_mode {
        KeyMode::K4 => 4,
        KeyMode::K5 => 6,
        KeyMode::K6 => 6,
        KeyMode::K7 | KeyMode::K8 | KeyMode::K14 => 8,
        KeyMode::K9 => 9,
        KeyMode::K10 => 6,
    }
}

pub(super) fn p2_random_side_key_count(key_mode: KeyMode) -> u8 {
    match key_mode {
        KeyMode::K10 => 6,
        KeyMode::K14 => 8,
        _ => 0,
    }
}

pub(super) fn select_gauge_index(gauge: &str) -> usize {
    match gauge {
        "A-EASY" => 0,
        "EASY" => 1,
        "NORMAL" => 2,
        "HARD" => 3,
        "EX-HARD" => 4,
        "HAZARD" => 5,
        _ => 2,
    }
}

pub(super) fn select_gauge_auto_shift_index(mode: &str) -> usize {
    match mode {
        "CONTINUE" => 1,
        "HARD TO GROOVE" => 2,
        "BEST CLEAR" => 3,
        "SELECT TO UNDER" => 4,
        _ => 0,
    }
}

pub(super) fn select_bottom_shiftable_gauge_index(mode: &str) -> usize {
    match mode {
        "EASY" => 1,
        "NORMAL" => 2,
        _ => 0,
    }
}

/// beatoraja の既定 target list と、play skin の target graph (ref 41/77) が
/// 使う 11 段階の画像 index。選曲画面用の BMZ target 列挙順とは別物。
pub(crate) fn play_target_image_index(target: &str) -> usize {
    match target {
        "RANK_A" | "A" => 1,
        "RANK_AA-" => 3,
        "RANK_AA" | "AA" => 4,
        "RANK_AAA-" => 6,
        "RANK_AAA" | "AAA" => 7,
        "RANK_MAX-" => 9,
        "MAX" => 10,
        // BMZ 固有の動的 target は専用 sprite を持たないため、先頭へ
        // fallback する（従来の NONE と同じ扱い）。
        _ => 0,
    }
}

pub(super) fn select_bga_index(bga: &str) -> usize {
    match bga {
        "AUTO" => 1,
        "OFF" => 2,
        _ => 0,
    }
}

pub(super) fn select_assist_index(assist: &str) -> usize {
    match assist {
        "AUTOPLAY" | "AUTOPLAY BATTLE" => 1,
        _ => 0,
    }
}

pub(super) fn select_mode_index(mode: &str) -> usize {
    match mode {
        "5K" => 1,
        "7K" => 2,
        "10K" => 3,
        "14K" => 4,
        "9K" => 5,
        "24K" => 6,
        "24K_DOUBLE" => 7,
        _ => 0,
    }
}

pub(super) fn select_sort_index(sort: &str) -> usize {
    match sort {
        "ARTIST" => 1,
        "BPM" => 2,
        "LENGTH" => 3,
        "LEVEL" => 4,
        "CLEAR" => 5,
        "SCORE" => 6,
        "BPCOUNT" => 7,
        _ => 0,
    }
}

pub(super) fn select_ln_mode_index(mode: &str) -> usize {
    match mode {
        "CN" | "AUTO(CN)" | "FORCE(CN)" => 1,
        "HCN" | "AUTO(HCN)" | "FORCE(HCN)" => 2,
        _ => 0,
    }
}

pub(super) fn select_judge_algorithm_index(algorithm: &str) -> usize {
    match algorithm {
        "Duration" | "DURATION" => 1,
        "Lowest" | "LOWEST" => 2,
        _ => 0,
    }
}

pub(super) fn select_scroll_progress(snapshot: &SelectSnapshot) -> f32 {
    if snapshot.chart_count <= 1 {
        return 0.0;
    }
    snapshot.selected_index.min(snapshot.chart_count - 1) as f32 / (snapshot.chart_count - 1) as f32
}

pub(super) fn select_snapshot_selected_row_position(
    rows: &[SelectRowSnapshot],
    selected_index: u32,
) -> usize {
    let center = rows.len() / 2;
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.index == selected_index)
        .min_by_key(|(index, _)| index.abs_diff(center))
        .map(|(index, _)| index)
        .unwrap_or(0)
}
