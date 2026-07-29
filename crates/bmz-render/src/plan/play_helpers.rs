use super::*;

pub(super) fn skin_lane_height_px(
    skin: &SkinContext,
    key_mode: KeyMode,
    fallback_canvas_h: f32,
) -> f32 {
    skin.document()
        .and_then(|document| {
            let enabled_options = document.enabled_options();
            document.note_lane_area(Lane::Key1, key_mode, &enabled_options)
        })
        .map_or(fallback_canvas_h, |rect| rect.height * fallback_canvas_h)
}

pub(super) fn skin_lift_offset_px(lift: f32, lane_h: f32) -> i32 {
    (lift * lane_h).round() as i32
}

pub(super) fn skin_lanecover_offset_px(lane_cover: f32, _lift: f32, lane_h: f32) -> i32 {
    (-(lane_h * lane_cover.clamp(0.0, 1.0))).round() as i32
}

pub(super) fn lane_cover_bottom_progress(lane_cover: f32, lift: f32) -> f32 {
    let lift = lift.clamp(0.0, 1.0);
    let visible = (1.0 - lift).max(f32::EPSILON);
    ((visible - lane_cover.clamp(0.0, visible)) / visible).clamp(0.0, 1.0)
}

pub(super) fn skin_hidden_cover_offset_px(lift: f32, hidden_cover: f32, lane_h: f32) -> i32 {
    ((1.0 - lift) * hidden_cover * lane_h).round() as i32
}

pub(super) fn push_judge_line(
    skin_manifest: &SkinManifest,
    commands: &mut Vec<DrawCommand>,
    board: Rect,
    lift: f32,
) {
    let image = skin_manifest.play_judge_line_image();
    let line_y = judge_line_y(board, lift);
    append_skin_render_items(
        commands,
        &[SkinRenderItem::Image {
            texture: SkinTextureId(image.texture),
            rect: Rect { x: board.x, y: line_y, width: board.width, height: 0.006 },
            uv: image.uv,
            tint: skin_image_tint(Lane::Key1),
            blend: BlendMode::Normal,
            scale: image.scale,
            border: image.border,
            source_size: image.source_size,
            linear_filter: false,
        }],
    );
}

pub(super) fn note_rect_y(board: Rect, lift: f32, progress_to_hit: f32) -> f32 {
    play_object_y(board, lift, progress_to_hit) - NOTE_HEIGHT
}

pub(super) fn play_object_y(board: Rect, lift: f32, progress_to_hit: f32) -> f32 {
    let judge_y = judge_line_y(board, lift);
    judge_y - progress_to_hit.clamp(0.0, 1.0) * (judge_y - board.y)
}

pub(super) fn push_play_bar_line(
    commands: &mut Vec<DrawCommand>,
    skin: &SkinContext,
    skin_state: &crate::skin::SkinDrawState,
    key_mode: KeyMode,
    board: Rect,
    lift: f32,
    bar: &crate::snapshot::VisibleBarLine,
    skin_offsets: &SkinOffsetValues,
) {
    let start = commands.len();
    let items = skin.document_bar_line_items(bar.y, key_mode, skin_state);
    if items.is_empty() {
        if skin.document().is_none() {
            push_bar_line_rect_geometry(commands, board, lift, bar.y, skin_offsets);
        }
    } else {
        let items = skin.apply_play_skin_global_offset(items, skin_state);
        append_skin_render_items(commands, &items);
    }
    apply_bar_line_alpha_offset(&mut commands[start..], skin_offsets);
}

pub(super) fn push_play_aux_lines(
    commands: &mut Vec<DrawCommand>,
    skin: &SkinContext,
    skin_state: &crate::skin::SkinDrawState,
    snapshot: &RenderSnapshot,
    key_mode: KeyMode,
    skin_offsets: &SkinOffsetValues,
) {
    if !snapshot.practice_mode {
        return;
    }
    for line in &snapshot.time_lines {
        push_play_aux_line(commands, skin, skin_state, line, skin_offsets, |skin, y| {
            skin.document_time_line_items(y, key_mode, skin_state)
        });
    }
    for line in &snapshot.bpm_lines {
        push_play_aux_line(commands, skin, skin_state, line, skin_offsets, |skin, y| {
            skin.document_bpm_line_items(y, key_mode, skin_state)
        });
    }
    for line in &snapshot.stop_lines {
        push_play_aux_line(commands, skin, skin_state, line, skin_offsets, |skin, y| {
            skin.document_stop_line_items(y, key_mode, skin_state)
        });
    }
}

