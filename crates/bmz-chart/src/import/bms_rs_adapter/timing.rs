use super::*;
use crate::timing::{IMPORT_TICK_SCALE, TICKS_PER_MEASURE};

pub(super) fn track_of(time: ObjTime) -> u32 {
    u32::try_from(time.track().0).unwrap_or(u32::MAX)
}

pub(super) fn compute_max_measure(
    bms: &Bms,
    objects: &[IntermediateObject],
) -> Result<u32, ImportError> {
    let mut max = objects.iter().map(|o| o.measure).max().unwrap_or(0);
    if let Some(last) = bms.last_obj_time() {
        max = max.max(track_of(last));
    }
    for &track in bms.section_len.section_len_changes.keys() {
        max = max.max(u32::try_from(track.0).unwrap_or(u32::MAX));
    }
    if max > MAX_SUPPORTED_MEASURE {
        return Err(ImportError::InvalidChart {
            message: format!(
                "chart has measure {max}, exceeding supported maximum {MAX_SUPPORTED_MEASURE}"
            ),
        });
    }
    Ok(max)
}

pub(super) fn build_measures(max_measure: u32, bms: &Bms) -> Vec<MeasureInfo> {
    let mut measures = Vec::with_capacity(max_measure as usize + 1);
    let mut start_tick = 0_u64;
    for index in 0..=max_measure {
        let length = bms
            .section_len
            .section_len_changes
            .get(&bms_rs::bms::command::time::Track(index as u64))
            .map_or(1.0, |change| sanitize_measure_length(change.length.get()));
        let (num, den) = fin_f64_to_ratio(length);
        let tick_len = scaled_measure_ticks(length);
        measures.push(MeasureInfo {
            index,
            length,
            length_ratio_num: num,
            length_ratio_den: den.max(1),
            start_tick: ChartTick(start_tick),
            tick_len,
        });
        start_tick = start_tick.saturating_add(tick_len);
    }
    measures
}

fn sanitize_measure_length(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 { value } else { 1.0 }
}

fn scaled_measure_ticks(length: f64) -> u64 {
    let ticks = TICKS_PER_MEASURE as f64 * length * IMPORT_TICK_SCALE as f64;
    ticks.round().max(1.0) as u64
}

pub(super) fn fin_f64_to_ratio(value: f64) -> (u32, u32) {
    if !value.is_finite() || value <= 0.0 {
        return (1, 1);
    }
    let den = 1_000_000_u32;
    let num = (value * den as f64).round() as u32;
    let gcd = gcd(num.max(1), den);
    (num.max(1) / gcd, den / gcd)
}

pub(super) fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

pub(super) fn map_lane(layout: ChartKeyLayout, side: PlayerSide, key: Key) -> Option<Lane> {
    match layout {
        ChartKeyLayout::Beat(_) => map_lane_beat(side, key),
        ChartKeyLayout::Pms(_) => map_lane_pms(side, key),
    }
}

pub(super) fn map_lane_beat(side: PlayerSide, key: Key) -> Option<Lane> {
    match (side, key) {
        (PlayerSide::Player1, Key::Key(1)) => Some(Lane::Key1),
        (PlayerSide::Player1, Key::Key(2)) => Some(Lane::Key2),
        (PlayerSide::Player1, Key::Key(3)) => Some(Lane::Key3),
        (PlayerSide::Player1, Key::Key(4)) => Some(Lane::Key4),
        (PlayerSide::Player1, Key::Key(5)) => Some(Lane::Key5),
        (PlayerSide::Player1, Key::Key(6)) => Some(Lane::Key6),
        (PlayerSide::Player1, Key::Key(7)) => Some(Lane::Key7),
        (PlayerSide::Player1, Key::Scratch(_)) => Some(Lane::Scratch),
        (PlayerSide::Player2, Key::Key(1)) => Some(Lane::Key8),
        (PlayerSide::Player2, Key::Key(2)) => Some(Lane::Key9),
        (PlayerSide::Player2, Key::Key(3)) => Some(Lane::Key10),
        (PlayerSide::Player2, Key::Key(4)) => Some(Lane::Key11),
        (PlayerSide::Player2, Key::Key(5)) => Some(Lane::Key12),
        (PlayerSide::Player2, Key::Key(6)) => Some(Lane::Key13),
        (PlayerSide::Player2, Key::Key(7)) => Some(Lane::Key14),
        (PlayerSide::Player2, Key::Scratch(_)) => Some(Lane::Scratch2),
        _ => None,
    }
}

pub(super) fn map_lane_pms(side: PlayerSide, key: Key) -> Option<Lane> {
    match (side, key) {
        (PlayerSide::Player1, Key::Key(n)) => Lane::from_pms_key(n),
        _ => None,
    }
}

pub(super) fn map_bms_warning(w: &BmsWarning) -> Option<ImportWarning> {
    use bms_rs::bms::parse::ParseWarning;
    use bms_rs::bms::parse::check_playing::{PlayingError, PlayingWarning};

    let (code, message) = match w {
        BmsWarning::Lex(inner) => ("LexWarning", format!("{}", inner.content())),
        BmsWarning::Parse(inner) => {
            let code = match inner.content() {
                ParseWarning::SyntaxError(_) => "ParseSyntaxError",
                ParseWarning::UndefinedObject(_) => "ParseUndefinedObject",
                ParseWarning::DuplicatingDef(_) => "ParseDuplicatingDef",
                ParseWarning::DuplicatingTrackObj(_, _) => "ParseDuplicatingTrackObj",
                ParseWarning::DuplicatingChannelObj(
                    _,
                    Channel::BgaBase | Channel::BgaPoor | Channel::BgaLayer | Channel::BgaLayer2,
                ) => return None,
                ParseWarning::DuplicatingChannelObj(_, _) => "ParseDuplicatingChannelObj",
                ParseWarning::OutOfBase62 => "ParseOutOfBase62",
            };
            (code, format!("{}", inner.content()))
        }
        BmsWarning::PlayingWarning(w) => {
            let code = match w {
                PlayingWarning::TotalUndefined => "PlayingTotalUndefined",
                PlayingWarning::NoDisplayableNotes => "PlayingNoDisplayableNotes",
                PlayingWarning::NoPlayableNotes => "PlayingNoPlayableNotes",
                PlayingWarning::StartBpmUndefined => "PlayingStartBpmUndefined",
                _ => "PlayingWarningOther",
            };
            (code, format!("{w}"))
        }
        BmsWarning::PlayingError(e) => {
            let code = match e {
                PlayingError::InvalidBpm { .. } => "PlayingInvalidBpm",
                PlayingError::InvalidStop { .. } => "PlayingInvalidStop",
                PlayingError::InvalidSpeed { .. } => "PlayingInvalidSpeed",
                PlayingError::InvalidScroll { .. } => "PlayingInvalidScroll",
                PlayingError::InvalidSeek { .. } => "PlayingInvalidSeek",
                _ => "PlayingErrorOther",
            };
            (code, format!("{e}"))
        }
        _ => ("BmsWarningOther", format!("{w:?}")),
    };
    Some(ImportWarning::ParserDiagnostic { code: code.to_string(), message })
}
