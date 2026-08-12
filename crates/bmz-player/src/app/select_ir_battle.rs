use super::*;

use sha2::{Digest, Sha256};
use std::sync::mpsc::TryRecvError;

const IR_BATTLE_HOLD_DURATION: Duration = Duration::from_millis(120);
const IR_BATTLE_REPLAY_MAX_BYTES: usize = 8 * 1024 * 1024;
const IR_BATTLE_REPLAY_MAX_EVENTS: usize = 2_000_000;

#[derive(Default)]
pub(super) struct SelectIrBattleRuntime {
    pub(super) active: bool,
    pub(super) pinned: bool,
    pub(super) cursor: usize,
    pub(super) source_chart_id: Option<i64>,
    pub(super) source_sha256: Option<[u8; 32]>,
    pub(super) hold_started_at: Option<Instant>,
    pub(super) hold_control: Option<String>,
    pub(super) generation: u64,
    pub(super) loading: bool,
    pub(super) pending: Option<Receiver<SelectIrBattleReplayResult>>,
}

pub(super) struct SelectIrBattleReplayResult {
    generation: u64,
    chart_id: i64,
    target: std::result::Result<crate::screens::play_start::GhostBattleTarget, String>,
}

impl WinitApp {
    fn select_ir_battle_entries(&self) -> &[crate::screens::select_ir::SelectIrBattleEntry] {
        self.select.select_ir.battle_entries_for(self.select.ir_battle.source_sha256)
    }

    fn select_ir_battle_source(&self) -> Option<(i64, [u8; 32], KeyMode)> {
        let row = self.selected_chart_row()?;
        let chart = row.chart.as_ref()?;
        Some((chart.chart_id, row.score_sha256()?, KeyMode::from_str_opt(&chart.mode)?))
    }

    fn select_ir_battle_is_available(&self) -> bool {
        if self.select.session_mode != SessionMode::GhostBattle
            || self.select.select_option_panel != 0
            || self.select.search.is_active()
            || in_settings_stack(&self.select.folder_stack)
        {
            return false;
        }
        let Some((_, sha256, key_mode)) = self.select_ir_battle_source() else {
            return false;
        };
        if !matches!(key_mode, KeyMode::K5 | KeyMode::K7) {
            return false;
        }
        let Some(provider) =
            crate::ir::provider_key::primary_provider_config(&self.boot.profile_config.ir)
        else {
            return false;
        };
        !crate::ir::rian_ir::is_rian_ir_config(provider)
            && !self.select.select_ir.battle_entries_for(Some(sha256)).is_empty()
    }

    /// KEY4 press is deferred while a usable IR ranking is present. A short
    /// press retains the normal back action; a 120 ms hold temporarily swaps
    /// the song list for the ranking, matching LR2's interaction.
    pub(super) fn begin_select_ir_battle_hold(&mut self, control: &str) -> bool {
        if !self.select_ir_battle_is_available() {
            return false;
        }
        if self.select.ir_battle.active {
            return true;
        }
        let Some((chart_id, sha256, _)) = self.select_ir_battle_source() else {
            return false;
        };
        self.select.ir_battle.source_chart_id = Some(chart_id);
        self.select.ir_battle.source_sha256 = Some(sha256);
        self.select.ir_battle.hold_started_at = Some(Instant::now());
        self.select.ir_battle.hold_control = Some(control.to_string());
        true
    }

    pub(super) fn finish_select_ir_battle_hold(&mut self, control: &str) -> bool {
        if self.select.ir_battle.hold_control.as_deref() != Some(control) {
            return false;
        }
        let held_long_enough = self
            .select
            .ir_battle
            .hold_started_at
            .is_some_and(|started| started.elapsed() >= IR_BATTLE_HOLD_DURATION);
        self.select.ir_battle.hold_started_at = None;
        self.select.ir_battle.hold_control = None;
        if self.select.ir_battle.active || held_long_enough {
            if !self.select.ir_battle.pinned {
                self.close_select_ir_battle();
            }
        } else {
            self.exit_folder();
        }
        true
    }

