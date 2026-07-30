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
