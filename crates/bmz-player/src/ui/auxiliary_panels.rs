use super::*;

pub(super) fn build_third_party_notice_panel(
    ctx: &egui::Context,
    open: &mut bool,
    app_paths: &AppPaths,
    notice_text: &mut Option<String>,
    text: Localizer,
) {
    if !*open {
        return;
    }
    let notice = notice_text.get_or_insert_with(|| combined_license_notice_text(app_paths));
    let mut notice = notice.as_str();
    localized_sized_panel_window(
        "license_notice_panel",
        tr!(text, "licenses-title"),
        ctx,
        open,
        620.0,
        560.0,
        egui::pos2(936.0, 320.0),
    )
    .show(ctx, |ui| {
        scrollable_window_content(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut notice)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .interactive(false),
            );
        });
    });
}

pub(super) fn combined_license_notice_text(app_paths: &AppPaths) -> String {
    combined_license_notice_text_with_repo_root(app_paths, &repo_root())
}

pub(super) fn combined_license_notice_text_with_repo_root(
    app_paths: &AppPaths,
    repo_root: &Path,
) -> String {
    let third_party = third_party_notice_text(app_paths);
    let rust_dependencies = rust_dependency_license_text(app_paths, repo_root);

    format!(
        "{third_party}\n\n\n================================================================\nGenerated Rust Dependency License Report\n================================================================\n\n{rust_dependencies}"
    )
}

pub(super) fn third_party_notice_text(app_paths: &AppPaths) -> String {
    let packaged = app_paths.resource_dir.join(THIRD_PARTY_NOTICE_PATH);
    read_non_empty_text(&packaged).unwrap_or_else(|| BUNDLED_THIRD_PARTY_NOTICES.to_string())
}

pub(super) fn rust_dependency_license_text(app_paths: &AppPaths, repo_root: &Path) -> String {
    let packaged = app_paths.resource_dir.join(RUST_DEPENDENCY_LICENSE_PATH);
    if let Some(text) = read_non_empty_text(&packaged) {
        return text;
    }

    let local = repo_root.join(LOCAL_RUST_DEPENDENCY_LICENSE_FILE);
    if let Some(text) = read_non_empty_text(&local) {
        return text;
    }

    missing_rust_dependency_license_text(&packaged, &local)
}

pub(super) fn read_non_empty_text(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().filter(|text| !text.trim().is_empty())
}

pub(super) fn missing_rust_dependency_license_text(packaged: &Path, local: &Path) -> String {
    format!(
        "BMZ Player Rust Dependency Licenses\n===================================\n\nThe generated Rust dependency license report was not found.\n\nExpected packaged path:\n  {}\n\nLocal development fallback:\n  {}\n\nGenerate it from the repository root with:\n\n  cargo-about generate --workspace --locked --fail \\\n    --output-file rust-dependency-licenses.txt \\\n    about.hbs\n",
        packaged.display(),
        local.display()
    )
}

pub(super) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(super) fn directory_open_targets(app_paths: &AppPaths) -> [DirectoryOpenTarget<'_>; 4] {
    [
        DirectoryOpenTarget { label: "resource_dir", path: &app_paths.resource_dir },
        DirectoryOpenTarget { label: "data_dir", path: &app_paths.data_dir },
        DirectoryOpenTarget { label: "cache_dir", path: &app_paths.cache_dir },
        DirectoryOpenTarget { label: "logs_dir", path: &app_paths.logs_dir },
    ]
}

pub(super) fn open_directory_target(
    target: DirectoryOpenTarget<'_>,
    text: Localizer,
) -> DirectoryOpenStatus {
    let error = open_directory(target.path, text).err();
    DirectoryOpenStatus { label: target.label, path: target.path.to_path_buf(), error }
}

pub(super) fn open_directory(path: &Path, text: Localizer) -> Result<(), String> {
    if !path.is_dir() {
        return Err(tr!(
            text,
            "menu-directory-missing",
            "path" => path.display().to_string()
        ));
    }
    spawn_directory_opener(path).map_err(|error| format!("{} ({})", error, path.display()))
}