    pub(super) fn advance_select_ir_battle_hold(&mut self) {
        if !self.ui.focused || !matches!(self.view_state(), AppViewState::Select) {
            if self.select.ir_battle.active
                || self.select.ir_battle.hold_started_at.is_some()
                || self.select.ir_battle.pending.is_some()
            {
                self.close_select_ir_battle();
            }
            return;
        }
        if self.select.ir_battle.active
            && (!self.select_ir_battle_is_available() || self.select_ir_battle_entries().is_empty())
        {
            self.close_select_ir_battle();
            return;
        }
        if self.select.ir_battle.active {
            return;
        }
        if self
            .select
            .ir_battle
            .hold_started_at
            .is_some_and(|started| started.elapsed() >= IR_BATTLE_HOLD_DURATION)
        {
            self.select.ir_battle.active = true;
            self.select.ir_battle.cursor = 0;
            self.restart_select_bar_timer_without_scroll(Instant::now());
            self.play_system_sound(crate::system_sound::SoundType::FolderOpen);
        }
    }

    pub(super) fn toggle_select_ir_battle(&mut self) -> bool {
        if self.select.ir_battle.active {
            self.close_select_ir_battle();
            return true;
        }
        if !self.select_ir_battle_is_available() {
            let text = Localizer::new(self.boot.profile_config.ui.locale());
            self.show_left_overlay_toast(text.text("toast-ir-battle-unavailable"));
            return true;
        }
        let Some((chart_id, sha256, _)) = self.select_ir_battle_source() else {
            return true;
        };
        self.select.ir_battle.source_chart_id = Some(chart_id);
        self.select.ir_battle.source_sha256 = Some(sha256);
        self.select.ir_battle.cursor = 0;
        self.select.ir_battle.active = true;
        self.select.ir_battle.pinned = true;
        self.restart_select_bar_timer_without_scroll(Instant::now());
        self.play_system_sound(crate::system_sound::SoundType::FolderOpen);
        true
    }

    pub(super) fn close_select_ir_battle(&mut self) -> bool {
        let was_open =
            self.select.ir_battle.active || self.select.ir_battle.hold_started_at.is_some();
        self.select.ir_battle.active = false;
        self.select.ir_battle.pinned = false;
        self.select.ir_battle.cursor = 0;
        self.select.ir_battle.source_chart_id = None;
        self.select.ir_battle.source_sha256 = None;
        self.select.ir_battle.hold_started_at = None;
        self.select.ir_battle.hold_control = None;
        self.select.ir_battle.loading = false;
        self.select.ir_battle.pending = None;
        self.select.ir_battle.generation = self.select.ir_battle.generation.wrapping_add(1);
        if was_open {
            self.restart_select_bar_timer_without_scroll(Instant::now());
            self.play_system_sound(crate::system_sound::SoundType::FolderClose);
        }
        was_open
    }

    pub(super) fn move_select_ir_battle(&mut self, select_move: SelectMove, duration: Duration) {
        let len = self.select_ir_battle_entries().len();
        if len == 0 {
            self.close_select_ir_battle();
            return;
        }
        let previous = self.select.ir_battle.cursor;
        self.select.ir_battle.cursor = moved_select_index(previous, len, select_move);
        if previous != self.select.ir_battle.cursor {
            self.select.select_bar_started_at = Instant::now();
            self.select.select_bar_scroll_direction = select_move_scroll_direction(select_move);
            self.select.select_bar_scroll_duration = duration;
            self.play_system_sound(crate::system_sound::SoundType::Scratch);
        }
    }

    pub(super) fn start_selected_ir_ghost_battle(&mut self) {
        if self.select.ir_battle.loading {
            return;
        }
        let Some(chart_id) = self.select.ir_battle.source_chart_id else {
            return;
        };
        let Some(sha256) = self.select.ir_battle.source_sha256 else {
            return;
        };
        let Some(entry) =
            self.select_ir_battle_entries().get(self.select.ir_battle.cursor).cloned()
        else {
            return;
        };
        let Some(score_id) = entry.score_id.clone().filter(|value| !value.is_empty()) else {
            self.show_ir_battle_error("ranking entry has no replay score id");
            return;
        };
        let Some(provider) =
            crate::ir::provider_key::primary_provider_config(&self.boot.profile_config.ir).cloned()
        else {
            self.show_ir_battle_error("primary IR provider is not configured");
            return;
        };
        if crate::ir::rian_ir::is_rian_ir_config(&provider) {
            self.show_ir_battle_error("this IR provider does not publish BMZ replay data");
            return;
        }
        let Some(provider_key) =
            crate::ir::provider_key::configured_provider_key(&provider).map(str::to_string)
        else {
            self.show_ir_battle_error("primary IR provider key is missing");
            return;
        };
        let ln_policy = self
            .selected_chart_row()
            .map(|row| {
                crate::ln_policy::score_ln_policy(
                    self.boot.profile_config.play.ln_mode_policy,
                    row.chart.as_ref().map(|chart| chart.ln_profile).unwrap_or_default(),
                )
            })
            .unwrap_or(crate::ln_policy::LnScorePolicy::ForceLn);
        let replay_dir = self.boot.profile_paths.replay_dir.clone();
        let generation = self.select.ir_battle.generation.wrapping_add(1);
        self.select.ir_battle.generation = generation;
        let (sender, receiver) = mpsc::channel();
        self.select.ir_battle.pending = Some(receiver);
        self.select.ir_battle.loading = true;
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        self.show_left_overlay_toast(text.text("toast-ir-battle-loading"));

        tokio::spawn(async move {
            let target = download_ir_ghost_target(
                &provider.base_url,
                &provider_key,
                &score_id,
                chart_id,
                sha256,
                ln_policy,
                &replay_dir,
                entry,
            )
            .await
            .map_err(|error| format!("{error:#}"));
            let _ = sender.send(SelectIrBattleReplayResult { generation, chart_id, target });
        });
    }

