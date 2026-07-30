use super::*;

/// リザルト画面の IR 送信状況とランキングを表示するオーバーレイ。
pub(super) fn build_result_ir_panel(
    ctx: &egui::Context,
    state: &mut crate::screens::result_ir::ResultIrState,
    text: Localizer,
) {
    use crate::screens::result_ir::{IrSubmitState, RankingLoadState, ResultRankingTab};

    let content_rect = ctx.content_rect();
    let panel_width = 360.0_f32;
    let pos = egui::pos2(content_rect.right() - panel_width - 16.0, 16.0);

    egui::Window::new(tr!(text, "result-ir-title"))
        .id(egui::Id::new("result_ir_overlay"))
        .resizable(false)
        .collapsible(true)
        .movable(true)
        .current_pos(pos)
        .default_width(panel_width)
        .show(ctx, |ui| {
            match &state.submit {
                IrSubmitState::Sending => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tr!(text, "result-ir-submitting"));
                    });
                }
                IrSubmitState::Done { submitted, failed, message } => {
                    if *failed > 0 {
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            tr!(
                                text,
                                "result-ir-submit-failed",
                                "failed" => *failed,
                                "submitted" => *submitted
                            ),
                        );
                        if let Some(message) = message {
                            ui.small(message.clone());
                        }
                    } else if *submitted > 0 {
                        ui.colored_label(
                            egui::Color32::LIGHT_GREEN,
                            tr!(
                                text,
                                "result-ir-submitted",
                                "submitted" => *submitted
                            ),
                        );
                    } else {
                        ui.label(tr!(text, "result-ir-nothing-to-submit"));
                    }
                }
            }

            ui.separator();
            let mut selected_tab = None;
            ui.horizontal(|ui| {
                let global = state.active_tab == ResultRankingTab::Global;
                let rivals = state.active_tab == ResultRankingTab::SelfAndRivals;
                if ui.selectable_label(global, tr!(text, "result-ir-tab-global")).clicked()
                    && !global
                {
                    selected_tab = Some(ResultRankingTab::Global);
                }
                if state.supports_tab(ResultRankingTab::SelfAndRivals)
                    && ui.selectable_label(rivals, tr!(text, "result-ir-tab-rivals")).clicked()
                    && !rivals
                {
                    selected_tab = Some(ResultRankingTab::SelfAndRivals);
                }
            });
            if let Some(tab) = selected_tab {
                state.select_tab(tab);
            }
            // タブ未選択のまま NotRequested の場合 (prefetch OFF) も取得を開始する。
            if matches!(state.active_state(), RankingLoadState::NotRequested) {
                state.select_tab(state.active_tab);
            }

            match state.active_state() {
                RankingLoadState::NotRequested | RankingLoadState::Loading => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tr!(text, "result-ir-loading"));
                    });
                }
                RankingLoadState::Failed(error) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, tr!(text, "result-ir-load-failed"));
                    ui.small(error.clone());
                }
                RankingLoadState::Loaded(ranking) => {
                    if ranking.entries.is_empty() {
                        ui.label(tr!(text, "result-ir-empty"));
                    } else {
                        egui::Grid::new("result_ir_ranking_grid")
                            .num_columns(5)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("#");
                                ui.strong(tr!(text, "result-ir-player"));
                                ui.strong(tr!(text, "course-column-ex"));
                                ui.strong(tr!(text, "result-ir-clear"));
                                ui.strong(tr!(text, "course-column-bp"));
                                ui.end_row();
                                for entry in &ranking.entries {
                                    ui.monospace(entry.rank.to_string());
                                    ui.label(&entry.player_name);
                                    ui.monospace(entry.ex_score.to_string());
                                    ui.label(&entry.clear);
                                    ui.monospace(entry.bp.to_string());
                                    ui.end_row();
                                }
                            });
                        if let Some(rank) = ranking.self_rank {
                            ui.separator();
                            ui.label(tr!(text, "result-ir-self-rank", "rank" => rank));
                        }
                    }
                }
            }
        });
}