#[cfg(target_os = "macos")]
pub(super) fn spawn_directory_opener(path: &Path) -> std::io::Result<()> {
    run_directory_opener("open", path)
}

#[cfg(target_os = "windows")]
pub(super) fn spawn_directory_opener(path: &Path) -> std::io::Result<()> {
    // explorer.exe may hand the request to the existing shell process and
    // return a non-zero status even though the directory was opened.
    Command::new("explorer").arg(path).spawn().map(|_| ())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn spawn_directory_opener(path: &Path) -> std::io::Result<()> {
    run_directory_opener("xdg-open", path)
}

#[cfg(unix)]
pub(super) fn run_directory_opener(program: &str, path: &Path) -> std::io::Result<()> {
    let status = Command::new(program).arg(path).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("{program} exited with {status}")))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
pub(super) fn spawn_directory_opener(_path: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "opening directories is not supported on this platform",
    ))
}

/// Window 内コンテンツを全体スクロール可能にする。
pub(super) fn scrollable_window_content<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    // レイアウト確定前に inner が膨らむのを防ぐため、
    // 利用可能矩形から ScrollArea 高さを明示的に制限する。
    let available = ui.available_rect_before_wrap();
    let max_height = available.height().max(64.0);
    egui::ScrollArea::vertical()
        .max_height(max_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(available.width());
            add_contents(ui)
        })
        .inner
}

/// パネル Window の default / max サイズと初期位置をビューポート内に収める。
pub(super) const PANEL_VIEWPORT_MARGIN: f32 = 16.0;

/// Window の outer サイズ = inner + chrome。egui `Window` の resize margin 計算に合わせる。
pub(super) fn panel_window_chrome(ctx: &egui::Context) -> egui::Vec2 {
    let style = ctx.global_style();
    let frame = egui::Frame::window(&style);
    let title_bar_inner_height = ctx
        .fonts_mut(|fonts| fonts.row_height(&style.text_styles[&egui::TextStyle::Heading]))
        .at_least(style.spacing.interact_size.y)
        + frame.inner_margin.sum().y;
    let title_content_spacing = frame.stroke.width;
    let frame_margin = frame.total_margin().sum();
    egui::vec2(frame_margin.x, frame_margin.y + title_bar_inner_height + title_content_spacing)
}

pub(super) fn clamp_panel_layout(
    constrain: egui::Rect,
    chrome: egui::Vec2,
    preferred_width: f32,
    preferred_height: f32,
    preferred_pos: egui::Pos2,
) -> (egui::Vec2, egui::Vec2, egui::Pos2) {
    let max_inner = egui::vec2(
        (constrain.width() - chrome.x).max(200.0),
        (constrain.height() - chrome.y).max(80.0),
    );
    let default_inner =
        egui::vec2(preferred_width.min(max_inner.x), preferred_height.min(max_inner.y));
    let outer = default_inner + chrome;
    let max_x = (constrain.max.x - outer.x).max(constrain.min.x);
    let max_y = (constrain.max.y - outer.y).max(constrain.min.y);
    let default_pos = egui::pos2(
        preferred_pos.x.clamp(constrain.min.x, max_x),
        preferred_pos.y.clamp(constrain.min.y, max_y),
    );
    (default_inner, max_inner, default_pos)
}

/// egui `Context::constrain_window_rect_to_area` と同等 (crate 外からは非公開のため)。
pub(super) fn constrain_window_rect_to_area(window: egui::Rect, area: egui::Rect) -> egui::Rect {
    let mut pos = window.min;
    let margin_x = (window.width() - area.width()).at_least(0.0);
    let margin_y = (window.height() - area.height()).at_least(0.0);
    pos.x = pos.x.at_most(area.right() + margin_x - window.width());
    pos.x = pos.x.at_least(area.left() - margin_x);
    pos.y = pos.y.at_most(area.bottom() + margin_y - window.height());
    pos.y = pos.y.at_least(area.top() - margin_y);
    egui::Rect::from_min_size(pos, window.size())
}