    pub(super) fn poll_select_ir_battle_replay(&mut self) {
        let result = match self.select.ir_battle.pending.as_ref().map(Receiver::try_recv) {
            Some(Ok(result)) => result,
            Some(Err(TryRecvError::Empty)) | None => return,
            Some(Err(TryRecvError::Disconnected)) => {
                self.select.ir_battle.pending = None;
                self.select.ir_battle.loading = false;
                self.show_ir_battle_error("IR replay worker stopped unexpectedly");
                return;
            }
        };
        self.select.ir_battle.pending = None;
        self.select.ir_battle.loading = false;
        if result.generation != self.select.ir_battle.generation || !self.select.ir_battle.active {
            return;
        }
        let target = match result.target {
            Ok(target) => target,
            Err(error) => {
                tracing::warn!(%error, "failed to prepare IR ghost battle replay");
                self.show_ir_battle_error(&error);
                return;
            }
        };
        let mut options = self.play_start_options();
        options.ghost_battle_target = Some(target);
        if !self.prepare_session_mode_or_show_error(result.chart_id, &mut options) {
            return;
        }
        self.close_select_ir_battle();
        self.begin_decide_for_chart(result.chart_id, options);
    }

    fn show_ir_battle_error(&mut self, error: &str) {
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let mut args = FluentArgs::new();
        args.set("error", error.to_string());
        self.show_left_overlay_toast(text.format("toast-ir-battle-failed", &args));
    }
}

pub(in crate::app) fn select_ir_battle_snapshot_rows(
    entries: &[crate::screens::select_ir::SelectIrBattleEntry],
    selected_index: usize,
    visible_limit: usize,
) -> Vec<SelectRowSnapshot> {
    select_visible_item_indices(entries.len(), selected_index, visible_limit)
        .into_iter()
        .map(|index| {
            let entry = &entries[index];
            SelectRowSnapshot {
                index: index as u32,
                title: entry.player_name.clone(),
                subtitle: entry.player_id.clone(),
                artist: format!("EX {} / BP {}", entry.ex_score, entry.bp),
                genre: "G-BATTLE".to_string(),
                difficulty_name: entry.clear.clone(),
                play_level: format!("#{}", entry.rank),
                table_level: entry.gauge.clone().unwrap_or_default(),
                clear_type: entry.clear.clone(),
                ex_score: Some(entry.ex_score),
                max_combo: Some(entry.max_combo),
                bp: Some(entry.bp),
                in_library: entry.score_id.is_some(),
                ..SelectRowSnapshot::default()
            }
        })
        .collect()
}