pub(super) fn push_play_aux_line(
    commands: &mut Vec<DrawCommand>,
    skin: &SkinContext,
    skin_state: &crate::skin::SkinDrawState,
    line: &crate::snapshot::VisibleBarLine,
    skin_offsets: &SkinOffsetValues,
    render: impl FnOnce(&SkinContext, f32) -> Vec<crate::skin::SkinRenderItem>,
) {
    let start = commands.len();
    let items = skin.apply_play_skin_global_offset(render(skin, line.y), skin_state);
    append_skin_render_items(commands, &items);
    apply_bar_line_alpha_offset(&mut commands[start..], skin_offsets);
}

/// beatoraja `SkinObject.prepareColor` 相当。小節線コマンド列に alpha offset を加算する。
pub(super) fn apply_bar_line_alpha_offset(
    commands: &mut [DrawCommand],
    skin_offsets: &SkinOffsetValues,
) {
    let offset = skin_offsets.get(SKIN_OFFSET_BAR_LINE).unwrap_or_default();
    if offset.a == 0 {
        return;
    }
    let alpha_delta = offset.a as f32 / 255.0;
    for command in commands {
        match command {
            DrawCommand::Image { tint, .. } | DrawCommand::RotatedImage { tint, .. } => {
                tint.a = (tint.a + alpha_delta).clamp(0.0, 1.0);
            }
            DrawCommand::Rect { color, .. } => {
                color.a = (color.a + alpha_delta).clamp(0.0, 1.0);
            }
            DrawCommand::RectBatch { rects, .. } => {
                for rect in Arc::make_mut(rects) {
                    rect.color.a = (rect.color.a + alpha_delta).clamp(0.0, 1.0);
                }
            }
            DrawCommand::Text { .. } => {}
        }
    }
}

pub(super) fn push_bar_line_rect_geometry(
    commands: &mut Vec<DrawCommand>,
    board: Rect,
    lift: f32,
    bar_y: f32,
    skin_offsets: &SkinOffsetValues,
) {
    let y = play_object_y(board, lift, bar_y);
    let offset = skin_offsets.get(SKIN_OFFSET_BAR_LINE).unwrap_or_default();
    let height = (0.004 + offset.h as f32 / 1080.0).max(0.0);
    commands.push(DrawCommand::Rect {
        rect: Rect { x: board.x, y, width: board.width, height },
        color: Color::rgba(0.45, 0.48, 0.5, 1.0),
    });
}

pub(super) fn judge_line_y(board: Rect, lift: f32) -> f32 {
    let lift_offset = lift.clamp(0.0, 1.0) * board.height;
    let raw = board.y + board.height * JUDGE_LINE_Y_RATIO - lift_offset;
    raw.max(board.y)
}

pub(super) fn push_receptors(
    skin_manifest: &SkinManifest,
    commands: &mut Vec<DrawCommand>,
    board: Rect,
    lift: f32,
    lane_width: f32,
    active_lanes: &[Lane],
) {
    let receptor = skin_manifest.play_receptor_image();
    let lift_offset = lift.clamp(0.0, 1.0) * board.height;
    let receptor_y = (board.y + board.height * 0.825 - lift_offset).max(board.y);
    for (display_index, &lane) in active_lanes.iter().enumerate() {
        let x = board.x + display_index as f32 * lane_width;
        append_skin_render_items(
            commands,
            &[SkinRenderItem::Image {
                texture: SkinTextureId(receptor.texture_for_lane(lane)),
                rect: Rect {
                    x: x + lane_width * 0.1,
                    y: receptor_y,
                    width: lane_width * 0.8,
                    height: 0.026,
                },
                uv: receptor.uv,
                tint: skin_image_tint(lane),
                blend: BlendMode::Normal,
                scale: receptor.scale,
                border: receptor.border,
                source_size: receptor.source_size,
                linear_filter: false,
            }],
        );
    }
}