/// 翻訳で title が変わっても Window の状態を維持する、固定 ID 付きパネル。
pub(super) fn localized_sized_panel_window<'open>(
    id: &'static str,
    title: String,
    ctx: &egui::Context,
    open: &'open mut bool,
    preferred_width: f32,
    preferred_height: f32,
    default_pos: egui::Pos2,
) -> egui::Window<'open> {
    let constrain = ctx.content_rect().shrink(PANEL_VIEWPORT_MARGIN);
    let chrome = panel_window_chrome(ctx);
    let (default_inner, max_inner, clamped_default_pos) =
        clamp_panel_layout(constrain, chrome, preferred_width, preferred_height, default_pos);
    let window_id = egui::Id::new(id);
    let pos = ctx
        .memory(|memory| memory.area_rect(window_id))
        .map(|rect| constrain_window_rect_to_area(rect, constrain).min)
        .unwrap_or(clamped_default_pos);
    egui::Window::new(title)
        .id(window_id)
        .open(open)
        .resizable(true)
        .constrain_to(constrain)
        .current_pos(pos)
        .default_size(default_inner)
        .max_size(max_inner)
        .min_size([280.0, 80.0])
}

/// コース全体リザルトを画面上にオーバーレイ表示する。
///
/// `finished_course` が `Some` のあいだ表示され続け、リザルト画面を抜けると
/// `None` になって自動的に消える。最小実装として egui::Window を 1 枚出すだけ。
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

