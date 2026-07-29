macro_rules! skin_document_render_play_methods {
    () => {
        fn note_image_render_item(
            &self,
            lane: Lane,
            key_mode: KeyMode,
            rect: Rect,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            let note = self.note.as_ref()?;
            let image_id = note.note.get(beatoraja_note_index(lane, key_mode))?;
            self.note_part_render_item(image_id, rect, 0, sources)
        }

        /// LN START（ヘッドキャップ）画像を描画する。
        /// HCN モードでは `hcnstart`（beatoraja: `longImage[5]`）を優先し、
        /// `lnstart` → `note` の順にフォールバックする。
        fn note_ln_start_render_item(
            &self,
            lane: Lane,
            key_mode: KeyMode,
            rect: Rect,
            mode: LongNoteMode,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            let note = self.note.as_ref()?;
            let index = beatoraja_note_index(lane, key_mode);
            let hcn = (mode == LongNoteMode::Hcn).then(|| note.hcnstart.get(index)).flatten();
            let image_id =
                hcn.or_else(|| note.lnstart.get(index)).or_else(|| note.note.get(index))?;
            self.note_part_render_item(image_id, rect, 0, sources)
        }

        /// LN END（テールキャップ）画像を描画する。
        /// HCN モードでは `hcnend`（beatoraja: `longImage[4]`）を優先し、
        /// `lnend` → `note` の順にフォールバックする。
        fn note_ln_end_render_item(
            &self,
            lane: Lane,
            key_mode: KeyMode,
            rect: Rect,
            mode: LongNoteMode,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            let note = self.note.as_ref()?;
            let index = beatoraja_note_index(lane, key_mode);
            let hcn = (mode == LongNoteMode::Hcn).then(|| note.hcnend.get(index)).flatten();
            let image_id =
                hcn.or_else(|| note.lnend.get(index)).or_else(|| note.note.get(index))?;
            self.note_part_render_item(image_id, rect, 0, sources)
        }

        /// LN/CN 用の胴体画像 id を選択する。
        /// 新形式 (`lnbodyActive` 定義あり): 押下中=`lnbodyActive`, 非押下=`lnbody`。
        /// 旧形式: 押下中=`lnbody` (longImage\[2\]), 非押下=`lnactive` (longImage\[3\])。
        fn ln_body_image_id<'a>(
            &self,
            note: &'a SkinNoteSetDef,
            index: usize,
            pressing: bool,
        ) -> Option<&'a String> {
            if !note.lnbody_active.is_empty() {
                if pressing {
                    note.lnbody_active.get(index).or_else(|| note.lnbody.get(index))
                } else {
                    note.lnbody.get(index).or_else(|| note.lnbody_active.get(index))
                }
            } else if pressing {
                note.lnbody.get(index).or_else(|| note.lnactive.get(index))
            } else {
                note.lnactive.get(index).or_else(|| note.lnbody.get(index))
            }
        }

        /// HCN 用の胴体画像 id を選択する。beatoraja `JsonPlaySkinObjectLoader` の
        /// longImage 割り当てに準拠:
        /// 新形式 (`hcnbodyActive` 定義あり): \[6\]=`hcnbodyActive` \[7\]=`hcnbody`
        /// \[8\]=`hcnbodyReactive` \[9\]=`hcnbodyMiss`。
        /// 旧形式: \[6\]=`hcnbody` \[7\]=`hcnactive` \[8\]=`hcndamage` \[9\]=`hcnreactive`。
        fn hcn_body_image_id<'a>(
            &self,
            note: &'a SkinNoteSetDef,
            index: usize,
            state: LongBodyState,
        ) -> Option<&'a String> {
            let new_format = !note.hcnbody_active.is_empty();
            let primary = match state {
                LongBodyState::Processing => {
                    if new_format {
                        note.hcnbody_active.get(index)
                    } else {
                        note.hcnbody.get(index)
                    }
                }
                LongBodyState::Inactive => {
                    if new_format {
                        note.hcnbody.get(index)
                    } else {
                        note.hcnactive.get(index)
                    }
                }
                LongBodyState::HcnActive => {
                    if new_format {
                        note.hcnbody_reactive.get(index)
                    } else {
                        note.hcndamage.get(index)
                    }
                }
                LongBodyState::HcnDamage => {
                    if new_format {
                        note.hcnbody_miss.get(index)
                    } else {
                        note.hcnreactive.get(index)
                    }
                }
            };
            // 状態別画像が無い場合は HCN の基本 2 状態 → LN 胴体の順にフォールバック。
            primary
                .or_else(|| {
                    if new_format {
                        if state.is_processing() {
                            note.hcnbody_active.get(index).or_else(|| note.hcnbody.get(index))
                        } else {
                            note.hcnbody.get(index)
                        }
                    } else if state.is_processing() {
                        note.hcnbody.get(index).or_else(|| note.hcnactive.get(index))
                    } else {
                        note.hcnactive.get(index).or_else(|| note.hcnbody.get(index))
                    }
                })
                .or_else(|| self.ln_body_image_id(note, index, state.is_processing()))
        }

        /// ロングノート胴体画像を描画する。`mode` と `state` の組み合わせで
        /// beatoraja `drawLongNote` の longImage 選択を再現する。
        /// 該当画像が無ければ LN 胴体 → `note` の順にフォールバックする。
        fn note_long_body_render_item(
            &self,
            lane: Lane,
            key_mode: KeyMode,
            rect: Rect,
            mode: LongNoteMode,
            state: LongBodyState,
            draw_state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            let note = self.note.as_ref()?;
            let index = beatoraja_note_index(lane, key_mode);
            let image_id = if mode == LongNoteMode::Hcn {
                self.hcn_body_image_id(note, index, state)
            } else {
                self.ln_body_image_id(note, index, state.is_processing())
            }
            .or_else(|| note.note.get(index))?;
            let image = self.image.iter().find(|image| image.id == *image_id)?;
            // LR2 `SRC_LN_BODY` uses the lane HOLD timer for the processing image,
            // while the inactive copy has no timer and must remain on frame zero.
            let elapsed_ms = skin_timer_elapsed_ms(image.timer, draw_state).unwrap_or(0);
            self.note_part_render_item(image_id, rect, elapsed_ms, sources)
        }

        /// Mine ノート画像（`note.mine`）を描画する。スキンが `mine` を定義していない、
        /// または該当レーンの index が空なら `None` を返し、呼び出し側でフォールバックを
        /// 使う想定。
        fn note_mine_render_item(
            &self,
            lane: Lane,
            key_mode: KeyMode,
            rect: Rect,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            let note = self.note.as_ref()?;
            let image_id = note.mine.get(beatoraja_note_index(lane, key_mode))?;
            self.note_part_render_item(image_id, rect, 0, sources)
        }

        fn note_height_for_lane(&self, lane: Lane, key_mode: KeyMode) -> Option<f32> {
            let note = self.note.as_ref()?;
            let index = beatoraja_note_index(lane, key_mode);
            if let Some(size) = note.size.get(index).copied().filter(|size| *size > 0) {
                return Some(size as f32 / self.h.max(1) as f32);
            }
            let image_id = note.note.get(index)?;
            let image = self.image.iter().find(|image| image.id == *image_id)?;
            let divy = image.divy.max(1);
            Some((image.h.max(1) as f32 / divy as f32) / self.h.max(1) as f32)
        }

        fn note_part_render_item(
            &self,
            image_id: &str,
            rect: Rect,
            elapsed_ms: i32,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            let image = self.image.iter().find(|image| image.id == image_id)?;
            let source = resolve_document_source(sources, &image.src)?;
            Some(SkinRenderItem::Image {
                texture: source.texture,
                rect,
                uv: skin_image_texture_region(image, source.source_size, elapsed_ms),
                tint: Color::rgb(1.0, 1.0, 1.0),
                blend: BlendMode::Normal,
                scale: SkinImageScale::Stretch,
                border: None,
                source_size: Some(source.source_size),
                linear_filter: false,
            })
        }

        fn note_group_render_items(
            &self,
            note_y: f32,
            key_mode: KeyMode,
            state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Vec<SkinRenderItem> {
            let Some(note) = self.note.as_ref() else {
                return Vec::new();
            };
            self.note_line_render_items(&note.group, note_y, key_mode, state, sources)
        }

        fn note_line_render_items(
            &self,
            destinations: &[SkinDestinationDef],
            note_y: f32,
            key_mode: KeyMode,
            state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Vec<SkinRenderItem> {
            let images = self.image_map();
            let enabled_options = self.enabled_options();
            let Some(area) = self.note_lane_area(Lane::Key1, key_mode, &enabled_options) else {
                return Vec::new();
            };
            let canvas_h = self.h.max(1) as f32;
            let bottom_y = note_progress_to_y(area, note_y, state, canvas_h);
            let judge_bottom_px = canvas_h * (1.0 - note_judge_bottom_y(area, state, canvas_h));
            let timeline_bottom_px = canvas_h * (1.0 - bottom_y);
            let mut items = Vec::new();
            for destination in destinations {
                if !test_skin_ops(&destination.op, &enabled_options, state)
                    || !eval_skin_draw_condition(&destination.draw, state)
                {
                    continue;
                }
                let Some(elapsed) = skin_timer_elapsed_ms(destination.timer, state) else {
                    continue;
                };
                let Some(mut frame) =
                    resolve_destination_frame(destination, elapsed, &enabled_options, state)
                else {
                    continue;
                };
                frame.y += (timeline_bottom_px - judge_bottom_px).round() as i32;
                apply_bar_line_skin_offsets_to_frame(destination, &mut frame, state);
                let Some(image) = images.get(destination.id.as_str()) else {
                    continue;
                };
                let Some(source) = resolve_document_source(sources, &image.src) else {
                    continue;
                };
                let pixel_rect = skin_image_pixel_rect(image, &images);
                let (rect, uv) = stretch_skin_image_geometry(
                    destination.stretch,
                    normalize_skin_frame_rect(frame, self.w, self.h),
                    skin_image_texture_region_for_state(
                        image,
                        source.source_size,
                        elapsed,
                        Some(state),
                        pixel_rect,
                    ),
                    source.source_size,
                    self.w,
                    self.h,
                );
                let item = skin_image_item_for_frame(
                    source.texture,
                    rect,
                    uv,
                    frame,
                    destination.center,
                    if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal },
                    Some(source.source_size),
                    destination.filter != 0,
                );
                items.push(item);
            }
            items
        }

        /// `note.dst` の中から有効な条件に一致するエントリを探し、
        /// 指定レーンのノートエリア矩形（正規化座標）を返す。
        /// ノートエリアはレーン列全体を表す。Y軸: 上端=ノートが最も早い時点、下端=判定ライン。
        ///
        /// note.dst の解釈は2通り:
        /// 1. `load_beatoraja_json` 経由で読んだ場合: `expand_json_skin_value` により条件ブロックが
        ///    展開済みで、dst はレーン順の Frame エントリ列になっている。
        ///    → 全 Frame をフラット配列として `lane_idx` 番目を使う。
        /// 2. 直接 JSON パースした場合: Conditional エントリの frames 配列がレーン対応を持つ。
        ///    → 条件を満たす Conditional を探し、その frames[lane_idx] を使う。
        fn note_lane_area(
            &self,
            lane: Lane,
            key_mode: KeyMode,
            enabled_options: &[i32],
        ) -> Option<Rect> {
            let note = self.note.as_ref()?;
            let lane_idx = beatoraja_note_index(lane, key_mode);
            let canvas_w = self.w as f32;
            let canvas_h = self.h as f32;

            // 全エントリを展開してフラット化。Conditional は条件が合うものだけ展開する。
            let mut flat: Vec<SkinAnimationDef> = Vec::new();
            for entry in &note.dst {
                match entry {
                    SkinDstEntry::Frame(f) => flat.push(*f),
                    SkinDstEntry::Conditional { if_ops, frames } => {
                        if test_skin_dst_if(if_ops, enabled_options) {
                            flat.extend_from_slice(frames);
                        }
                    }
                }
            }

            let frame = flat.get(lane_idx)?;
            if let (Some(x), Some(y), Some(w), Some(h)) = (frame.x, frame.y, frame.w, frame.h) {
                Some(normalize_skin_frame_rect(
                    ResolvedSkinFrame { x, y, w, h, ..ResolvedSkinFrame::default() },
                    canvas_w as u32,
                    canvas_h as u32,
                ))
            } else {
                None
            }
        }

        fn primary_note_lane_height_px(&self) -> Option<i32> {
            let enabled_options = self.enabled_options();
            self.note_lane_area(Lane::Scratch, KeyMode::K7, &enabled_options)
                .or_else(|| self.note_lane_area(Lane::Key1, KeyMode::K7, &enabled_options))
                .map(|area| (area.height * self.h.max(1) as f32).round() as i32)
                .filter(|height| *height > 0)
        }

        fn apply_notes_offset_to_rect(&self, rect: Rect, state: &SkinDrawState) -> Rect {
            let Some(offset) = state.skin_offsets.get(OFFSET_NOTES_1P) else {
                return rect;
            };
            let canvas_w = self.w.max(1) as f32;
            let canvas_h = self.h.max(1) as f32;
            let offset_w = offset.w as f32 / canvas_w;
            let offset_h = offset.h as f32 / canvas_h;
            Rect {
                x: rect.x + offset.x as f32 / canvas_w - offset_w / 2.0,
                y: rect.y - offset.y as f32 / canvas_h - offset_h / 2.0,
                width: rect.width + offset_w,
                height: rect.height + offset_h,
            }
        }

        fn gauge_render_items(
            &self,
            gauge: f32,
            elapsed_ms: i32,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<Vec<SkinRenderItem>> {
            let state = SkinDrawState { elapsed_ms, gauge, ..SkinDrawState::default() };
            let enabled_options = self.enabled_options();
            let destination =
                self.all_destinations(&enabled_options).into_iter().find(|destination| {
                    self.destination_uses_skin_gauge_bar_render(destination)
                        && destination.timer.is_none()
                        && test_skin_ops(&destination.op, &enabled_options, &state)
                        && eval_skin_draw_condition(&destination.draw, &state)
                })?;
            self.resolve_gauge_destination_items(destination, &enabled_options, &state, sources)
        }

        fn destination_uses_skin_gauge_bar_render(&self, destination: &SkinDestinationDef) -> bool {
            self.skin_gauge_for_destination(destination).is_some()
                && destination.draw.trim().is_empty()
                && destination.blend != 2
        }

        fn destination_uses_skin_gauge_overlay_render(
            &self,
            destination: &SkinDestinationDef,
        ) -> bool {
            self.skin_gauge_for_destination(destination).is_some()
                && (!destination.draw.trim().is_empty() || destination.blend == 2)
        }

        fn skin_gauge_for_destination(
            &self,
            destination: &SkinDestinationDef,
        ) -> Option<&SkinGaugeDef> {
            self.gauges
                .iter()
                .find(|gauge| gauge.id == destination.id)
                .or_else(|| self.gauge.as_ref().filter(|gauge| gauge.id == destination.id))
        }

        fn resolve_gauge_destination_items(
            &self,
            destination: &SkinDestinationDef,
            enabled_options: &[i32],
            state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<Vec<SkinRenderItem>> {
            let gauge_def = self.skin_gauge_for_destination(destination)?;
            let elapsed_ms = skin_timer_elapsed_ms(destination.timer, state)?;
            let mut frame =
                resolve_destination_frame(destination, elapsed_ms, enabled_options, state)?;
            apply_skin_offset_to_frame(destination, &mut frame, state, false);
            let reverse_parts = skin_gauge_reverse_parts(frame);
            let rect = normalize_skin_frame_rect(frame, self.w, self.h);
            let parts = gauge_def.parts.max(1);
            let max = state.gauge_max.max(1.0);
            let border = state.gauge_border;
            let notes = skin_gauge_notes_count(state.gauge, parts, max);
            let animation = skin_gauge_animation_index(gauge_def, state);
            let exgauge = skin_gauge_node_base(state.gauge_type);
            let anim_type = gauge_def.gauge_type;
            let base_color = skin_gauge_frame_color(frame);
            let blend = skin_gauge_destination_blend(destination);
            let mut items = Vec::new();
            for part in 1..=parts {
                let part_border = part as f32 * max / parts as f32;
                let node_index = skin_gauge_sprite_node_index(
                    exgauge,
                    part,
                    notes,
                    animation,
                    border,
                    part_border,
                    gauge_def.nodes.len(),
                    anim_type,
                );
                let node_id = gauge_def.nodes.get(node_index)?;
                let part_rect = skin_gauge_part_rect(rect, parts, part, reverse_parts);
                if let Some(item) = self.gauge_image_render_item(
                    node_id,
                    part_rect,
                    elapsed_ms,
                    sources,
                    base_color,
                    blend,
                    destination.filter != 0,
                ) {
                    items.push(item);
                }
                if anim_type == SKIN_GAUGE_ANIM_FLICKERING
                    && notes > 0
                    && part == notes
                    && let Some(tip_index) = skin_gauge_flicker_tip_node_index(
                        exgauge,
                        border,
                        part_border,
                        gauge_def.nodes.len(),
                    )
                    && let Some(tip_id) = gauge_def.nodes.get(tip_index)
                {
                    let flicker_alpha = skin_gauge_flicker_alpha(animation, gauge_def.cycle);
                    let flicker_color = Color::rgba(
                        base_color.r,
                        base_color.g,
                        base_color.b,
                        base_color.a * flicker_alpha,
                    );
                    if let Some(item) = self.gauge_image_render_item(
                        tip_id,
                        part_rect,
                        elapsed_ms,
                        sources,
                        flicker_color,
                        blend,
                        destination.filter != 0,
                    ) {
                        items.push(item);
                    }
                }
            }
            Some(items)
        }

        fn judge_render_items(
            &self,
            judge: &str,
            combo: u32,
            elapsed_ms: i32,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<Vec<SkinRenderItem>> {
            self.judge_render_items_with_offsets(
                judge,
                combo,
                elapsed_ms,
                &SkinOffsetValues::default(),
                sources,
            )
        }

        fn judge_render_items_with_offsets(
            &self,
            judge: &str,
            combo: u32,
            elapsed_ms: i32,
            skin_offsets: &SkinOffsetValues,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<Vec<SkinRenderItem>> {
            let judge_image_index = judge_image_index(judge)?;
            let judge_def = self.judge.first()?;
            let state = SkinDrawState { skin_offsets: *skin_offsets, ..SkinDrawState::default() };
            self.judge_render_items_for_def(
                judge_def,
                judge_image_index,
                combo,
                elapsed_ms,
                sources,
                &state,
            )
        }

        fn judge_render_items_for_def(
            &self,
            judge: &SkinJudgeDef,
            judge_index: usize,
            combo: u32,
            elapsed_ms: i32,
            sources: &HashMap<String, SkinDocumentTexture>,
            state: &SkinDrawState,
        ) -> Option<Vec<SkinRenderItem>> {
            let image_destination = judge.images.get(judge_index)?;
            let enabled_options = self.enabled_options();
            let mut image_frame = resolve_destination_frame_until_end(
                image_destination,
                elapsed_ms,
                &enabled_options,
                state,
            )?;
            let offset_state = SkinDrawState {
                skin_offsets: state.skin_offsets,
                offset_lift_px: state.offset_lift_px,
                offset_lanecover_px: state.offset_lanecover_px,
                ..SkinDrawState::default()
            };
            // OFFSET_JUDGE_1P (id 32) は beatoraja では明示注入されず、destination の
            // `offsets` フィールドで宣言されたぶんだけ適用される。ここで重ねて
            // 注入すると、`offsets: [32]` を持つ skin (beatoraja 標準形) で
            // 二重適用になり、判定文字とコンボ数の Y が乖離する原因になる。
            apply_skin_offset_to_frame(image_destination, &mut image_frame, &offset_state, false);
            // beatoraja はコンボ数字をシフト前の判定文字 X を基準に配置する。
            let image_frame_for_numbers = image_frame;
            if judge.shift
                && combo > 0
                && let Some(number_destination) = judge.numbers.get(judge_index)
                && let Some(number_frame) = resolve_destination_frame_until_end(
                    number_destination,
                    elapsed_ms,
                    &enabled_options,
                    state,
                )
            {
                image_frame.x -=
                    self.value_number_length(&number_destination.id, combo as i64, number_frame)
                        / 2;
            }
            let image = self.image.iter().find(|image| image.id == image_destination.id)?;
            let source = resolve_document_source(sources, &image.src)?;
            let uv = skin_image_texture_region(image, source.source_size, elapsed_ms);
            let (rect, uv) = stretch_skin_image_geometry(
                image_destination.stretch,
                normalize_skin_frame_rect(image_frame, self.w, self.h),
                uv,
                source.source_size,
                self.w,
                self.h,
            );
            let mut items = vec![skin_image_item_for_frame(
                source.texture,
                rect,
                uv,
                image_frame,
                image_destination.center,
                BlendMode::Normal,
                Some(source.source_size),
                image_destination.filter != 0,
            )];
            if combo > 0
                && let Some(number_destination) = judge.numbers.get(judge_index)
                && let Some(mut number_frame) = resolve_destination_frame_until_end(
                    number_destination,
                    elapsed_ms,
                    &enabled_options,
                    state,
                )
            {
                // beatoraja は SkinNumber に `setRelative(true)` を立てるため、
                // destination の offsets を適用しても x/y は移動せず w/h/r/a だけ
                // 加算される。これにより combo digit の最終位置は
                // base_frame.y (= 適用後 image_frame.y) + number_frame.y_orig となり、
                // 判定文字と同じ量だけ y シフトする (中心アンカー伸縮)。
                apply_skin_offset_to_frame_relative(
                    number_destination,
                    &mut number_frame,
                    &offset_state,
                );
                let judge_align = self
                    .value
                    .iter()
                    .find(|value| value.id == number_destination.id)
                    .map_or(2, |value| value.judge_align.unwrap_or(2));
                if let Some(value) =
                    self.value.iter().find(|value| value.id == number_destination.id)
                    && judge_align == 2
                {
                    Self::apply_beatoraja_judge_number_dst_x(&mut number_frame, value.digit);
                }
                let signed_render = if self
                    .value
                    .iter()
                    .find(|value| value.id == number_destination.id)
                    .is_some_and(|value| {
                        ref_id_is_signed(value.ref_id) || value_layout_is_signed(value)
                    }) {
                    SignedNumberRender::Signed(SignedNumberRowOrder::PositiveFirst)
                } else {
                    SignedNumberRender::Unsigned
                };
                items.extend(self.value_number_render_items(
                    &number_destination.id,
                    combo as i64,
                    image_frame_for_numbers,
                    number_frame,
                    elapsed_ms,
                    sources,
                    false,
                    Some(judge_align),
                    signed_render,
                ));
            }
            Some(items)
        }

        /// beatoraja `JsonPlaySkinObjectLoader` が judge number の各 dst に適用する X 補正。
        fn beatoraja_judge_number_dst_x(dst_w: i32, digit: i32) -> i32 {
            dst_w.saturating_mul(digit.max(0)) / 2
        }

        fn apply_beatoraja_judge_number_dst_x(frame: &mut ResolvedSkinFrame, digit: i32) {
            frame.x -= Self::beatoraja_judge_number_dst_x(frame.w, digit);
        }

        fn value_number_length(
            &self,
            value_id: &str,
            number: i64,
            frame: ResolvedSkinFrame,
        ) -> i32 {
            let Some(value) = self.value.iter().find(|value| value.id == value_id) else {
                return 0;
            };
            let max_digits = value.digit.max(0) as usize;
            let padding = number_padding(value);
            let digits = if ref_id_is_signed(value.ref_id) || value_layout_is_signed(value) {
                display_signed_number_digits(
                    number,
                    max_digits,
                    signed_value_padding(value, padding),
                    value.divx.max(1) as u32,
                )
            } else {
                display_number_digits(number, max_digits, padding)
            };
            if digits.is_empty() { 0 } else { digits.len() as i32 * (frame.w + value.space) }
        }

        fn judge_image_render_item(
            &self,
            judge: &str,
            elapsed_ms: i32,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            self.judge_render_items(judge, 0, elapsed_ms, sources)?.into_iter().next()
        }

        fn value_number_render_items(
            &self,
            value_id: &str,
            number: i64,
            base_frame: ResolvedSkinFrame,
            frame: ResolvedSkinFrame,
            elapsed_ms: i32,
            sources: &HashMap<String, SkinDocumentTexture>,
            compact_digits: bool,
            align_override: Option<i32>,
            signed_render: SignedNumberRender,
        ) -> Vec<SkinRenderItem> {
            let Some(value) = self.value.iter().find(|value| value.id == value_id) else {
                return Vec::new();
            };
            let Some(source) = sources.get(&value.src) else {
                return Vec::new();
            };
            let divx = value.divx.max(1);
            let divy = value.divy.max(1);
            let source_width_px =
                if value.w == -1 { source.source_size.width.round() as i32 } else { value.w };
            let source_height_px =
                if value.h == -1 { source.source_size.height.round() as i32 } else { value.h };
            let cell_width_px = (source_width_px / divx) as f32;
            let cell_height_px = (source_height_px / divy) as f32;
            if cell_width_px <= 0.0 || cell_height_px <= 0.0 {
                return Vec::new();
            }
            let padding = number_padding(value);
            let max_digits = value.digit.max(0) as usize;
            let digits = match signed_render {
                SignedNumberRender::Signed(row_order) => {
                    display_signed_number_digits_with_row_order(
                        number,
                        max_digits,
                        signed_value_padding(value, padding),
                        divx as u32,
                        row_order,
                    )
                }
                SignedNumberRender::Unsigned => display_number_digits(number, max_digits, padding),
            };
            // 桁間スペース (space フィールド、px 単位)
            let digit_step = frame.w + value.space;
            // 先頭の空き桁数 (align のためのオフセット計算に使用)
            let shiftbase = max_digits.saturating_sub(digits.len());
            // align=0: 右寄せ (デフォルト), align=1: 左寄せ, align=2: 中央
            let align = align_override.unwrap_or(value.align);
            let shift = match align {
                1 => digit_step * shiftbase as i32,
                2 => digit_step * shiftbase as i32 / 2,
                _ => 0,
            };

            digits
                .into_iter()
                .enumerate()
                .map(|(index, digit)| {
                    let digit_position =
                        if compact_digits { index } else { shiftbase + index } as i32;
                    let rect = normalize_skin_frame_rect(
                        ResolvedSkinFrame {
                            x: base_frame.x + frame.x + digit_step * digit_position - shift,
                            y: base_frame.y + frame.y,
                            w: frame.w,
                            h: frame.h,
                            ..frame
                        },
                        self.w,
                        self.h,
                    );
                    let uv = Self::value_digit_texture_region(
                        value,
                        digit.into(),
                        elapsed_ms,
                        source.source_size,
                        cell_width_px,
                        cell_height_px,
                        divx,
                        divy,
                    );
                    let tint = Color::rgba(
                        frame.r as f32 / 255.0,
                        frame.g as f32 / 255.0,
                        frame.b as f32 / 255.0,
                        frame.a as f32 / 255.0,
                    );
                    SkinRenderItem::Image {
                        texture: source.texture,
                        rect,
                        uv,
                        tint,
                        blend: BlendMode::Normal,
                        scale: SkinImageScale::Stretch,
                        border: None,
                        source_size: Some(source.source_size),
                        linear_filter: false,
                    }
                })
                .collect()
        }

        fn value_digit_texture_region(
            value: &SkinValueDef,
            digit: u32,
            elapsed_ms: i32,
            source_size: SkinImageSize,
            cell_width_px: f32,
            cell_height_px: f32,
            divx: i32,
            divy: i32,
        ) -> TextureRegion {
            let source_width = source_size.width.max(1.0);
            let source_height = source_size.height.max(1.0);
            let digit_column = digit as i32 % divx;
            let digit_row = digit as i32 / divx;
            let animation_rows = divy.saturating_sub(digit_row).max(1);
            let animation_row = if value.cycle > 0 && animation_rows > 1 {
                (elapsed_ms.rem_euclid(value.cycle) * animation_rows / value.cycle)
                    .min(animation_rows - 1)
            } else {
                0
            };
            let source_row = (digit_row + animation_row).min(divy - 1);
            TextureRegion {
                x: (value.x as f32 + cell_width_px * digit_column as f32) / source_width,
                y: (value.y as f32 + cell_height_px * source_row as f32) / source_height,
                width: cell_width_px / source_width,
                height: cell_height_px / source_height,
            }
        }

        fn gauge_image_render_item(
            &self,
            image_id: &str,
            rect: Rect,
            elapsed_ms: i32,
            sources: &HashMap<String, SkinDocumentTexture>,
            tint: Color,
            blend: BlendMode,
            linear_filter: bool,
        ) -> Option<SkinRenderItem> {
            let image = self.image.iter().find(|image| image.id == image_id)?;
            let source = resolve_document_source(sources, &image.src)?;
            let uv = skin_image_texture_region(image, source.source_size, elapsed_ms);
            let (rect, uv) =
                stretch_skin_image_geometry(0, rect, uv, source.source_size, self.w, self.h);
            Some(SkinRenderItem::Image {
                texture: source.texture,
                rect,
                uv,
                tint,
                blend,
                scale: SkinImageScale::Stretch,
                border: None,
                source_size: Some(source.source_size),
                linear_filter,
            })
        }
    };
}

pub(super) use skin_document_render_play_methods;
