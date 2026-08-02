/// HCN passing 中レーンの表示タイマー状態。
use super::judgement::update_gauge_increase_timer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HcnLaneTimer {
    /// 押下中 (回復中) なら true、離している (減衰中) なら false。
    pub inclease: bool,
    /// 現在の inclease 状態が始まった時刻。タイマー経過時間の起点。
    pub since: TimeUs,
    /// beatoraja `mpassingcount`: inclease 中は加算、離している間は減算する
    /// 符号付き経過時間 (us)。±200ms を超えるたびにゲージを 1 tick 更新する。
    /// inclease の反転ではリセットせず (相殺される)、passing 終了でリセットする。
    pub passing_count_us: i64,
}

/// beatoraja の TIMER_HCN_ACTIVE / TIMER_HCN_DAMAGE 切り替えと
/// HCN キー音の音量制御に対応する。
/// 「HCN の始端〜終端の区間内 (passing) かつ始端ノート判定済み」のレーンで、
/// inclease（押下中、または終端を PG/GR/GD で判定済み）に応じて
/// active/damage を切り替える。状態が反転したらタイマー起点 (`since`) を
/// リセットする。
/// キー音は beatoraja 同様、終端が BAD 以下で判定済み（早離し）の passing HCN
/// に限り、離している間は音量 0、押し直したら元の音量に戻す。
pub fn update_hcn_lane_timers(session: &mut GameSession, audio_now: TimeUs) {
    let mut next: [Option<HcnLaneTimer>; LANE_COUNT] = [None; LANE_COUNT];
    let mut next_muted: [Option<bool>; LANE_COUNT] = [None; LANE_COUNT];
    for pair in &session.chart.long_notes {
        let mode = pair.mode.unwrap_or(session.chart.metadata.long_note_mode);
        if mode != LongNoteMode::Hcn
            || audio_now.0 < pair.start_time.0
            || audio_now.0 >= pair.end_time.0
            || !session.judge.judged_notes.contains_key(&pair.start_note_id)
        {
            continue;
        }
        let idx = pair.lane.index();
        let end_judge = session.judge.judged_notes.get(&pair.end_note_id).copied();
        // beatoraja: pressed || 終端が PG/GR/GD で判定済みなら inclease。
        let inclease = session.lane_keyon_started_at[idx].is_some()
            || matches!(end_judge, Some(Judge::PGreat | Judge::Great | Judge::Good));
        let since = match session.lane_hcn_timer[idx] {
            Some(prev) if prev.inclease == inclease => prev.since,
            _ => audio_now,
        };
        // mpassingcount は inclease 反転でもリセットしない (beatoraja 準拠)。
        let passing_count_us = session.lane_hcn_timer[idx].map_or(0, |prev| prev.passing_count_us);
        next[idx] = Some(HcnLaneTimer { inclease, since, passing_count_us });

        // beatoraja: passing.getPair().getState() > 3 (終端 BAD 以下で判定済み)
        // のときのみキー音音量を制御する。
        if matches!(end_judge, Some(Judge::Bad | Judge::Poor | Judge::EmptyPoor))
            && session
                .chart
                .note_by_id(pair.start_note_id)
                .is_some_and(|note| note.sounds().next().is_some())
        {
            let muted = !inclease;
            if !session.display_only_lane_mask[idx]
                && session.lane_hcn_keysound_muted[idx] != Some(muted)
            {
                let volume = if muted {
                    0.0
                } else {
                    let chart_volume = bmz_chart::volume::chart_channel_volume_factor(
                        bmz_chart::volume::chart_volume_at_time(
                            &session.chart.key_volume_events,
                            pair.start_time,
                        ),
                    );
                    (session.audio_mix.master_volume
                        * session.audio_mix.effective_normalization_gain()
                        * session.audio_mix.key_volume
                        * chart_volume)
                        .clamp(0.0, 1.0)
                };
                if let Some(note) = session.chart.note_by_id(pair.start_note_id) {
                    session
                        .pending_keysound_volumes
                        .extend(note.sounds().map(|sound_id| (sound_id, volume)));
                }
            }
            next_muted[idx] = Some(muted);
        }
    }
    session.lane_hcn_timer = next;
    session.lane_hcn_keysound_muted = next_muted;
}

/// beatoraja `JudgeManager` の `hcnmduration` (200ms)。
const HCN_UPDATE_US: i64 = 200_000;

/// beatoraja の HCN ゲージ増減判定 (passing ベース)。
/// `lane_hcn_timer` (HCN 区間内かつ始端判定済みのレーン) を参照し、
/// `mpassingcount` 相当の符号付きカウンタにフレーム経過時間を加減算して、
/// ±200ms を超えるたびに GREAT/BAD × rate 0.5 でゲージを 1 tick 更新する。
/// 始端を見逃しても途中から押し直せば回復する。
pub fn apply_hcn_gauge(session: &mut GameSession, audio_now: TimeUs) {
    if session.lane_hcn_timer.iter().all(Option::is_none) {
        session.last_hcn_gauge_at = None;
        return;
    }

    let previous = session.last_hcn_gauge_at.unwrap_or(audio_now);
    session.last_hcn_gauge_at = Some(audio_now);
    if audio_now.0 <= previous.0 {
        return;
    }

    let delta_us = audio_now.0 - previous.0;
    for idx in 0..LANE_COUNT {
        let Some(mut timer) = session.lane_hcn_timer[idx] else {
            continue;
        };
        if timer.inclease {
            timer.passing_count_us += delta_us;
            while timer.passing_count_us > HCN_UPDATE_US {
                if session.display_only_lane_mask[idx] {
                    if let Some(gauge) = &mut session.opponent_gauge {
                        let previous_gauge = gauge.current().value;
                        gauge.apply_hcn_hold();
                        if gauge.current().value > previous_gauge + f32::EPSILON {
                            session.opponent_gauge_increase_started_at = Some(audio_now);
                        }
                    }
                } else {
                    let previous_gauge = session.gauge.current().value;
                    session.gauge.apply_hcn_hold();
                    update_gauge_increase_timer(session, previous_gauge, audio_now);
                }
                timer.passing_count_us -= HCN_UPDATE_US;
            }
        } else {
            timer.passing_count_us -= delta_us;
            while timer.passing_count_us < -HCN_UPDATE_US {
                if session.display_only_lane_mask[idx] {
                    if let Some(gauge) = &mut session.opponent_gauge {
                        gauge.apply_hcn_drain();
                    }
                } else {
                    session.gauge.apply_hcn_drain();
                }
                timer.passing_count_us += HCN_UPDATE_US;
            }
        }
        session.lane_hcn_timer[idx] = Some(timer);
    }
}
use super::*;