pub(super) fn build_course_result_panel(
    ctx: &egui::Context,
    summary: &CourseResultSummary,
    result_ir_visible: bool,
    text: Localizer,
) {
    let content_rect = ctx.content_rect();
    // Panel widened from 360px to 440px so the 6-column per-chart grid
    // (#/title/EX/combo/clear/miss) fits without horizontal scroll.
    let panel_width = 440.0_f32;
    let right_margin = if result_ir_visible { 360.0 + 32.0 } else { 16.0 };
    let pos_x = (content_rect.right() - panel_width - right_margin).max(content_rect.left() + 16.0);
    let pos = egui::pos2(pos_x, 16.0);

    egui::Window::new(tr!(text, "course-result-title"))
        .id(egui::Id::new("course_result_overlay"))
        .resizable(false)
        .collapsible(true)
        .movable(true)
        .title_bar(true)
        .current_pos(pos)
        .default_width(panel_width)
        .show(ctx, |ui| {
            ui.heading(&summary.title);

            ui.horizontal(|ui| {
                let kind_label = match summary.kind {
                    bmz_core::course::CourseKind::Dan => tr!(text, "course-kind-dan"),
                    bmz_core::course::CourseKind::Course => tr!(text, "course-kind-course"),
                };
                ui.label(kind_label);
                ui.separator();
                if summary.course_failed {
                    ui.colored_label(egui::Color32::LIGHT_RED, tr!(text, "course-status-failed"));
                } else if summary.course_clear {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, tr!(text, "course-status-clear"));
                } else {
                    ui.colored_label(
                        egui::Color32::LIGHT_YELLOW,
                        tr!(text, "course-status-no-trophy"),
                    );
                }
                ui.separator();
                ui.label(format!("{}/{}", summary.played_entries, summary.total_entries));
            });

            ui.separator();

            // Totals.
            let score_rate = if summary.max_ex_score > 0 {
                summary.total_ex_score as f32 / summary.max_ex_score as f32 * 100.0
            } else {
                0.0
            };
            egui::Grid::new("course_result_totals").num_columns(2).show(ui, |ui| {
                ui.label(tr!(text, "course-ex-score"));
                ui.label(format!(
                    "{} / {} ({:.2}%)",
                    summary.total_ex_score, summary.max_ex_score, score_rate
                ));
                ui.end_row();
                ui.label(tr!(text, "course-notes"));
                ui.label(format!("{}", summary.total_notes));
                ui.end_row();
                ui.label("BP");
                ui.label(format!("{}", summary.bp));
                ui.end_row();
                ui.label(tr!(text, "course-pg-great"));
                ui.label(format!(
                    "{} / {}",
                    summary.judge_counts.pgreat, summary.judge_counts.great
                ));
                ui.end_row();
                ui.label(tr!(text, "course-good-bad-poor"));
                ui.label(format!(
                    "{} / {} / {}",
                    summary.judge_counts.good, summary.judge_counts.bad, summary.judge_counts.poor,
                ));
                ui.end_row();
            });

            if !summary.trophy_results.is_empty() {
                ui.separator();
                ui.label(tr!(text, "course-trophies"));
                // `trophy_results` is built only from `definition.trophies`
                // in `ActiveCourseSession::into_result`, so it cannot show
                // a name that the course author did not declare.
                ui.horizontal_wrapped(|ui| {
                    for trophy in &summary.trophy_results {
                        let color = if trophy.achieved {
                            egui::Color32::from_rgb(255, 215, 0) // gold
                        } else {
                            egui::Color32::DARK_GRAY
                        };
                        ui.colored_label(color, &trophy.name);
                    }
                });
            }

            // BEST section: shows the highest persisted attempt for this
            // course.  Includes the current attempt if it improved the
            // record (the lookup runs after insert_course_score).
            if let Some(best) = &summary.best_score {
                ui.separator();
                ui.label(tr!(text, "course-best"));
                let best_rate = if best.max_ex_score > 0 {
                    best.ex_score as f32 / best.max_ex_score as f32 * 100.0
                } else {
                    0.0
                };
                let is_new_record = best.ex_score == summary.total_ex_score
                    && best.max_ex_score == summary.max_ex_score
                    && !summary.course_failed;
                egui::Grid::new("course_result_best").num_columns(2).show(ui, |ui| {
                    ui.label(tr!(text, "course-ex-score"));
                    let ex_text =
                        format!("{} / {} ({:.2}%)", best.ex_score, best.max_ex_score, best_rate);
                    if is_new_record {
                        ui.colored_label(egui::Color32::from_rgb(255, 215, 0), ex_text);
                    } else {
                        ui.label(ex_text);
                    }
                    ui.end_row();
                    ui.label(tr!(text, "course-column-clear"));
                    ui.label(&best.clear_type);
                    ui.end_row();
                    ui.label(tr!(text, "course-max-combo"));
                    ui.label(format!("{}", best.max_combo));
                    ui.end_row();
                    ui.label("BP");
                    ui.label(format!("{}", best.bp));
                    ui.end_row();
                });
                if is_new_record {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 215, 0),
                        tr!(text, "course-new-record"),
                    );
                }
            }

            if !summary.entry_summaries.is_empty() {
                ui.separator();
                ui.label(tr!(text, "course-each-song"));
                egui::Grid::new("course_result_entries").num_columns(6).striped(true).show(
                    ui,
                    |ui| {
                        // Header row.
                        ui.label("#");
                        ui.label(tr!(text, "course-column-title"));
                        ui.label(tr!(text, "course-column-ex"));
                        ui.label(tr!(text, "course-column-combo"));
                        ui.label(tr!(text, "course-column-clear"));
                        ui.label(tr!(text, "course-column-bp"));
                        ui.end_row();
                        for (i, entry) in summary.entry_summaries.iter().enumerate() {
                            ui.label(format!("{}", i + 1));
                            let title = if entry.title.is_empty() {
                                tr!(text, "common-no-title")
                            } else {
                                entry.title.clone()
                            };
                            ui.label(title);
                            ui.label(format!("{}", entry.ex_score));
                            ui.label(format!("{}", entry.max_combo));
                            // Color the clear cell so failed entries stand out.
                            let clear_text = entry.clear_type.as_str();
                            let clear_color = match entry.clear_type {
                                bmz_core::clear::ClearType::Failed => egui::Color32::LIGHT_RED,
                                bmz_core::clear::ClearType::FullCombo
                                | bmz_core::clear::ClearType::Perfect
                                | bmz_core::clear::ClearType::Max => egui::Color32::LIGHT_GREEN,
                                _ => ui.visuals().text_color(),
                            };
                            ui.colored_label(clear_color, clear_text);
                            ui.label(format!("{}", entry.bp));
                            ui.end_row();
                        }
                    },
                );
            }
        });
}