pub(super) fn push_combo_panel(
    skin_manifest: &SkinManifest,
    commands: &mut Vec<DrawCommand>,
    combo: u32,
) {
    let width = if combo >= 1000 { 0.2 } else { 0.15 };
    let image = skin_manifest.play_combo_panel_image(combo > 0);
    append_skin_render_items(
        commands,
        &[SkinRenderItem::Image {
            texture: SkinTextureId(image.texture),
            rect: Rect { x: 0.425 - width / 2.0, y: 0.16, width, height: 0.07 },
            uv: image.uv,
            tint: Color::rgb(1.0, 1.0, 1.0),
            blend: BlendMode::Normal,
            scale: image.scale,
            border: image.border,
            source_size: image.source_size,
            linear_filter: false,
        }],
    );
}

pub(super) fn play_progress(snapshot: &RenderSnapshot) -> f32 {
    if snapshot.duration.0 <= 0 {
        0.0
    } else {
        (snapshot.time.0 as f32 / snapshot.duration.0 as f32).clamp(0.0, 1.0)
    }
}

pub(super) fn end_of_note(snapshot: &RenderSnapshot) -> bool {
    snapshot.duration.0 > 0 && snapshot.time.0 >= snapshot.duration.0
}

pub(super) fn push_play_text(
    text: &TextRenderer,
    commands: &mut Vec<DrawCommand>,
    snapshot: &RenderSnapshot,
) {
    push_play_status_text(text, commands, snapshot);
    push_judge_count_text(text, commands, snapshot);
    let metadata = difficulty_level_label(&snapshot.difficulty_name, &snapshot.play_level, "");
    if !metadata.is_empty() {
        text.push_text(
            commands,
            &metadata,
            BitmapTextStyle {
                x: 0.055,
                y: 0.155,
                cell: 0.0045,
                color: Color::rgb(0.62, 0.76, 0.72),
            },
        );
    }
    text.push_text(
        commands,
        &format!("G{}", snapshot.gauge.round() as u32),
        BitmapTextStyle { x: 0.885, y: 0.08, cell: 0.007, color: Color::rgb(0.8, 0.92, 0.86) },
    );
    if let Some(judgement) = snapshot.recent_judgements.last() {
        text.push_text(
            commands,
            &format_delta_ms(judgement.delta_us),
            BitmapTextStyle {
                x: 0.405,
                y: 0.282,
                cell: 0.0045,
                color: Color::rgb(0.72, 0.82, 0.86),
            },
        );
    }
}

