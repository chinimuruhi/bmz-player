use super::*;

pub(super) fn finalize_playable_chart(mut draft: PlayableChartDraft) -> PlayableChart {
    for lane_notes in &mut draft.lane_notes {
        lane_notes.sort_by_key(|note| note.time);
    }
    draft.long_notes.sort_by_key(|pair| pair.start_time);
    draft.bgm_events.sort_by_key(|event| event.time);
    draft.bga_events.sort_by_key(|event| event.time);
    draft.timing_events.sort_by_key(|event| event.time);
    draft.scroll_events.sort_by_key(|event| event.time);
    draft.speed_events.sort_by_key(|event| event.time);
    draft.judge_rank_events.sort_by_key(|event| event.time);
    draft.bgm_volume_events.sort_by_key(|event| event.time);
    draft.key_volume_events.sort_by_key(|event| event.time);
    draft.text_events.sort_by_key(|event| event.time);
    draft.bga_opacity_events.sort_by_key(|event| event.time);
    draft.bga_argb_events.sort_by_key(|event| event.time);
    draft.bga_keybound_events.sort_by_key(|event| event.time);
    draft.bar_lines.sort_by_key(|line| line.time);

    draft.total_notes = compute_total_notes(&draft.lane_notes);
    if draft.metadata.difficulty_name.trim().is_empty()
        || draft.metadata.difficulty_name.trim() == "0"
    {
        draft.metadata.difficulty_name = infer_beatoraja_difficulty_name(
            &draft.metadata.title,
            &draft.metadata.subtitle,
            draft.total_notes,
        )
        .to_string();
    }
    draft.end_time = compute_end_time(&draft);

    PlayableChart {
        identity: draft.identity,
        metadata: draft.metadata,
        lane_notes: draft.lane_notes,
        long_notes: draft.long_notes,
        bgm_events: draft.bgm_events,
        bga_events: draft.bga_events,
        timing_events: draft.timing_events,
        scroll_events: draft.scroll_events,
        speed_events: draft.speed_events,
        judge_rank_events: draft.judge_rank_events,
        bgm_volume_events: draft.bgm_volume_events,
        key_volume_events: draft.key_volume_events,
        text_events: draft.text_events,
        bga_opacity_events: draft.bga_opacity_events,
        bga_argb_events: draft.bga_argb_events,
        swbga_definitions: draft.swbga_definitions,
        bga_keybound_events: draft.bga_keybound_events,
        bga_asset_by_bmp_key: draft.bga_asset_by_bmp_key,
        bar_lines: draft.bar_lines,
        sounds: draft.sounds,
        bga_assets: draft.bga_assets,
        total_notes: draft.total_notes,
        end_time: draft.end_time,
    }
}

pub(super) fn compute_total_notes(lane_notes: &[Vec<NoteEvent>; LANE_COUNT]) -> u32 {
    lane_notes
        .iter()
        .flat_map(|notes| notes.iter())
        .filter(|note| matches!(note.kind, NoteKind::Tap | NoteKind::LongStart))
        .count() as u32
}

fn infer_beatoraja_difficulty_name(title: &str, subtitle: &str, total_notes: u32) -> &'static str {
    let subtitle = subtitle.to_lowercase();
    if let Some(difficulty) = difficulty_name_in_text(&subtitle) {
        return difficulty;
    }

    let full_title = format!("{title}{subtitle}").to_lowercase();
    if let Some(difficulty) = difficulty_name_in_text(&full_title) {
        return difficulty;
    }

    match total_notes {
        0..250 => "BEGINNER",
        250..600 => "NORMAL",
        600..1000 => "HYPER",
        1000..2000 => "ANOTHER",
        _ => "INSANE",
    }
}

fn difficulty_name_in_text(text: &str) -> Option<&'static str> {
    if text.contains("beginner") {
        Some("BEGINNER")
    } else if text.contains("normal") {
        Some("NORMAL")
    } else if text.contains("hyper") {
        Some("HYPER")
    } else if text.contains("another") {
        Some("ANOTHER")
    } else if text.contains("insane") || text.contains("leggendaria") {
        Some("INSANE")
    } else {
        None
    }
}

pub(super) fn compute_end_time(draft: &PlayableChartDraft) -> TimeUs {
    let lane_end = draft
        .lane_notes
        .iter()
        .flat_map(|notes| notes.iter().map(|note| note.time.0))
        .max()
        .unwrap_or(0);
    let long_end = draft.long_notes.iter().map(|pair| pair.end_time.0).max().unwrap_or(0);
    let playable_end = lane_end.max(long_end);
    if playable_end > 0 {
        return TimeUs(playable_end);
    }

    let bgm_end = draft.bgm_events.iter().map(|event| event.time.0).max().unwrap_or(0);
    TimeUs(bgm_end)
}

impl PlayableChartDraft {
    pub(super) fn new(
        identity: bmz_core::chart::ChartIdentity,
        metadata: ChartMetadata,
        sounds: Vec<SoundAssetRef>,
        bga_assets: Vec<BgaAssetRef>,
    ) -> Self {
        Self {
            identity,
            metadata,
            lane_notes: std::array::from_fn(|_| Vec::new()),
            long_notes: Vec::new(),
            bgm_events: Vec::new(),
            bga_events: Vec::new(),
            timing_events: Vec::new(),
            scroll_events: Vec::new(),
            speed_events: Vec::new(),
            judge_rank_events: Vec::new(),
            bgm_volume_events: Vec::new(),
            key_volume_events: Vec::new(),
            text_events: Vec::new(),
            bga_opacity_events: Vec::new(),
            bga_argb_events: Vec::new(),
            swbga_definitions: Vec::new(),
            bga_keybound_events: Vec::new(),
            bga_asset_by_bmp_key: HashMap::new(),
            bar_lines: Vec::new(),
            sounds,
            bga_assets,
            total_notes: 0,
            end_time: TimeUs(0),
        }
    }
}

#[cfg(test)]
mod difficulty_tests {
    use super::infer_beatoraja_difficulty_name;

    #[test]
    fn infers_difficulty_with_beatoraja_text_priority() {
        assert_eq!(infer_beatoraja_difficulty_name("Song Another", "", 1), "ANOTHER");
        assert_eq!(infer_beatoraja_difficulty_name("Song Another", "Hyper", 1), "HYPER");
        assert_eq!(infer_beatoraja_difficulty_name("Song", "LEGGENDARIA", 1), "INSANE");
    }

    #[test]
    fn infers_stronger_difficulties_from_playable_note_counts() {
        let title = "STRONGER (あたしは、もっと強く)";
        for (total_notes, expected) in
            [(299, "NORMAL"), (590, "NORMAL"), (1_238, "ANOTHER"), (1_593, "ANOTHER")]
        {
            assert_eq!(infer_beatoraja_difficulty_name(title, "", total_notes), expected);
        }
    }

    #[test]
    fn infers_difficulty_at_beatoraja_note_count_boundaries() {
        for (total_notes, expected) in [
            (249, "BEGINNER"),
            (250, "NORMAL"),
            (599, "NORMAL"),
            (600, "HYPER"),
            (999, "HYPER"),
            (1_000, "ANOTHER"),
            (1_999, "ANOTHER"),
            (2_000, "INSANE"),
        ] {
            assert_eq!(infer_beatoraja_difficulty_name("Song", "", total_notes), expected);
        }
    }
}