/// 選曲画面でコース行にカーソルがある間、コース内の各曲のメタ情報を表示する
/// プレビューパネル。
pub(super) fn build_course_preview_panel(
    ctx: &egui::Context,
    preview: &SelectCourseRow,
    text: Localizer,
) {
    let content_rect = ctx.content_rect();
    let pos = egui::pos2(16.0, content_rect.bottom() - 320.0);

    egui::Window::new(tr!(text, "course-preview-title"))
        .id(egui::Id::new("course_preview_overlay"))
        .resizable(false)
        .collapsible(true)
        .movable(true)
        .title_bar(true)
        .current_pos(pos)
        .default_width(380.0)
        .max_height(300.0)
        .show(ctx, |ui| {
            ui.heading(&preview.title);
            ui.horizontal(|ui| {
                ui.label(&preview.category_label);
                ui.separator();
                ui.label(tr!(
                    text,
                    "course-preview-resolved",
                    "resolved" => preview.resolved_count,
                    "total" => preview.entry_count
                ));
                ui.separator();
                ui.label(tr!(text, "course-preview-notes", "notes" => preview.total_notes));
            });
            if !preview.trophy_names.is_empty() {
                ui.label(tr!(
                    text,
                    "course-preview-trophies",
                    "trophies" => preview.trophy_names.join(" / ")
                ));
            }
            ui.separator();
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                egui::Grid::new("course_preview_entries").num_columns(4).striped(true).show(
                    ui,
                    |ui| {
                        ui.label("#");
                        ui.label(tr!(text, "course-column-title"));
                        ui.label("☆");
                        ui.label(tr!(text, "course-notes"));
                        ui.end_row();
                        for (i, entry) in preview.entry_previews.iter().enumerate() {
                            ui.label(format!("{}", i + 1));
                            let title = if entry.title.is_empty() {
                                tr!(text, "common-no-title")
                            } else {
                                entry.title.clone()
                            };
                            if entry.resolved {
                                ui.label(&title);
                            } else {
                                ui.colored_label(
                                    egui::Color32::GRAY,
                                    tr!(
                                        text,
                                        "course-preview-missing",
                                        "title" => title.as_str()
                                    ),
                                );
                            }
                            ui.label(&entry.play_level);
                            ui.label(format!("{}", entry.total_notes));
                            ui.end_row();
                        }
                    },
                );
            });
        });
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum DebugLogFilter {
    All,
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl DebugLogFilter {
    const ALL: [Self; 6] =
        [Self::All, Self::Error, Self::Warn, Self::Info, Self::Debug, Self::Trace];

    fn label(self, text: Localizer) -> String {
        match self {
            Self::All => tr!(text, "debug-log-filter-all"),
            Self::Error => tr!(text, "debug-log-filter-error"),
            Self::Warn => tr!(text, "debug-log-filter-warn"),
            Self::Info => tr!(text, "debug-log-filter-info"),
            Self::Debug => tr!(text, "debug-log-filter-debug"),
            Self::Trace => tr!(text, "debug-log-filter-trace"),
        }
    }

    const fn minimum_level(self) -> Option<TracingLogLevel> {
        match self {
            Self::All => None,
            Self::Error => Some(TracingLogLevel::Error),
            Self::Warn => Some(TracingLogLevel::Warn),
            Self::Info => Some(TracingLogLevel::Info),
            Self::Debug => Some(TracingLogLevel::Debug),
            Self::Trace => Some(TracingLogLevel::Trace),
        }
    }

    pub(super) fn allows(self, level: TracingLogLevel) -> bool {
        self.minimum_level().is_none_or(|minimum| level >= minimum)
    }
}

pub(super) fn log_level_color(level: TracingLogLevel) -> egui::Color32 {
    match level {
        TracingLogLevel::Trace => egui::Color32::GRAY,
        TracingLogLevel::Debug => egui::Color32::LIGHT_BLUE,
        TracingLogLevel::Info => egui::Color32::LIGHT_GREEN,
        TracingLogLevel::Warn => egui::Color32::YELLOW,
        TracingLogLevel::Error => egui::Color32::LIGHT_RED,
    }
}

pub(super) fn localized_log_message(entry: &LogEntry, text: Localizer) -> String {
    if entry.message.is_empty() { tr!(text, "debug-log-no-message") } else { entry.message.clone() }
}

pub(super) fn format_log_entry(entry: &LogEntry, text: Localizer) -> String {
    format!("[{}] {} {}", entry.level.as_str(), entry.target, localized_log_message(entry, text))
}

/// FPS / フレーム時間 / シーン / 解像度 / tracing ログを表示するデバッグパネル。
pub(super) fn build_debug_panel(
    ctx: &egui::Context,
    open: &mut bool,
    info: &DebugInfo,
    log_buffer: &LogBuffer,
    debug_log_filter: &mut DebugLogFilter,
    debug_log_autoscroll: &mut bool,
    text: Localizer,
) {
    localized_sized_panel_window(
        "debug_panel",
        tr!(text, "debug-title"),
        ctx,
        open,
        620.0,
        500.0,
        egui::pos2(16.0, 140.0),
    )
    .show(ctx, |ui| {
        scrollable_window_content(ui, |ui| {
            let dt = ctx.input(|i| i.stable_dt);
            egui::Grid::new("debug_grid").num_columns(2).show(ui, |ui| {
                ui.label("FPS");
                ui.label(info.current_fps.to_string());
                ui.end_row();
                ui.label(tr!(text, "debug-frame-time"));
                ui.label(format!("{:.2} ms", dt * 1000.0));
                ui.end_row();
                ui.label(tr!(text, "debug-scene"));
                ui.label(info.scene);
                ui.end_row();
                ui.label(tr!(text, "debug-resolution"));
                ui.label(format!("{} x {}", info.width, info.height));
                ui.end_row();
                ui.label(tr!(text, "debug-present-mode"));
                ui.label(
                    info.effective_present_mode
                        .map_or_else(|| tr!(text, "debug-uninitialized"), ToString::to_string),
                );
                ui.end_row();
                ui.label(tr!(text, "debug-max-frame-latency"));
                ui.label(info.maximum_frame_latency.map_or_else(
                    || tr!(text, "debug-uninitialized"),
                    |latency| latency.to_string(),
                ));
                ui.end_row();
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(tr!(text, "debug-log"));
                egui::ComboBox::from_id_salt("debug_log_filter")
                    .selected_text(debug_log_filter.label(text))
                    .show_ui(ui, |ui| {
                        for filter in DebugLogFilter::ALL {
                            ui.selectable_value(debug_log_filter, filter, filter.label(text));
                        }
                    });
                ui.checkbox(debug_log_autoscroll, tr!(text, "debug-log-autoscroll"));
            });

            let entries = log_buffer.snapshot();
            let visible_entries = entries
                .iter()
                .filter(|entry| debug_log_filter.allows(entry.level))
                .collect::<Vec<_>>();
            let mut copy_requested = false;
            let mut clear_requested = false;
            ui.horizontal(|ui| {
                ui.small(tr!(
                    text,
                    "debug-log-count",
                    "visible" => visible_entries.len(),
                    "total" => entries.len()
                ));
                if ui.button(tr!(text, "common-copy")).clicked() {
                    copy_requested = true;
                }
                if ui.button(tr!(text, "debug-log-clear")).clicked() {
                    clear_requested = true;
                }
            });

            egui::ScrollArea::vertical()
                .id_salt("debug_log_scroll")
                .max_height(300.0)
                .auto_shrink([false, false])
                .stick_to_bottom(*debug_log_autoscroll)
                .show(ui, |ui| {
                    if visible_entries.is_empty() {
                        ui.weak(tr!(text, "debug-log-empty"));
                    }
                    for entry in visible_entries {
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(log_level_color(entry.level), entry.level.as_str());
                            ui.weak(format!("{}:", entry.target));
                            ui.label(localized_log_message(entry, text));
                        });
                    }
                });

            if copy_requested {
                let text = entries
                    .iter()
                    .filter(|entry| debug_log_filter.allows(entry.level))
                    .map(|entry| format_log_entry(entry, text))
                    .collect::<Vec<_>>()
                    .join("\n");
                ui.ctx().copy_text(text);
            }
            if clear_requested {
                log_buffer.clear();
            }
        });
    });
}