pub(super) fn push_default_play_skin(
    skin_context: &SkinContext,
    commands: &mut Vec<DrawCommand>,
    snapshot: &RenderSnapshot,
) {
    let skin = default_play_skin(snapshot);
    let recent_judgement = snapshot.recent_judgements.last();
    let judge_text = recent_judgement.map(|judgement| judgement.text.clone()).unwrap_or_default();
    let judge_image = recent_judgement.and_then(|judgement| {
        let region_count = skin_context.document().map(|d| d.judge_region_count()).unwrap_or(1);
        let region = crate::skin::lane_judge_region(
            judgement.lane.index(),
            bmz_core::lane::LANE_COUNT,
            region_count,
        );
        skin_context.document_judge_items(
            &judgement.text,
            snapshot.combo,
            ((snapshot.time.0 - judgement.time.0) / 1_000).clamp(i32::MIN as i64, i32::MAX as i64)
                as i32,
            &snapshot.skin_offsets,
            region,
        )
    });
    let has_judge_image = judge_image.is_some();
    if let Some(judge_items) = judge_image {
        append_skin_render_items(commands, &judge_items);
    }
    let text_values = [(TextSlot::Judge, judge_text)];
    let number_values = [
        (NumberSlot::Combo, snapshot.combo as i64),
        (NumberSlot::Gauge, snapshot.gauge.round() as i64),
        (NumberSlot::Hispeed, (snapshot.hispeed * 100.0).round() as i64),
    ];
    let items = skin.resolve(&SkinRenderContext {
        phase: SkinPhase::Play,
        elapsed_ms: (snapshot.time.0 / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        text: &text_values,
        numbers: &number_values,
    });
    let items = if has_judge_image {
        items
            .into_iter()
            .filter(|item| {
                !matches!(item, SkinRenderItem::Text { text, .. } if text == &text_values[0].1)
                    && !matches!(item, SkinRenderItem::Text { text, .. } if text == &snapshot.combo.to_string())
            })
            .collect::<Vec<_>>()
    } else {
        items
    };
    append_skin_render_items(commands, &items);
}

pub(super) fn push_default_note_skin(
    skin_manifest: &SkinManifest,
    commands: &mut Vec<DrawCommand>,
    lane: Lane,
    rect: Rect,
) {
    push_note_skin_image(commands, lane, rect, skin_manifest.play_note_image());
}

pub(super) fn push_ln_start_skin(
    skin_manifest: &SkinManifest,
    commands: &mut Vec<DrawCommand>,
    lane: Lane,
    rect: Rect,
) {
    push_note_skin_image(commands, lane, rect, skin_manifest.play_ln_start_image());
}

pub(super) fn push_ln_end_skin(
    skin_manifest: &SkinManifest,
    commands: &mut Vec<DrawCommand>,
    lane: Lane,
    rect: Rect,
) {
    push_note_skin_image(commands, lane, rect, skin_manifest.play_ln_end_image());
}

pub(super) fn push_note_skin_image(
    commands: &mut Vec<DrawCommand>,
    lane: Lane,
    rect: Rect,
    note: SkinImageManifest,
) {
    append_skin_render_items(
        commands,
        &[SkinRenderItem::Image {
            texture: SkinTextureId(note.texture_for_lane(lane)),
            rect,
            uv: note.uv,
            tint: skin_image_tint(lane),
            blend: BlendMode::Normal,
            scale: note.scale,
            border: note.border,
            source_size: note.source_size,
            linear_filter: false,
        }],
    );
}

pub(super) fn default_play_skin(snapshot: &RenderSnapshot) -> SkinDefinition {
    let mut objects = Vec::new();
    if snapshot.combo > 0 {
        objects.push(SkinObject {
            id: SkinObjectId(1),
            source: SkinSource::Number {
                slot: NumberSlot::Combo,
                style: TextStyle {
                    font_id: None,
                    size: 0.05,
                    bitmap_size: None,
                    color: Color::rgb(0.94, 0.98, 1.0),
                    layer: TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
                digits: 0,
            },
            placements: vec![skin_placement(Rect { x: 0.38, y: 0.18, width: 0.18, height: 0.07 })],
        });
    }

    if snapshot.recent_judgements.last().is_some() {
        objects.push(SkinObject {
            id: SkinObjectId(2),
            source: SkinSource::Text {
                slot: TextSlot::Judge,
                style: TextStyle {
                    font_id: None,
                    size: 0.03,
                    bitmap_size: None,
                    color: Color::rgb(0.96, 0.92, 0.54),
                    layer: TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            },
            placements: vec![skin_placement(Rect { x: 0.38, y: 0.245, width: 0.3, height: 0.04 })],
        });
    }

    SkinDefinition { objects }
}

pub(super) fn skin_placement(rect: Rect) -> SkinPlacement {
    SkinPlacement {
        phase: SkinPhase::Play,
        time_ms: 0,
        rect,
        alpha: 1.0,
        blend: BlendMode::Normal,
        animation: Animation::none(),
    }
}

pub(super) fn push_start_overlay(
    text: &TextRenderer,
    commands: &mut Vec<DrawCommand>,
    snapshot: &RenderSnapshot,
) {
    let Some(label) = start_overlay_label(snapshot.time) else {
        return;
    };
    let cell = 0.018;
    text.push_text(
        commands,
        label,
        BitmapTextStyle {
            x: 0.5 - label_width(label, cell) / 2.0,
            y: 0.385,
            cell,
            color: if label == "READY" {
                Color::rgb(0.74, 0.88, 0.9)
            } else {
                Color::rgb(0.96, 0.92, 0.54)
            },
        },
    );
}

pub(super) fn push_default_failed_overlay(
    text: &TextRenderer,
    commands: &mut Vec<DrawCommand>,
    snapshot: &RenderSnapshot,
) {
    let Some(elapsed_ms) = snapshot.failed_elapsed_ms else {
        return;
    };
    let alpha = (elapsed_ms as f32 / 700.0).clamp(0.0, 0.82);
    commands.push(DrawCommand::Rect {
        rect: Rect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
        color: Color::rgba(0.0, 0.0, 0.0, alpha),
    });
    let label = "FAILED";
    let cell = 0.02;
    text.push_text(
        commands,
        label,
        BitmapTextStyle {
            x: 0.5 - label_width(label, cell) / 2.0,
            y: 0.43,
            cell,
            color: Color::rgba(1.0, 0.24, 0.28, alpha.clamp(0.35, 1.0)),
        },
    );
}

pub(super) fn push_judge_count_text(
    text: &TextRenderer,
    commands: &mut Vec<DrawCommand>,
    snapshot: &RenderSnapshot,
) {
    commands.push(DrawCommand::Rect {
        rect: Rect { x: 0.05, y: 0.36, width: 0.11, height: 0.235 },
        color: Color::rgb(0.032, 0.036, 0.04),
    });

    let rows = [
        ("PG", snapshot.judge_counts.pgreat, Color::rgb(0.66, 0.92, 0.98)),
        ("GR", snapshot.judge_counts.great, Color::rgb(0.66, 0.92, 0.98)),
        ("GD", snapshot.judge_counts.good, Color::rgb(0.84, 0.88, 0.48)),
        ("BD", snapshot.judge_counts.bad, Color::rgb(0.94, 0.56, 0.36)),
        ("PR", snapshot.judge_counts.poor, Color::rgb(0.96, 0.4, 0.44)),
        ("EP", snapshot.judge_counts.empty_poor, Color::rgb(0.96, 0.4, 0.44)),
    ];

    for (index, (label, count, color)) in rows.into_iter().enumerate() {
        text.push_text(
            commands,
            &format!("{label} {count}"),
            BitmapTextStyle { x: 0.065, y: 0.382 + index as f32 * 0.032, cell: 0.004, color },
        );
    }
}

pub(super) fn push_lane_text(
    text: &TextRenderer,
    commands: &mut Vec<DrawCommand>,
    board: Rect,
    lane_width: f32,
    active_lanes: &[Lane],
) {
    for (display_index, &lane) in active_lanes.iter().enumerate() {
        let center_x = board.x + display_index as f32 * lane_width + lane_width / 2.0;
        let label = lane_label(lane);
        text.push_text(
            commands,
            label,
            BitmapTextStyle {
                x: center_x - label_width(label, 0.0035) / 2.0,
                y: board.y + 0.018,
                cell: 0.0035,
                color: Color::rgb(0.45, 0.55, 0.58),
            },
        );
        let key = lane_key_label(lane);
        text.push_text(
            commands,
            key,
            BitmapTextStyle {
                x: center_x - label_width(key, 0.004) / 2.0,
                y: board.y + board.height * 0.9,
                cell: 0.004,
                color: Color::rgb(0.78, 0.86, 0.84),
            },
        );
    }
}

pub(super) fn push_play_status_text(
    text: &TextRenderer,
    commands: &mut Vec<DrawCommand>,
    snapshot: &RenderSnapshot,
) {
    commands.push(DrawCommand::Rect {
        rect: Rect { x: 0.05, y: 0.08, width: 0.11, height: 0.285 },
        color: Color::rgb(0.035, 0.04, 0.044),
    });
    text.push_text(
        commands,
        &format!("EX {}", snapshot.ex_score),
        BitmapTextStyle { x: 0.065, y: 0.105, cell: 0.0055, color: Color::rgb(0.82, 0.9, 0.92) },
    );
    text.push_text(
        commands,
        &format!("MAX {}", snapshot.max_combo),
        BitmapTextStyle { x: 0.065, y: 0.15, cell: 0.0055, color: Color::rgb(0.82, 0.9, 0.92) },
    );
    text.push_text(
        commands,
        &format!("NOTE {}", snapshot.past_notes.min(snapshot.total_notes)),
        BitmapTextStyle { x: 0.065, y: 0.195, cell: 0.005, color: Color::rgb(0.68, 0.78, 0.8) },
    );
    text.push_text(
        commands,
        &format!("/{}", snapshot.total_notes),
        BitmapTextStyle { x: 0.065, y: 0.235, cell: 0.005, color: Color::rgb(0.68, 0.78, 0.8) },
    );
    text.push_text(
        commands,
        &format_time(snapshot.time),
        BitmapTextStyle { x: 0.065, y: 0.28, cell: 0.0045, color: Color::rgb(0.48, 0.62, 0.66) },
    );
    text.push_text(
        commands,
        &format!("HS {:.2}", snapshot.hispeed),
        BitmapTextStyle { x: 0.065, y: 0.32, cell: 0.0045, color: Color::rgb(0.72, 0.82, 0.8) },
    );
}

pub(super) fn push_judgement_history(
    text: &TextRenderer,
    commands: &mut Vec<DrawCommand>,
    snapshot: &RenderSnapshot,
) {
    if snapshot.recent_judgements.is_empty() {
        return;
    }

    commands.push(DrawCommand::Rect {
        rect: Rect { x: 0.885, y: 0.17, width: 0.09, height: 0.19 },
        color: Color::rgb(0.03, 0.035, 0.038),
    });
    text.push_text(
        commands,
        "JUDGE",
        BitmapTextStyle { x: 0.897, y: 0.188, cell: 0.004, color: Color::rgb(0.5, 0.62, 0.64) },
    );

    for (index, judgement) in snapshot.recent_judgements.iter().rev().take(4).enumerate() {
        let y = 0.225 + index as f32 * 0.032;
        text.push_text(
            commands,
            &judgement_history_label(judgement),
            BitmapTextStyle {
                x: 0.897,
                y,
                cell: 0.0038,
                color: judgement_history_color(&judgement.text),
            },
        );
    }
}

pub(super) fn format_delta_ms(delta_us: i64) -> String {
    let sign = if delta_us < 0 { "-" } else { "+" };
    format!("{}{}MS", sign, delta_us.abs() / 1_000)
}

pub(super) fn format_percent(rate: f32) -> String {
    format!("{}%", (rate.clamp(0.0, 1.0) * 100.0).round() as u32)
}

pub(super) fn format_time(time: TimeUs) -> String {
    let seconds = (time.0.max(0) / 1_000_000) as u32;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

pub(super) fn start_overlay_label(time: TimeUs) -> Option<&'static str> {
    match time.0 {
        ..=999_999 => Some("READY"),
        1_000_000..=1_599_999 => Some("GO"),
        _ => None,
    }
}

pub(super) fn lane_flash_color(snapshot: &RenderSnapshot, lane: Lane) -> Option<Color> {
    if let Some(judgement_color) = judgement_lane_flash_color(snapshot, lane) {
        return Some(judgement_color);
    }

    input_lane_flash_color(snapshot, lane)
}

pub(super) fn long_note_body_color(mode: LongNoteMode) -> Color {
    match mode {
        LongNoteMode::Ln => LONG_NOTE_BODY_COLOR,
        LongNoteMode::Cn => CN_BODY_COLOR,
        LongNoteMode::Hcn => HCN_BODY_COLOR,
    }
}

pub(super) fn judgement_lane_flash_color(snapshot: &RenderSnapshot, lane: Lane) -> Option<Color> {
    let judgement = snapshot.recent_judgements.iter().rev().find(|judgement| {
        judgement.lane == lane
            && !judgement.is_miss
            && (0..=220_000).contains(&(snapshot.time.0 - judgement.time.0))
    })?;
    let age_us = (snapshot.time.0 - judgement.time.0).max(0) as f32;
    let alpha = (1.0 - age_us / 220_000.0).clamp(0.0, 1.0) * 0.55;
    Some(judge_flash_color(&judgement.text, alpha))
}

pub(super) fn input_lane_flash_color(snapshot: &RenderSnapshot, lane: Lane) -> Option<Color> {
    let input = snapshot.recent_inputs.iter().rev().find(|input| {
        input.lane == lane && (0..=140_000).contains(&(snapshot.time.0 - input.time.0))
    })?;
    let age_us = (snapshot.time.0 - input.time.0).max(0) as f32;
    let alpha = (1.0 - age_us / 140_000.0).clamp(0.0, 1.0) * 0.32;
    Some(Color::rgba(0.95, 0.98, 1.0, alpha))
}

pub(super) fn judge_flash_color(text: &str, alpha: f32) -> Color {
    if text.starts_with("PGREAT") || text.starts_with("GREAT") {
        Color::rgba(0.55, 0.9, 1.0, alpha)
    } else if text.starts_with("GOOD") {
        Color::rgba(0.85, 0.9, 0.45, alpha)
    } else {
        Color::rgba(1.0, 0.28, 0.32, alpha)
    }
}

pub(super) fn judgement_history_label(judgement: &crate::snapshot::DisplayJudgement) -> String {
    format!("{} {}", judge_short_label(&judgement.text), side_short_label(&judgement.text))
}

pub(super) fn judge_short_label(text: &str) -> &'static str {
    if text.starts_with("PGREAT") {
        "PG"
    } else if text.starts_with("GREAT") {
        "GR"
    } else if text.starts_with("GOOD") {
        "GD"
    } else if text.starts_with("BAD") {
        "BD"
    } else if text.starts_with("EMPTY POOR") {
        "EP"
    } else if text.starts_with("POOR") {
        "PR"
    } else {
        "??"
    }
}

pub(super) fn side_short_label(text: &str) -> &'static str {
    if text.ends_with("FAST") {
        "F"
    } else if text.ends_with("SLOW") {
        "S"
    } else {
        "-"
    }
}

pub(super) fn judgement_history_color(text: &str) -> Color {
    if text.starts_with("PGREAT") || text.starts_with("GREAT") {
        Color::rgb(0.64, 0.9, 0.98)
    } else if text.starts_with("GOOD") {
        Color::rgb(0.84, 0.88, 0.48)
    } else {
        Color::rgb(0.96, 0.4, 0.44)
    }
}

pub(super) fn display_title(title: &str) -> String {
    display_label(title, 24)
}

pub(super) fn display_label(text: &str, max_chars: usize) -> String {
    let ascii: String = text
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '.' | '/' | ':') {
                ch
            } else {
                '?'
            }
        })
        .take(max_chars)
        .collect();
    if ascii.is_empty() { "NO TITLE".to_string() } else { ascii }
}