async fn download_ir_ghost_target(
    base_url: &str,
    provider: &str,
    score_id: &str,
    _chart_id: i64,
    chart_sha256: [u8; 32],
    ln_policy: crate::ln_policy::LnScorePolicy,
    replay_dir: &Path,
    entry: crate::screens::select_ir::SelectIrBattleEntry,
) -> Result<crate::screens::play_start::GhostBattleTarget> {
    let client = crate::ir::bmz_official::BmzOfficialIrClient::anonymous(base_url)?;
    let (bytes, metadata) =
        client.download_replay_with_metadata_limit(score_id, IR_BATTLE_REPLAY_MAX_BYTES).await?;
    if metadata.status.as_deref() != Some("verified") {
        anyhow::bail!("IR replay is not verified");
    }
    if metadata.format.as_deref() != Some("bmz-replay-v1") {
        anyhow::bail!("IR replay format is not supported");
    }
    if bytes.len() > IR_BATTLE_REPLAY_MAX_BYTES
        || metadata.size_bytes.is_some_and(|size| size > IR_BATTLE_REPLAY_MAX_BYTES as u64)
    {
        anyhow::bail!("IR replay exceeds the size limit");
    }
    if metadata.size_bytes.is_some_and(|size| size != bytes.len() as u64) {
        anyhow::bail!("IR replay size does not match its metadata");
    }
    let actual_hash = hash_to_hex(&Sha256::digest(&bytes));
    let declared_hash = metadata.hash.as_deref().filter(|hash| !hash.is_empty());
    if declared_hash != Some(actual_hash.as_str()) {
        anyhow::bail!("IR replay hash does not match its metadata");
    }
    let text = std::str::from_utf8(&bytes).context("IR replay is not UTF-8 TOML")?;
    let replay = crate::storage::replay::parse_replay(text)?;
    if replay.chart_sha256_bytes()? != chart_sha256 {
        anyhow::bail!("IR replay chart hash does not match the selected chart");
    }
    if !replay.ln_policy.is_empty()
        && crate::ln_policy::LnScorePolicy::from_str_opt(&replay.ln_policy) != Some(ln_policy)
    {
        anyhow::bail!("IR replay long note policy does not match the selected chart");
    }
    if replay.double_option_bucket() != crate::select_options::DoubleOptionScoreBucket::Off {
        anyhow::bail!("IR replay is not a single-play replay");
    }
    if replay.uses_legacy_seed_scheme() {
        anyhow::bail!("IR replay uses an unsupported legacy random seed");
    }
    if replay.events.is_empty() || replay.events.len() > IR_BATTLE_REPLAY_MAX_EVENTS {
        anyhow::bail!("IR replay has an invalid event count");
    }
    if replay.events.iter().any(|event| event.lane.index() >= 8) {
        anyhow::bail!("IR replay contains non-SP input lanes");
    }
    if replay.events.windows(2).any(|events| events[0].time > events[1].time) {
        anyhow::bail!("IR replay events are not ordered by time");
    }

    let provider_cache = hash_to_hex(&Sha256::digest(provider.as_bytes()));
    let score_cache = hash_to_hex(&Sha256::digest(score_id.as_bytes()));
    let cache_path = replay_dir.join("ir").join(provider_cache).join(format!("{score_cache}.toml"));
    crate::storage::replay::save_replay(&cache_path, &replay)
        .with_context(|| format!("failed to cache IR replay: {}", cache_path.display()))?;

    Ok(crate::screens::play_start::GhostBattleTarget {
        provider: provider.to_string(),
        score_id: score_id.to_string(),
        player_id: entry.player_id,
        player_name: entry.player_name,
        rank: entry.rank,
        ex_score: entry.ex_score,
        gauge: entry.gauge.as_deref().and_then(gauge_type_from_ir),
        replay,
    })
}

fn gauge_type_from_ir(value: &str) -> Option<GaugeType> {
    let normalized: String = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    match normalized.as_str() {
        "assisteasy" | "aeasy" => Some(GaugeType::AssistEasy),
        "easy" => Some(GaugeType::Easy),
        "normal" | "groove" | "clear" => Some(GaugeType::Normal),
        "hard" => Some(GaugeType::Hard),
        "exhard" => Some(GaugeType::ExHard),
        "hazard" => Some(GaugeType::Hazard),
        "class" => Some(GaugeType::Class),
        "exclass" => Some(GaugeType::ExClass),
        "exhardclass" => Some(GaugeType::ExHardClass),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranking_rows_keep_rank_and_score_identity() {
        let rows = select_ir_battle_snapshot_rows(
            &[crate::screens::select_ir::SelectIrBattleEntry {
                rank: 2,
                player_id: "rival-id".to_string(),
                player_name: "RIVAL".to_string(),
                score_id: Some("score-id".to_string()),
                ex_score: 1234,
                clear: "Hard".to_string(),
                bp: 7,
                max_combo: 400,
                gauge: Some("Hard".to_string()),
                verification: Some("verified_play".to_string()),
            }],
            0,
            25,
        );
        assert_eq!(rows[0].title, "RIVAL");
        assert_eq!(rows[0].play_level, "#2");
        assert_eq!(rows[0].ex_score, Some(1234));
        assert!(rows[0].in_library);
    }

    #[test]
    fn ir_gauge_names_are_normalized() {
        assert_eq!(gauge_type_from_ir("EX-HARD"), Some(GaugeType::ExHard));
        assert_eq!(gauge_type_from_ir("groove"), Some(GaugeType::Normal));
    }
}