pub(super) fn build_update_dialog(
    ctx: &egui::Context,
    dialog: UpdateDialog<'_>,
    text: Localizer,
) -> Option<UpdateDialogAction> {
    let mut action = None;
    egui::Window::new(tr!(text, "update-title"))
        .id(egui::Id::new("update_dialog"))
        .collapsible(false)
        .resizable(false)
        .default_width(440.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| match dialog {
            UpdateDialog::Available(candidate) => {
                ui.heading(tr!(
                    text,
                    "update-available",
                    "version" => candidate.version.as_str()
                ));
                ui.label(tr!(
                    text,
                    "update-current-version",
                    "version" => current_version()
                ));
                if let Some(published_at) = candidate.published_at.as_deref() {
                    ui.label(tr!(text, "update-published-at", "date" => published_at));
                }
                if let Some(asset) = candidate.asset.as_ref() {
                    ui.label(tr!(text, "update-asset", "asset" => asset.name.as_str()));
                    ui.label(update_asset_kind_label(asset.kind, text));
                } else {
                    ui.label(tr!(text, "update-no-compatible-asset"));
                }
                if let Some(body) = release_body_excerpt(&candidate.body) {
                    ui.separator();
                    ui.label(body);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    let can_update = candidate.asset.is_some();
                    if ui
                        .add_enabled(can_update, egui::Button::new(tr!(text, "update-button")))
                        .clicked()
                    {
                        action = Some(UpdateDialogAction::Update);
                    }
                    if ui.button(tr!(text, "update-not-now")).clicked() {
                        action = Some(UpdateDialogAction::NotNow);
                    }
                    if ui.button(tr!(text, "update-skip-release")).clicked() {
                        action = Some(UpdateDialogAction::SkipRelease);
                    }
                });
                if ui.button(tr!(text, "update-open-release-page")).clicked() {
                    action = Some(UpdateDialogAction::OpenReleasePage);
                }
            }
            UpdateDialog::Downloading(candidate) => {
                ui.heading(tr!(
                    text,
                    "update-downloading",
                    "version" => candidate.version.as_str()
                ));
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(tr!(text, "update-fetching-asset"));
                });
                if let Some(asset) = candidate.asset.as_ref() {
                    ui.label(tr!(text, "update-asset", "asset" => asset.name.as_str()));
                }
            }
            UpdateDialog::Error { message, candidate } => {
                ui.heading(tr!(text, "update-check-failed"));
                ui.colored_label(egui::Color32::LIGHT_RED, message);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr!(text, "common-close")).clicked() {
                        action = Some(UpdateDialogAction::NotNow);
                    }
                    if candidate.is_some()
                        && ui.button(tr!(text, "update-open-release-page")).clicked()
                    {
                        action = Some(UpdateDialogAction::OpenReleasePage);
                    }
                });
            }
            UpdateDialog::UpToDate => {
                ui.heading(tr!(text, "update-up-to-date"));
                ui.label(tr!(
                    text,
                    "update-current-version",
                    "version" => current_version()
                ));
                if ui.button(tr!(text, "common-close")).clicked() {
                    action = Some(UpdateDialogAction::NotNow);
                }
            }
        });
    action
}

pub(super) fn release_body_excerpt(body: &str) -> Option<String> {
    let mut lines =
        body.lines().map(str::trim).filter(|line| !line.is_empty()).take(6).collect::<Vec<_>>();
    if lines.is_empty() {
        return None;
    }
    let mut text = lines.join("\n");
    const MAX_LEN: usize = 480;
    if text.len() > MAX_LEN {
        text = text.chars().take(MAX_LEN).collect();
        text.push_str("...");
    } else if body.lines().filter(|line| !line.trim().is_empty()).count() > lines.len() {
        text.push_str("\n...");
    }
    lines.clear();
    Some(text)
}

pub(super) fn update_asset_kind_label(kind: UpdateAssetKind, text: Localizer) -> String {
    match kind {
        UpdateAssetKind::WindowsInstaller => tr!(text, "update-kind-windows-installer"),
        UpdateAssetKind::MacosAppZip => tr!(text, "update-kind-macos-manual"),
        UpdateAssetKind::Other => tr!(text, "update-kind-manual"),
    }
}