pub(super) fn skin_image_tint(_lane: Lane) -> Color {
    Color::rgb(1.0, 1.0, 1.0)
}

pub(super) fn lane_label(lane: Lane) -> &'static str {
    match lane {
        Lane::Scratch => "SC",
        Lane::Key1 => "1",
        Lane::Key2 => "2",
        Lane::Key3 => "3",
        Lane::Key4 => "4",
        Lane::Key5 => "5",
        Lane::Key6 => "6",
        Lane::Key7 => "7",
        Lane::Key8 => "1'",
        Lane::Key9 => "2'",
        Lane::Key10 => "3'",
        Lane::Key11 => "4'",
        Lane::Key12 => "5'",
        Lane::Key13 => "6'",
        Lane::Key14 => "7'",
        Lane::Scratch2 => "S2",
    }
}

pub(super) fn lane_key_label(lane: Lane) -> &'static str {
    match lane {
        Lane::Scratch => "LS",
        Lane::Key1 => "Z",
        Lane::Key2 => "S",
        Lane::Key3 => "X",
        Lane::Key4 => "D",
        Lane::Key5 => "C",
        Lane::Key6 => "F",
        Lane::Key7 => "V",
        Lane::Key8 => "Z",
        Lane::Key9 => "S",
        Lane::Key10 => "X",
        Lane::Key11 => "D",
        Lane::Key12 => "C",
        Lane::Key13 => "F",
        Lane::Key14 => "V",
        Lane::Scratch2 => "LS",
    }
}

pub(super) fn label_width(label: &str, cell: f32) -> f32 {
    let chars = label.chars().count() as f32;
    if chars == 0.0 { 0.0 } else { (chars * 3.0 + (chars - 1.0)) * cell }
}
