use super::*;

/// 本体設定パネルからのアクション要求。
pub(super) struct SettingsPanelActions {
    pub(super) save: bool,
    pub(super) obs_enabled_changed: bool,
    pub(super) save_profile: bool,
    pub(super) check_update: bool,
    pub(super) rescan: bool,
    pub(super) song_scan_requests: Vec<SongScanRequest>,
    pub(super) table_fetch_urls: Vec<String>,
    pub(super) score_import_request: Option<ScoreImportRequest>,
    /// 音声出力(cpal ストリーム)を現在の設定で開き直す要求。
    pub(super) apply_audio: bool,
}

pub(super) struct SettingsPanelState<'a> {
    pub(super) new_root_path: &'a mut String,
    pub(super) add_root_error: &'a mut String,
    pub(super) new_table_url: &'a mut String,
    pub(super) add_table_error: &'a mut String,
    pub(super) score_import_path: &'a mut String,
    pub(super) score_import_kind: &'a mut ScoreImportKind,
    pub(super) score_import_device_type: &'a mut InputDeviceKind,
    pub(super) score_import_status: &'a str,
    pub(super) score_import_error: &'a str,
    pub(super) audio_device_picker: &'a mut AudioDevicePickerState,
    pub(super) obs_scene_picker: &'a mut ObsScenePickerState,
    pub(super) obs_connection_status: &'a crate::obs::ObsConnectionStatus,
    pub(super) connected_gamepads: &'a [crate::input::gamepad::ConnectedGamepad],
}

#[derive(Default)]
pub(super) struct ObsScenePickerState {
    busy: bool,
    scenes: Vec<String>,
    message: String,
    error: String,
    receiver: Option<std::sync::mpsc::Receiver<Result<crate::obs::ObsSceneList, String>>>,
}

impl ObsScenePickerState {
    fn poll(&mut self, text: Localizer) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        let Ok(result) = receiver.try_recv() else {
            return;
        };
        self.receiver = None;
        self.busy = false;
        match result {
            Ok(list) => {
                self.scenes = list.scenes;
                self.error.clear();
                self.message = tr!(
                    text,
                    "settings-obs-scenes-loaded",
                    "count" => self.scenes.len(),
                    "version" => list.version,
                    "recording" => if list.recording_active { "ON" } else { "OFF" }
                );
            }
            Err(error) => {
                self.message.clear();
                self.error = error;
            }
        }
    }

    fn start_load(&mut self, config: crate::config::app_config::ObsConfig) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.receiver = Some(receiver);
        self.busy = true;
        self.message.clear();
        self.error.clear();
        tokio::spawn(async move {
            let result =
                crate::obs::load_scenes(config).await.map_err(|error| format!("{error:#}"));
            let _ = sender.send(result);
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SettingsListAction {
    MoveUp(usize),
    MoveDown(usize),
    MoveTo { from: usize, to: usize },
    Remove(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SettingsDragList {
    SongRoots,
    TableSources,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SettingsDragPayload {
    list: SettingsDragList,
    index: usize,
}

const SETTINGS_LIST_BUTTONS_WIDTH: f32 = 224.0;
const SETTINGS_TABLE_LIST_BUTTONS_WIDTH: f32 = 224.0;
const SETTINGS_TABLE_ENABLED_WIDTH: f32 = 56.0;
const SETTINGS_LIST_DRAG_HANDLE_WIDTH: f32 = 28.0;
const SETTINGS_LIST_MIN_LABEL_WIDTH: f32 = 96.0;

pub(super) fn apply_settings_list_action<T>(items: &mut Vec<T>, action: SettingsListAction) {
    match action {
        SettingsListAction::MoveUp(index) if index > 0 && index < items.len() => {
            items.swap(index - 1, index);
        }
        SettingsListAction::MoveDown(index) if index + 1 < items.len() => {
            items.swap(index, index + 1);
        }
        SettingsListAction::MoveTo { from, to }
            if from < items.len() && to < items.len() && from != to =>
        {
            let item = items.remove(from);
            items.insert(to.min(items.len()), item);
        }
        SettingsListAction::Remove(index) if index < items.len() => {
            items.remove(index);
        }
        _ => {}
    }
}

pub(super) fn settings_list_label_width(ui: &egui::Ui) -> f32 {
    (ui.available_width() - SETTINGS_LIST_BUTTONS_WIDTH).max(SETTINGS_LIST_MIN_LABEL_WIDTH)
}

pub(super) fn settings_list_label(ui: &mut egui::Ui, text: &str, width: f32) {
    ui.add_sized([width, ui.spacing().interact_size.y], egui::Label::new(text).truncate())
        .on_hover_text(text);
}

pub(super) fn settings_drag_handle(
    ui: &mut egui::Ui,
    payload: SettingsDragPayload,
    text: Localizer,
) {
    let response = ui.add_sized(
        [SETTINGS_LIST_DRAG_HANDLE_WIDTH, ui.spacing().interact_size.y],
        egui::Button::new(egui::RichText::new("≡").size(18.0)).sense(egui::Sense::drag()),
    );
    response.dnd_set_drag_payload(payload);
    response
        .on_hover_cursor(egui::CursorIcon::Grab)
        .on_hover_text(tr!(text, "settings-drag-to-reorder"));
}

pub(super) fn settings_drag_ghost(
    ctx: &egui::Context,
    id: egui::Id,
    label: &str,
    label_width: f32,
    show_song_options: bool,
    text: Localizer,
) {
    let Some(pointer_pos) = ctx.pointer_interact_pos() else {
        return;
    };
    egui::Area::new(id)
        .order(egui::Order::Tooltip)
        .interactable(false)
        .fixed_pos(pointer_pos + egui::vec2(10.0, 8.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [SETTINGS_LIST_DRAG_HANDLE_WIDTH, ui.spacing().interact_size.y],
                        egui::Label::new(egui::RichText::new("≡").size(18.0)),
                    );
                    settings_list_label(ui, label, label_width);
                });
                if show_song_options {
                    let mut enabled = true;
                    let mut recursive = true;
                    ui.horizontal(|ui| {
                        ui.add_enabled(
                            false,
                            egui::Checkbox::new(&mut enabled, tr!(text, "common-enabled")),
                        );
                        ui.add_enabled(
                            false,
                            egui::Checkbox::new(
                                &mut recursive,
                                tr!(text, "settings-song-recursive"),
                            ),
                        );
                    });
                }
            });
        });
}

/// `AppConfig` を編集する本体設定パネル。
pub(super) fn build_settings_panel(
    ctx: &egui::Context,
    window: &Window,
    open: &mut bool,
    config: &mut AppConfig,
    profile: &mut ProfileConfig,
    show_fps: &mut bool,
    editable: bool,
    difficulty_tables: &[DifficultyTableRecord],
    text: Localizer,
    state: SettingsPanelState<'_>,
) -> SettingsPanelActions {
    let mut save_clicked = false;
    let mut obs_enabled_changed = false;
    let mut save_profile = false;
    let mut rescan_clicked = false;
    let mut check_update_clicked = false;
    let mut song_scan_requests = Vec::new();
    let mut table_fetch_urls = Vec::new();
    let mut score_import_request = None;
    let mut apply_audio = false;
    localized_sized_panel_window(
        "app_settings_panel",
        tr!(text, "settings-app-title"),
        ctx,
        open,
        440.0,
        520.0,
        egui::pos2(16.0, 320.0),
    )
    .show(
        ctx,
        |ui| {
            if !editable {
                ui.label(tr!(text, "settings-disabled-during-play"));
                ui.separator();
            }
            ui.add_enabled_ui(editable, |ui| {
                scrollable_window_content(ui, |ui| {
                egui::CollapsingHeader::new(tr!(text, "settings-song-folders"))
                    .id_salt("settings_song_folders")
                    .default_open(true)
                    .show(ui, |ui| {
                        let mut root_action = None;
                        let root_len = config.songs.roots.len();
                        for (index, root) in config.songs.roots.iter_mut().enumerate() {
                            ui.push_id(index, |ui| {
                                let label_width = (settings_list_label_width(ui)
                                    - SETTINGS_LIST_DRAG_HANDLE_WIDTH)
                                    .max(SETTINGS_LIST_MIN_LABEL_WIDTH);
                                let (_, dropped) = ui.dnd_drop_zone::<SettingsDragPayload, _>(
                                    egui::Frame::NONE,
                                    |ui| {
                                        let payload = SettingsDragPayload {
                                            list: SettingsDragList::SongRoots,
                                            index,
                                        };
                                        ui.horizontal(|ui| {
                                            settings_drag_handle(ui, payload, text);
                                            settings_list_label(ui, &root.path, label_width);
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui.button(tr!(text, "common-delete")).clicked() {
                                                        root_action =
                                                            Some(SettingsListAction::Remove(index));
                                                    }
                                                    if ui
                                                        .add_enabled(
                                                            root.enabled,
                                                            egui::Button::new(tr!(text, "common-reload")),
                                                        )
                                                        .clicked()
                                                    {
                                                        song_scan_requests.push(SongScanRequest {
                                                            roots: vec![root.clone()],
                                                            force: true,
                                                            label: "egui song reload".to_string(),
                                                        });
                                                    }
                                                    if ui
                                                        .add_enabled(
                                                            index + 1 < root_len,
                                                            egui::Button::new(tr!(text, "common-move-down")),
                                                        )
                                                        .clicked()
                                                    {
                                                        root_action = Some(
                                                            SettingsListAction::MoveDown(index),
                                                        );
                                                    }
                                                    if ui
                                                        .add_enabled(
                                                            index > 0,
                                                            egui::Button::new(tr!(text, "common-move-up")),
                                                        )
                                                        .clicked()
                                                    {
                                                        root_action =
                                                            Some(SettingsListAction::MoveUp(index));
                                                    }
                                                },
                                            );
                                        });
                                        ui.horizontal(|ui| {
                                            ui.checkbox(
                                                &mut root.enabled,
                                                tr!(text, "common-enabled"),
                                            );
                                            ui.checkbox(
                                                &mut root.recursive,
                                                tr!(text, "settings-song-recursive"),
                                            );
                                        });
                                    },
                                );
                                if egui::DragAndDrop::payload::<SettingsDragPayload>(ui.ctx())
                                    .is_some_and(|payload| {
                                        payload.list == SettingsDragList::SongRoots
                                            && payload.index == index
                                    })
                                {
                                    settings_drag_ghost(
                                        ui.ctx(),
                                        egui::Id::new(("settings_song_root_ghost", index)),
                                        &root.path,
                                        label_width,
                                        true,
                                        text,
                                    );
                                }
                                if let Some(payload) = dropped
                                    && payload.list == SettingsDragList::SongRoots
                                {
                                    root_action = Some(SettingsListAction::MoveTo {
                                        from: payload.index,
                                        to: index,
                                    });
                                }
                                ui.separator();
                            });
                        }
                        if let Some(action) = root_action {
                            apply_settings_list_action(&mut config.songs.roots, action);
                        }
                        if config.songs.roots.is_empty() {
                            ui.label(tr!(text, "settings-song-folders-empty"));
                        }
                        ui.horizontal(|ui| {
                            ui.label(tr!(text, "common-path"));
                            ui.add(
                                egui::TextEdit::singleline(state.new_root_path)
                                    .desired_width(240.0)
                                    .hint_text("/path/to/bms"),
                            );
                        });
                        ui.horizontal(|ui| {
                            if ui.button(tr!(text, "common-choose-folder")).clicked()
                                && let Some(folder) = rfd::FileDialog::new().pick_folder()
                            {
                                *state.new_root_path = folder.to_string_lossy().into_owned();
                                state.add_root_error.clear();
                            }
                            if ui.button(tr!(text, "common-add")).clicked() {
                                let path = state.new_root_path.trim().to_string();
                                if path.is_empty() {
                                    *state.add_root_error =
                                        tr!(text, "settings-song-path-required");
                                } else {
                                    match add_song_root_entry(
                                        &mut config.songs.roots,
                                        &path,
                                        true,
                                        true,
                                    ) {
                                        Ok(()) => {
                                            song_scan_requests.push(SongScanRequest {
                                                roots: vec![PathEntry {
                                                    path,
                                                    enabled: true,
                                                    recursive: true,
                                                }],
                                                force: false,
                                                label: "egui song load".to_string(),
                                            });
                                            save_clicked = true;
                                            state.new_root_path.clear();
                                            state.add_root_error.clear();
                                        }
                                        Err(error) => *state.add_root_error = error.to_string(),
                                    }
                                }
                            }
                        });
                        if !state.add_root_error.is_empty() {
                            ui.colored_label(egui::Color32::RED, state.add_root_error.as_str());
                        }
                        if ui.button(tr!(text, "settings-library-rescan")).clicked() {
                            rescan_clicked = true;
                        }
                        ui.label(tr!(text, "settings-song-scan-help"));
                    });

                egui::CollapsingHeader::new(tr!(text, "settings-scan-title"))
                    .id_salt("settings_scan")
                    .show(ui, |ui| {
                    ui.checkbox(
                        &mut config.scan.follow_symlinks,
                        tr!(text, "settings-scan-follow-symlinks"),
                    );
                    ui.checkbox(
                        &mut config.scan.skip_hidden,
                        tr!(text, "settings-scan-skip-hidden"),
                    );
                    ui.checkbox(
                        &mut config.scan.auto_rescan_on_startup,
                        tr!(text, "settings-scan-on-startup"),
                    );
                    ui.checkbox(
                        &mut config.scan.rescan_missing_files,
                        tr!(text, "settings-scan-remove-missing"),
                    );
                });

                egui::CollapsingHeader::new(tr!(text, "settings-select-title"))
                    .id_salt("settings_select")
                    .show(ui, |ui| {
                    ui.add(
                        egui::Slider::new(
                            &mut config.select.scroll_duration_low_ms,
                            2..=1000,
                        )
                        .text(tr!(text, "settings-select-scroll-initial")),
                    );
                    ui.add(
                        egui::Slider::new(
                            &mut config.select.scroll_duration_high_ms,
                            1..=1000,
                        )
                        .text(tr!(text, "settings-select-scroll-repeat")),
                    );
                    ui.label(tr!(text, "settings-select-scroll-help"));
                });

                egui::CollapsingHeader::new(tr!(text, "settings-tables-title"))
                    .id_salt("settings_tables")
                    .show(ui, |ui| {
                    ui.checkbox(
                        &mut config.tables.auto_fetch_on_startup,
                        tr!(text, "settings-tables-fetch-on-startup"),
                    );
                    let mut table_action = None;
                    let table_len = config.tables.sources.len();
                    for (index, source) in config.tables.sources.iter_mut().enumerate() {
                        ui.push_id(("table_source", index), |ui| {
                            let source_label =
                                difficulty_table_source_label(&source.url, difficulty_tables);
                            let label_width = (ui.available_width()
                                - SETTINGS_TABLE_LIST_BUTTONS_WIDTH
                                - SETTINGS_TABLE_ENABLED_WIDTH
                                - SETTINGS_LIST_DRAG_HANDLE_WIDTH)
                                .max(64.0);
                            let (_, dropped) = ui.dnd_drop_zone::<SettingsDragPayload, _>(
                                egui::Frame::NONE,
                                |ui| {
                                    let payload = SettingsDragPayload {
                                        list: SettingsDragList::TableSources,
                                        index,
                                    };
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [
                                                SETTINGS_TABLE_ENABLED_WIDTH,
                                                ui.spacing().interact_size.y,
                                            ],
                                            egui::Checkbox::new(
                                                &mut source.enabled,
                                                tr!(text, "common-enabled"),
                                            ),
                                        );
                                        settings_drag_handle(ui, payload, text);
                                        settings_list_label(ui, &source_label, label_width);
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.button(tr!(text, "common-delete")).clicked() {
                                                    table_action =
                                                        Some(SettingsListAction::Remove(index));
                                                }
                                                if ui.button(tr!(text, "common-fetch")).clicked() {
                                                    table_fetch_urls.push(source.url.clone());
                                                }
                                                if ui
                                                    .add_enabled(
                                                        index + 1 < table_len,
                                                        egui::Button::new(tr!(text, "common-move-down")),
                                                    )
                                                    .clicked()
                                                {
                                                    table_action =
                                                        Some(SettingsListAction::MoveDown(index));
                                                }
                                                if ui
                                                    .add_enabled(
                                                        index > 0,
                                                        egui::Button::new(tr!(text, "common-move-up")),
                                                    )
                                                    .clicked()
                                                {
                                                    table_action =
                                                        Some(SettingsListAction::MoveUp(index));
                                                }
                                            },
                                        );
                                    });
                                },
                            );
                            if egui::DragAndDrop::payload::<SettingsDragPayload>(ui.ctx())
                                .is_some_and(|payload| {
                                    payload.list == SettingsDragList::TableSources
                                        && payload.index == index
                                })
                            {
                                settings_drag_ghost(
                                    ui.ctx(),
                                    egui::Id::new(("settings_table_source_ghost", index)),
                                    &source_label,
                                    label_width,
                                    false,
                                    text,
                                );
                            }
                            if let Some(payload) = dropped
                                && payload.list == SettingsDragList::TableSources
                            {
                                table_action = Some(SettingsListAction::MoveTo {
                                    from: payload.index,
                                    to: index,
                                });
                            }
                        });
                    }
                    if let Some(action) = table_action {
                        apply_settings_list_action(&mut config.tables.sources, action);
                    }
                    if config.tables.sources.is_empty() {
                        ui.label(tr!(text, "settings-tables-empty"));
                    }
                    let enabled_table_urls: Vec<String> = config
                        .tables
                        .sources
                        .iter()
                        .filter(|source| source.enabled)
                        .map(|source| source.url.clone())
                        .collect();
                    if ui
                        .add_enabled(
                            !enabled_table_urls.is_empty(),
                            egui::Button::new(tr!(text, "settings-tables-fetch-enabled")),
                        )
                        .clicked()
                    {
                        table_fetch_urls.extend(enabled_table_urls);
                    }
                    ui.horizontal(|ui| {
                        ui.label("URL");
                        ui.add(
                            egui::TextEdit::singleline(state.new_table_url)
                                .desired_width(300.0)
                                .hint_text("https://.../header.json"),
                        );
                    });
                    if ui.button(tr!(text, "common-add")).clicked() {
                        let url = state.new_table_url.trim().to_string();
                        match add_difficulty_table_source(
                            &mut config.tables.sources,
                            &url,
                            text,
                        ) {
                            Ok(()) => {
                                table_fetch_urls.push(url);
                                save_clicked = true;
                                state.new_table_url.clear();
                                state.add_table_error.clear();
                            }
                            Err(error) => *state.add_table_error = error,
                        }
                    }
                    if !state.add_table_error.is_empty() {
                        ui.colored_label(egui::Color32::RED, state.add_table_error.as_str());
                    }
                    ui.label(tr!(text, "settings-tables-help"));
                });

                egui::CollapsingHeader::new(tr!(text, "settings-downloads-title"))
                    .id_salt("settings_downloads")
                    .show(ui, |ui| {
                    ui.label(tr!(text, "settings-downloads-disclaimer"));
                    ui.separator();
                    ui.checkbox(
                        &mut config.downloads.ipfs_enabled,
                        tr!(text, "settings-downloads-ipfs-enable"),
                    );
                    ui.horizontal(|ui| {
                        ui.label(tr!(text, "settings-downloads-ipfs-api-url"));
                        ui.add(
                            egui::TextEdit::singleline(&mut config.downloads.ipfs_api_url)
                                .desired_width(360.0)
                                .hint_text("http://127.0.0.1:5001/"),
                        );
                    });
                    ui.label(tr!(
                        text,
                        "settings-downloads-ipfs-help",
                        "cid" => "{cid}"
                    ));
                    ui.separator();
                    ui.checkbox(
                        &mut config.downloads.http_enabled,
                        tr!(text, "settings-downloads-http-enable"),
                    );
                    ui.horizontal(|ui| {
                        ui.label(tr!(text, "settings-downloads-http-api-url"));
                        ui.add(
                            egui::TextEdit::singleline(&mut config.downloads.http_api_url)
                                .desired_width(360.0)
                                .hint_text("https://example.com/package/{md5}"),
                        );
                    });
                    ui.label(tr!(
                        text,
                        "settings-downloads-http-help",
                        "md5" => "{md5}",
                        "sha256" => "{sha256}"
                    ));
                    ui.label(tr!(text, "settings-downloads-save-path"));
                });

                build_score_import_section(
                    ui,
                    state.score_import_path,
                    state.score_import_kind,
                    state.score_import_device_type,
                    state.score_import_status,
                    state.score_import_error,
                    &mut score_import_request,
                    text,
                );

                egui::CollapsingHeader::new(tr!(text, "settings-audio-title"))
                    .id_salt("settings_audio")
                    .show(ui, |ui| {
                    let available_audio_backends = crate::audio::available_audio_backends();
                    if !available_audio_backends.contains(&config.audio.backend) {
                        config.audio.backend = AudioBackend::Auto;
                    }
                    egui::ComboBox::new("audio_backend", tr!(text, "settings-backend"))
                        .selected_text(audio_backend_label(&config.audio.backend, text))
                        .show_ui(ui, |ui| {
                            for backend in &available_audio_backends {
                                ui.selectable_value(
                                    &mut config.audio.backend,
                                    backend.clone(),
                                    audio_backend_label(backend, text),
                                );
                            }
                        });
                    if config.audio.backend == AudioBackend::Wasapi {
                        egui::ComboBox::new(
                            "audio_output_mode",
                            tr!(text, "settings-audio-output-mode"),
                        )
                            .selected_text(audio_output_mode_label(&config.audio.output_mode, text))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut config.audio.output_mode,
                                    AudioOutputMode::Shared,
                                    tr!(text, "settings-audio-output-mode-shared"),
                                );
                                ui.selectable_value(
                                    &mut config.audio.output_mode,
                                    AudioOutputMode::SharedLowLatency,
                                    tr!(text, "settings-audio-output-mode-low-latency"),
                                );
                            });
                        if config.audio.output_mode == AudioOutputMode::SharedLowLatency {
                            ui.label(tr!(text, "settings-audio-low-latency-help"));
                        }
                    }
                    let sample_rate_text =
                        if config.audio.sample_rate_mode == AudioSampleRateMode::Auto {
                            tr!(text, "settings-audio-auto-driver-default")
                        } else {
                            audio_sample_rate_label(config.audio.sample_rate)
                        };
                    egui::ComboBox::new(
                        "audio_sample_rate",
                        tr!(text, "settings-audio-sample-rate"),
                    )
                        .selected_text(sample_rate_text)
                        .show_ui(ui, |ui| {
                            let is_auto =
                                config.audio.sample_rate_mode == AudioSampleRateMode::Auto;
                            if ui
                                .selectable_label(
                                    is_auto,
                                    tr!(text, "settings-audio-auto-driver-default"),
                                )
                                .clicked()
                            {
                                config.audio.sample_rate_mode = AudioSampleRateMode::Auto;
                            }
                            for hz in [44_100u32, 48_000, 96_000, 192_000, 384_000] {
                                let selected = config.audio.sample_rate_mode
                                    == AudioSampleRateMode::Fixed
                                    && config.audio.sample_rate == hz;
                                if ui
                                    .selectable_label(selected, audio_sample_rate_label(hz))
                                    .clicked()
                                {
                                    config.audio.sample_rate_mode = AudioSampleRateMode::Fixed;
                                    config.audio.sample_rate = hz;
                                }
                            }
                        });
                    egui::ComboBox::new(
                        "audio_buffer_mode",
                        tr!(text, "settings-audio-buffer-mode"),
                    )
                        .selected_text(audio_buffer_size_mode_label(
                            &config.audio.buffer_size_mode,
                            text,
                        ))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut config.audio.buffer_size_mode,
                                AudioBufferSizeMode::Auto,
                                tr!(text, "common-auto"),
                            );
                            ui.selectable_value(
                                &mut config.audio.buffer_size_mode,
                                AudioBufferSizeMode::Fixed,
                                tr!(text, "common-fixed"),
                            );
                        });
                    if config.audio.buffer_size_mode == AudioBufferSizeMode::Fixed {
                        ui.add(
                            egui::Slider::new(&mut config.audio.buffer_size, 32..=4096)
                                .text(tr!(text, "settings-audio-buffer-frames")),
                        );
                        ui.horizontal(|ui| {
                            ui.label(tr!(text, "settings-audio-presets"));
                            for frames in [32u32, 48, 64, 96, 128, 256] {
                                if ui.button(frames.to_string()).clicked() {
                                    config.audio.buffer_size = frames;
                                    config.audio.buffer_size_mode = AudioBufferSizeMode::Fixed;
                                }
                            }
                        });
                    }
                    // ASIO 以外は安価なのでバックエンド変更時に自動列挙する。
                    // ASIO はドライバ初期化を伴い得るため、更新ボタンでのみ列挙する。
                    let backend = config.audio.backend.clone();
                    if backend != AudioBackend::Asio
                        && state.audio_device_picker.backend.as_ref() != Some(&backend)
                    {
                        state.audio_device_picker.names =
                            crate::audio::list_output_devices(&backend);
                        state.audio_device_picker.backend = Some(backend);
                    }

                    ui.horizontal(|ui| {
                        if ui.button(tr!(text, "settings-audio-refresh-devices")).clicked() {
                            state.audio_device_picker.names =
                                crate::audio::list_output_devices(&config.audio.backend);
                            state.audio_device_picker.backend = Some(config.audio.backend.clone());
                        }
                        ui.label(tr!(
                            text,
                            "common-count",
                            "count" => state.audio_device_picker.names.len()
                        ));
                    });

                    if config.audio.backend == AudioBackend::Asio {
                        egui::ComboBox::new(
                            "audio_asio_driver",
                            tr!(text, "settings-audio-asio-driver"),
                        )
                            .selected_text(if config.audio.asio_driver.is_empty() {
                                tr!(text, "common-unspecified")
                            } else {
                                config.audio.asio_driver.clone()
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut config.audio.asio_driver,
                                    String::new(),
                                    tr!(text, "common-unspecified"),
                                );
                                for name in state.audio_device_picker.names.iter() {
                                    ui.selectable_value(
                                        &mut config.audio.asio_driver,
                                        name.clone(),
                                        name,
                                    );
                                }
                            });
                    } else {
                        egui::ComboBox::new(
                            "audio_output_device",
                            tr!(text, "settings-audio-output-device"),
                        )
                            .selected_text(if config.audio.output_device.is_empty() {
                                tr!(text, "common-default")
                            } else {
                                config.audio.output_device.clone()
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut config.audio.output_device,
                                    String::new(),
                                    tr!(text, "common-default"),
                                );
                                for name in state.audio_device_picker.names.iter() {
                                    ui.selectable_value(
                                        &mut config.audio.output_device,
                                        name.clone(),
                                        name,
                                    );
                                }
                            });
                    }
                    if config.audio.backend == AudioBackend::Asio {
                        egui::ComboBox::new(
                            "audio_output_channel",
                            tr!(text, "settings-audio-output-channel"),
                        )
                            .selected_text(audio_channel_pair_label(config.audio.output_channel_pair))
                            .show_ui(ui, |ui| {
                                for pair in 0u32..6 {
                                    ui.selectable_value(
                                        &mut config.audio.output_channel_pair,
                                        pair,
                                        audio_channel_pair_label(pair),
                                    );
                                }
                            });
                        ui.label(tr!(text, "settings-audio-channel-help"));
                    }
                    ui.label(tr!(text, "settings-audio-asio-buffer-help"));
                    if ui.button(tr!(text, "settings-audio-apply")).clicked() {
                        apply_audio = true;
                    }
                    ui.label(tr!(text, "settings-audio-apply-help"));
                });

                egui::CollapsingHeader::new(tr!(text, "settings-video-title"))
                    .id_salt("settings_video")
                    .show(ui, |ui| {
                    egui::ComboBox::new(
                        "video_window_mode",
                        tr!(text, "settings-video-window-mode"),
                    )
                        .selected_text(window_mode_label(&config.video.mode, text))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut config.video.mode,
                                WindowMode::Windowed,
                                tr!(text, "settings-windowed"),
                            );
                            ui.selectable_value(
                                &mut config.video.mode,
                                WindowMode::BorderlessFullscreen,
                                tr!(text, "settings-borderless-fullscreen"),
                            );
                            ui.selectable_value(
                                &mut config.video.mode,
                                WindowMode::ExclusiveFullscreen,
                                tr!(text, "settings-exclusive-fullscreen"),
                            );
                        });
                    ui.add(
                        egui::Slider::new(&mut config.video.width, 640..=3840)
                            .text(tr!(text, "settings-video-width")),
                    );
                    ui.add(
                        egui::Slider::new(&mut config.video.height, 480..=2160)
                            .text(tr!(text, "settings-video-height")),
                    );
                    egui::ComboBox::new(
                        "video_internal_resolution",
                        tr!(text, "settings-video-internal-resolution"),
                    )
                    .selected_text(internal_resolution_mode_label(
                        &config.video.internal_resolution,
                        text,
                    ))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut config.video.internal_resolution,
                            InternalResolutionModeConfig::Native,
                            tr!(text, "settings-video-internal-resolution-native"),
                        );
                        ui.selectable_value(
                            &mut config.video.internal_resolution,
                            InternalResolutionModeConfig::Skin,
                            tr!(text, "settings-video-internal-resolution-skin"),
                        );
                    });
                    let available_monitors = window.available_monitors().collect::<Vec<_>>();
                    let selected_monitor = if config.video.monitor_name.is_empty() {
                        tr!(text, "settings-video-primary-monitor")
                    } else if available_monitors
                        .iter()
                        .any(|monitor| monitor_config_name(monitor) == config.video.monitor_name)
                    {
                        config.video.monitor_name.clone()
                    } else {
                        tr!(
                            text,
                            "settings-video-monitor-disconnected",
                            "name" => config.video.monitor_name.as_str()
                        )
                    };
                    egui::ComboBox::new(
                        "video_monitor",
                        tr!(text, "settings-video-monitor"),
                    )
                        .selected_text(selected_monitor)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut config.video.monitor_name,
                                String::new(),
                                tr!(text, "settings-video-primary-monitor"),
                            );
                            for monitor in &available_monitors {
                                let name = monitor_config_name(monitor);
                                ui.selectable_value(
                                    &mut config.video.monitor_name,
                                    name.clone(),
                                    name,
                                );
                            }
                        });
                    egui::ComboBox::new(
                        "video_vsync_mode",
                        tr!(text, "settings-video-vsync-mode"),
                    )
                        .selected_text(vsync_mode_label(&config.video.vsync_mode))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut config.video.vsync_mode,
                                VsyncModeConfig::Vsync,
                                vsync_mode_label(&VsyncModeConfig::Vsync),
                            );
                            ui.selectable_value(
                                &mut config.video.vsync_mode,
                                VsyncModeConfig::AdaptiveVsync,
                                vsync_mode_label(&VsyncModeConfig::AdaptiveVsync),
                            );
                            ui.selectable_value(
                                &mut config.video.vsync_mode,
                                VsyncModeConfig::VsyncOff,
                                vsync_mode_label(&VsyncModeConfig::VsyncOff),
                            );
                            ui.selectable_value(
                                &mut config.video.vsync_mode,
                                VsyncModeConfig::FastVsync,
                                vsync_mode_label(&VsyncModeConfig::FastVsync),
                            );
                        });
                    ui.add(
                        egui::DragValue::new(&mut config.video.target_fps)
                            .range(0..=u32::MAX)
                            .speed(1.0)
                            .suffix(" FPS"),
                    );
                    ui.label(tr!(text, "settings-video-target-fps-unlimited"));
                    if ui.checkbox(show_fps, tr!(text, "settings-show-fps")).changed() {
                        profile.ui.show_fps = *show_fps;
                        save_profile = true;
                    }
                    ui.add(
                        egui::Slider::new(&mut config.video.frame_limit_in_background, 1..=120)
                            .text(tr!(text, "settings-video-background-fps")),
                    );
                    let available_renderer_backends = available_renderer_backends();
                    if !available_renderer_backends.contains(&config.video.renderer) {
                        config.video.renderer = RendererBackend::Auto;
                    }
                    egui::ComboBox::new(
                        "video_renderer",
                        tr!(text, "settings-video-renderer"),
                    )
                        .selected_text(renderer_backend_label(&config.video.renderer, text))
                        .show_ui(ui, |ui| {
                            for backend in &available_renderer_backends {
                                ui.selectable_value(
                                    &mut config.video.renderer,
                                    backend.clone(),
                                    renderer_backend_label(backend, text),
                                );
                            }
                        });
                    ui.label(tr!(text, "settings-video-apply-help"));
                });

                egui::CollapsingHeader::new(tr!(text, "settings-screenshot-title"))
                    .id_salt("settings_screenshot")
                    .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(tr!(text, "settings-screenshot-directory"));
                        ui.add(
                            egui::TextEdit::singleline(&mut config.screenshot.dir)
                                .desired_width(300.0)
                                .hint_text("screenshots"),
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui.button(tr!(text, "common-choose-folder")).clicked()
                            && let Some(dir) = rfd::FileDialog::new().pick_folder()
                        {
                            config.screenshot.dir = dir.to_string_lossy().into_owned();
                        }
                        ui.checkbox(
                            &mut config.screenshot.copy_to_clipboard,
                            tr!(text, "settings-screenshot-copy-clipboard"),
                        );
                    });
                });

                obs_enabled_changed |= build_obs_settings_section(
                    ui,
                    config,
                    state.obs_scene_picker,
                    state.obs_connection_status,
                    text,
                );

                egui::CollapsingHeader::new(tr!(text, "settings-updates-title"))
                    .id_salt("settings_updates")
                    .show(ui, |ui| {
                    ui.checkbox(
                        &mut config.updates.enabled,
                        tr!(text, "settings-updates-notifications"),
                    );
                    ui.checkbox(
                        &mut config.updates.check_on_startup,
                        tr!(text, "settings-updates-on-startup"),
                    );
                    egui::ComboBox::new(
                        "updates_channel",
                        tr!(text, "settings-updates-channel"),
                    )
                        .selected_text(update_channel_label(config.updates.channel))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut config.updates.channel,
                                UpdateChannelConfig::Stable,
                                update_channel_label(UpdateChannelConfig::Stable),
                            );
                            ui.selectable_value(
                                &mut config.updates.channel,
                                UpdateChannelConfig::Prerelease,
                                update_channel_label(UpdateChannelConfig::Prerelease),
                            );
                        });
                    if config.updates.skipped_version.is_empty() {
                        ui.label(tr!(text, "settings-updates-no-skipped-release"));
                    } else {
                        ui.horizontal(|ui| {
                            ui.label(tr!(
                                text,
                                "settings-updates-skipping",
                                "version" => config.updates.skipped_version.as_str()
                            ));
                            if ui.button(tr!(text, "common-clear")).clicked() {
                                config.updates.skipped_version.clear();
                                save_clicked = true;
                            }
                        });
                    }
                    if ui.button(tr!(text, "settings-updates-check")).clicked() {
                        check_update_clicked = true;
                    }
                });

                egui::CollapsingHeader::new("Discord").show(ui, |ui| {
                    ui.checkbox(&mut config.discord.enabled, "Rich Presence");
                    ui.horizontal(|ui| {
                        ui.label("Application ID");
                        ui.add(
                            egui::TextEdit::singleline(&mut config.discord.application_id)
                                .desired_width(260.0)
                                .hint_text(tr!(text, "settings-discord-default-hint")),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Large image key");
                        ui.add(
                            egui::TextEdit::singleline(&mut config.discord.large_image_key)
                                .desired_width(160.0)
                                .hint_text("bmz"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Large image text");
                        ui.add(
                            egui::TextEdit::singleline(&mut config.discord.large_image_text)
                                .desired_width(220.0)
                                .hint_text("BMZ Player"),
                        );
                    });
                    ui.checkbox(
                        &mut config.discord.show_song_details,
                        tr!(text, "settings-discord-song-details"),
                    );
                    ui.label(tr!(text, "settings-discord-default-help"));
                });

                egui::CollapsingHeader::new(tr!(text, "settings-input-title"))
                    .id_salt("settings_input")
                    .show(ui, |ui| {
                    egui::ComboBox::new("input_backend", tr!(text, "settings-backend"))
                        .selected_text(input_backend_label(&config.input.backend, text))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut config.input.backend,
                                InputBackendKind::Auto,
                                input_backend_label(&InputBackendKind::Auto, text),
                            );
                            ui.selectable_value(
                                &mut config.input.backend,
                                InputBackendKind::Winit,
                                input_backend_label(&InputBackendKind::Winit, text),
                            );
                            ui.selectable_value(
                                &mut config.input.backend,
                                InputBackendKind::RawInput,
                                input_backend_label(&InputBackendKind::RawInput, text),
                            );
                            ui.selectable_value(
                                &mut config.input.backend,
                                InputBackendKind::Hid,
                                input_backend_label(&InputBackendKind::Hid, text),
                            );
                            ui.selectable_value(
                                &mut config.input.backend,
                                InputBackendKind::Midi,
                                input_backend_label(&InputBackendKind::Midi, text),
                            );
                        });
                    egui::ComboBox::new(
                        "gamepad_backend",
                        tr!(text, "settings-input-gamepad-backend"),
                    )
                        .selected_text(gamepad_backend_label(&config.input.gamepad_backend, text))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut config.input.gamepad_backend,
                                GamepadBackendKind::Auto,
                                gamepad_backend_label(&GamepadBackendKind::Auto, text),
                            );
                            ui.selectable_value(
                                &mut config.input.gamepad_backend,
                                GamepadBackendKind::Gilrs,
                                gamepad_backend_label(&GamepadBackendKind::Gilrs, text),
                            );
                            ui.selectable_value(
                                &mut config.input.gamepad_backend,
                                GamepadBackendKind::GameInput,
                                gamepad_backend_label(&GamepadBackendKind::GameInput, text),
                            );
                        });
                    ui.checkbox(
                        &mut config.input.keyboard_enabled,
                        tr!(text, "settings-input-keyboard"),
                    );
                    ui.checkbox(
                        &mut config.input.gamepad_enabled,
                        tr!(text, "settings-input-gamepad"),
                    );
                    ui.checkbox(
                        &mut config.input.midi_enabled,
                        tr!(text, "settings-input-midi-unimplemented"),
                    );
                    ui.label(tr!(text, "settings-input-backend-help"));
                    ui.separator();
                    ui.label(tr!(text, "settings-input-controller-assignment"));
                    ui.label(tr!(
                        text,
                        "settings-input-connected-count",
                        "count" => state.connected_gamepads.iter().filter(|pad| pad.is_connected).count()
                    ));
                    if state.connected_gamepads.is_empty() {
                        ui.label(tr!(text, "settings-input-no-gamepads"));
                    } else {
                        for pad in state.connected_gamepads {
                            let status = if pad.is_connected {
                                tr!(text, "common-connected")
                            } else {
                                tr!(text, "common-disconnected")
                            };
                            ui.label(format!(
                                "#{} {} ({})",
                                pad.backend_id, pad.name, status
                            ));
                        }
                    }
                    for (slot_index, label) in [
                        (0usize, tr!(text, "settings-input-controller-1p")),
                        (1usize, tr!(text, "settings-input-controller-2p")),
                    ]
                    {
                        let current = config.input.gamepad_slot_device_ids[slot_index].as_deref();
                        let selected_text = match current {
                            Some(stable_id) => state
                                .connected_gamepads
                                .iter()
                                .find(|pad| pad.stable_id == stable_id)
                                .map(|pad| format!("#{} {}", pad.backend_id, pad.name))
                                .unwrap_or_else(|| {
                                    let end = stable_id.len().min(20);
                                    tr!(
                                        text,
                                        "settings-input-device-disconnected",
                                        "device" => format!("{}...", &stable_id[..end])
                                    )
                                }),
                            None => config.input.gamepad_slot_gilrs_ids[slot_index]
                                .and_then(|id| {
                                    state
                                        .connected_gamepads
                                        .iter()
                                        .find(|pad| pad.backend_id == id)
                                        .map(|pad| {
                                            tr!(
                                                text,
                                                "settings-input-legacy-device",
                                                "device" => format!("#{} {}", pad.backend_id, pad.name)
                                            )
                                        })
                                })
                                .unwrap_or_else(|| tr!(text, "settings-input-auto-order")),
                        };
                        egui::ComboBox::from_label(label)
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_value(
                                        &mut config.input.gamepad_slot_device_ids[slot_index],
                                        None,
                                        tr!(text, "settings-input-auto-order"),
                                    )
                                    .clicked()
                                {
                                    config.input.gamepad_slot_gilrs_ids[slot_index] = None;
                                }
                                for pad in state.connected_gamepads {
                                    if ui
                                        .selectable_value(
                                            &mut config.input.gamepad_slot_device_ids[slot_index],
                                            Some(pad.stable_id.clone()),
                                            format!("#{} {}", pad.backend_id, pad.name),
                                        )
                                        .clicked()
                                    {
                                        config.input.gamepad_slot_gilrs_ids[slot_index] = None;
                                    }
                                }
                            });
                    }
                    ui.horizontal(|ui| {
                        if ui.button(tr!(text, "settings-input-auto-assign")).clicked() {
                            let connected: Vec<String> = state
                                .connected_gamepads
                                .iter()
                                .filter(|pad| pad.is_connected)
                                .map(|pad| pad.stable_id.clone())
                                .collect();
                            config.input.gamepad_slot_device_ids[0] = connected.first().cloned();
                            config.input.gamepad_slot_device_ids[1] = connected.get(1).cloned();
                            config.input.gamepad_slot_gilrs_ids = [None, None];
                        }
                        if ui.button(tr!(text, "settings-input-swap")).clicked() {
                            config.input.gamepad_slot_device_ids.swap(0, 1);
                            config.input.gamepad_slot_gilrs_ids.swap(0, 1);
                        }
                        if ui.button(tr!(text, "settings-input-clear-assignment")).clicked() {
                            config.input.gamepad_slot_device_ids = [None, None];
                            config.input.gamepad_slot_gilrs_ids = [None, None];
                        }
                    });
                    ui.label(tr!(text, "settings-input-assignment-help"));
                });

                egui::CollapsingHeader::new(tr!(text, "settings-logging-title"))
                    .id_salt("settings_logging")
                    .show(ui, |ui| {
                    egui::ComboBox::new(
                        "logging_level",
                        tr!(text, "settings-logging-level"),
                    )
                        .selected_text(log_level_label(&config.logging.level))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut config.logging.level,
                                LogLevel::Trace,
                                log_level_label(&LogLevel::Trace),
                            );
                            ui.selectable_value(
                                &mut config.logging.level,
                                LogLevel::Debug,
                                log_level_label(&LogLevel::Debug),
                            );
                            ui.selectable_value(
                                &mut config.logging.level,
                                LogLevel::Info,
                                log_level_label(&LogLevel::Info),
                            );
                            ui.selectable_value(
                                &mut config.logging.level,
                                LogLevel::Warn,
                                log_level_label(&LogLevel::Warn),
                            );
                            ui.selectable_value(
                                &mut config.logging.level,
                                LogLevel::Error,
                                log_level_label(&LogLevel::Error),
                            );
                        });
                    ui.checkbox(
                        &mut config.logging.file_logging,
                        tr!(text, "settings-logging-file-unimplemented"),
                    );
                    ui.label(tr!(text, "settings-logging-help"));
                });

                ui.separator();
                if ui.button(tr!(text, "settings-save")).clicked() {
                    save_clicked = true;
                }
                });
            });
        });
    SettingsPanelActions {
        save: save_clicked || apply_audio,
        obs_enabled_changed,
        save_profile,
        check_update: check_update_clicked,
        rescan: rescan_clicked,
        song_scan_requests,
        table_fetch_urls,
        score_import_request,
        apply_audio,
    }
}

pub(super) fn difficulty_table_source_label(
    source_url: &str,
    difficulty_tables: &[DifficultyTableRecord],
) -> String {
    difficulty_tables
        .iter()
        .find(|table| table.source_url == source_url && !table.name.trim().is_empty())
        .map(|table| format!("{} ({source_url})", table.name))
        .unwrap_or_else(|| source_url.to_string())
}

pub(super) fn build_obs_settings_section(
    ui: &mut egui::Ui,
    config: &mut AppConfig,
    state: &mut ObsScenePickerState,
    connection_status: &crate::obs::ObsConnectionStatus,
    text: Localizer,
) -> bool {
    state.poll(text);
    let mut enabled_changed = false;
    egui::CollapsingHeader::new("OBS WebSocket").id_salt("settings_obs").show(ui, |ui| {
        enabled_changed =
            ui.checkbox(&mut config.obs.enabled, tr!(text, "settings-obs-enabled")).changed();
        let (status_label, status_color) =
            obs_connection_status_label(connection_status.kind, text);
        ui.horizontal(|ui| {
            ui.label(tr!(text, "settings-obs-connection-status"));
            ui.colored_label(status_color, status_label);
            if let Some(retry_in_ms) = connection_status.retry_in_ms {
                ui.label(tr!(
                    text,
                    "settings-obs-next-retry",
                    "seconds" => retry_in_ms as f64 / 1000.0
                ));
            }
        });
        if let Some(detail) = &connection_status.detail {
            ui.label(detail);
        }
        if let Some(error) = &connection_status.last_error {
            ui.colored_label(egui::Color32::RED, error);
        }
        ui.horizontal(|ui| {
            ui.label(tr!(text, "settings-obs-host"));
            ui.add(
                egui::TextEdit::singleline(&mut config.obs.host)
                    .desired_width(180.0)
                    .hint_text("localhost"),
            );
            ui.label(tr!(text, "settings-obs-port"));
            ui.add(egui::DragValue::new(&mut config.obs.port).range(0..=65535));
        });
        ui.horizontal(|ui| {
            ui.label(tr!(text, "settings-obs-password"));
            ui.add(
                egui::TextEdit::singleline(&mut config.obs.password)
                    .desired_width(220.0)
                    .password(true),
            );
        });
        egui::ComboBox::new("obs_recording_mode", tr!(text, "settings-obs-recording-mode"))
            .selected_text(obs_recording_mode_label(config.obs.recording_mode, text))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut config.obs.recording_mode,
                    ObsRecordingMode::KeepAll,
                    obs_recording_mode_label(ObsRecordingMode::KeepAll, text),
                );
                ui.selectable_value(
                    &mut config.obs.recording_mode,
                    ObsRecordingMode::OnScreenshot,
                    obs_recording_mode_label(ObsRecordingMode::OnScreenshot, text),
                );
                ui.selectable_value(
                    &mut config.obs.recording_mode,
                    ObsRecordingMode::OnReplay,
                    obs_recording_mode_label(ObsRecordingMode::OnReplay, text),
                );
            });
        ui.add(
            egui::Slider::new(&mut config.obs.record_stop_wait_ms, 0..=10_000)
                .text(tr!(text, "settings-obs-stop-delay")),
        );

        ui.horizontal(|ui| {
            if ui
                .add_enabled(!state.busy, egui::Button::new(tr!(text, "settings-obs-load-scenes")))
                .clicked()
            {
                state.start_load(config.obs.clone());
            }
            if state.busy {
                ui.label(tr!(text, "common-loading"));
            }
        });
        if !state.message.is_empty() {
            ui.label(state.message.as_str());
        }
        if !state.error.is_empty() {
            ui.colored_label(egui::Color32::RED, state.error.as_str());
        }

        ui.separator();
        ui.strong(tr!(text, "settings-obs-state-settings"));
        egui::Grid::new("obs_state_mapping_grid").striped(true).show(ui, |ui| {
            ui.label(tr!(text, "settings-obs-state"));
            ui.label(tr!(text, "settings-obs-scene"));
            ui.label(tr!(text, "settings-obs-action"));
            ui.end_row();
            for event in crate::obs::ObsEventKey::ALL {
                let key = event.config_key();
                ui.label(obs_event_label(event, text));

                let mut scene = config.obs.scenes.get(key).cloned().unwrap_or_default();
                let selected_scene = if scene.is_empty() {
                    tr!(text, "settings-obs-no-change")
                } else {
                    scene.clone()
                };
                egui::ComboBox::from_id_salt(("obs_scene", key))
                    .selected_text(selected_scene)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut scene,
                            String::new(),
                            tr!(text, "settings-obs-no-change"),
                        );
                        if !scene.is_empty() && !state.scenes.iter().any(|name| name == &scene) {
                            let current_scene = scene.clone();
                            ui.selectable_value(&mut scene, current_scene.clone(), current_scene);
                        }
                        for candidate in &state.scenes {
                            ui.selectable_value(&mut scene, candidate.clone(), candidate);
                        }
                    });
                if scene.is_empty() {
                    config.obs.scenes.remove(key);
                } else {
                    config.obs.scenes.insert(key.to_string(), scene);
                }

                let mut action = config.obs.actions.get(key).copied().unwrap_or_default();
                egui::ComboBox::from_id_salt(("obs_action", key))
                    .selected_text(obs_action_label(action, text))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut action,
                            ObsActionConfig::None,
                            obs_action_label(ObsActionConfig::None, text),
                        );
                        ui.selectable_value(
                            &mut action,
                            ObsActionConfig::StartRecord,
                            obs_action_label(ObsActionConfig::StartRecord, text),
                        );
                        ui.selectable_value(
                            &mut action,
                            ObsActionConfig::StopRecord,
                            obs_action_label(ObsActionConfig::StopRecord, text),
                        );
                    });
                if action == ObsActionConfig::None {
                    config.obs.actions.remove(key);
                } else {
                    config.obs.actions.insert(key.to_string(), action);
                }
                ui.end_row();
            }
        });
    });
    enabled_changed
}

pub(super) fn obs_connection_status_label(
    kind: crate::obs::ObsConnectionStatusKind,
    text: Localizer,
) -> (String, egui::Color32) {
    match kind {
        crate::obs::ObsConnectionStatusKind::Disabled => {
            (tr!(text, "settings-obs-disabled"), egui::Color32::GRAY)
        }
        crate::obs::ObsConnectionStatusKind::Connecting => {
            (tr!(text, "common-connecting"), egui::Color32::from_rgb(120, 190, 255))
        }
        crate::obs::ObsConnectionStatusKind::WaitingForServer => {
            (tr!(text, "settings-obs-waiting"), egui::Color32::from_rgb(225, 185, 75))
        }
        crate::obs::ObsConnectionStatusKind::Connected => {
            (tr!(text, "common-connected"), egui::Color32::GREEN)
        }
        crate::obs::ObsConnectionStatusKind::Reconnecting => {
            (tr!(text, "settings-obs-reconnecting"), egui::Color32::YELLOW)
        }
        crate::obs::ObsConnectionStatusKind::AuthenticationFailed => {
            (tr!(text, "settings-obs-auth-failed"), egui::Color32::RED)
        }
        crate::obs::ObsConnectionStatusKind::ConfigurationError => {
            (tr!(text, "settings-obs-config-error"), egui::Color32::RED)
        }
    }
}

pub(super) fn obs_recording_mode_label(mode: ObsRecordingMode, text: Localizer) -> String {
    match mode {
        ObsRecordingMode::KeepAll => tr!(text, "settings-obs-recording-keep-all"),
        ObsRecordingMode::OnScreenshot => tr!(text, "settings-obs-recording-screenshot"),
        ObsRecordingMode::OnReplay => tr!(text, "settings-obs-recording-replay"),
    }
}

pub(super) fn obs_action_label(action: ObsActionConfig, text: Localizer) -> String {
    match action {
        ObsActionConfig::None => tr!(text, "settings-obs-action-none"),
        ObsActionConfig::StartRecord => tr!(text, "settings-obs-action-start"),
        ObsActionConfig::StopRecord => tr!(text, "settings-obs-action-stop"),
    }
}

pub(super) fn obs_event_label(event: crate::obs::ObsEventKey, text: Localizer) -> String {
    match event {
        crate::obs::ObsEventKey::MusicSelect => tr!(text, "settings-obs-event-select"),
        crate::obs::ObsEventKey::Decide => tr!(text, "settings-obs-event-decide"),
        crate::obs::ObsEventKey::Play => tr!(text, "settings-obs-event-play"),
        crate::obs::ObsEventKey::PlayEnded => tr!(text, "settings-obs-event-play-ended"),
        crate::obs::ObsEventKey::Result => tr!(text, "settings-obs-event-result"),
        crate::obs::ObsEventKey::CourseResult => tr!(text, "settings-obs-event-course-result"),
    }
}

pub(super) fn build_score_import_section(
    ui: &mut egui::Ui,
    path: &mut String,
    kind: &mut ScoreImportKind,
    device_type: &mut InputDeviceKind,
    status: &str,
    error: &str,
    request: &mut Option<ScoreImportRequest>,
    text: Localizer,
) {
    egui::CollapsingHeader::new(tr!(text, "settings-score-import-title"))
        .id_salt("settings_score_import")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("DB");
                ui.add(
                    egui::TextEdit::singleline(path)
                        .desired_width(260.0)
                        .hint_text("score.db / scoredatalog.db / LR2 score db"),
                );
            });
            ui.horizontal(|ui| {
                if ui.button(tr!(text, "common-choose-file")).clicked()
                    && let Some(file) =
                        rfd::FileDialog::new().add_filter("SQLite DB", &["db"]).pick_file()
                {
                    *path = file.to_string_lossy().into_owned();
                }
                egui::ComboBox::from_id_salt("score_import_kind")
                    .selected_text(kind.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            kind,
                            ScoreImportKind::Lr2,
                            ScoreImportKind::Lr2.label(),
                        );
                        ui.selectable_value(
                            kind,
                            ScoreImportKind::Beatoraja,
                            ScoreImportKind::Beatoraja.label(),
                        );
                        ui.selectable_value(
                            kind,
                            ScoreImportKind::Lr2Oraja,
                            ScoreImportKind::Lr2Oraja.label(),
                        );
                        ui.selectable_value(
                            kind,
                            ScoreImportKind::Lr2OrajaDx,
                            ScoreImportKind::Lr2OrajaDx.label(),
                        );
                    });
            });
            ui.horizontal(|ui| {
                ui.label(tr!(text, "settings-score-import-device"));
                ui.selectable_value(
                    device_type,
                    InputDeviceKind::Keyboard,
                    tr!(text, "settings-input-keyboard"),
                );
                ui.selectable_value(
                    device_type,
                    InputDeviceKind::Controller,
                    tr!(text, "settings-score-import-controller"),
                );
            });
            if ui.button(tr!(text, "settings-score-import-button")).clicked() {
                let trimmed = path.trim();
                if trimmed.is_empty() {
                    *request = None;
                } else {
                    *request = Some(ScoreImportRequest {
                        path: PathBuf::from(trimmed),
                        kind: *kind,
                        device_type: *device_type,
                    });
                }
            }
            if !status.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_GREEN, status);
            }
            if !error.is_empty() {
                ui.colored_label(egui::Color32::RED, error);
            }
        });
}

pub(super) fn audio_backend_label(backend: &AudioBackend, text: Localizer) -> String {
    match backend {
        AudioBackend::Auto => tr!(text, "common-auto-select"),
        AudioBackend::Wasapi => "WASAPI".to_owned(),
        AudioBackend::Asio => "ASIO".to_owned(),
        AudioBackend::CoreAudio => "Core Audio".to_owned(),
        AudioBackend::Alsa => "ALSA".to_owned(),
        AudioBackend::Pulse => "PulseAudio".to_owned(),
        AudioBackend::PipeWire => "PipeWire".to_owned(),
    }
}

pub(super) fn audio_output_mode_label(mode: &AudioOutputMode, text: Localizer) -> String {
    match mode {
        AudioOutputMode::Shared => tr!(text, "settings-audio-output-mode-shared"),
        AudioOutputMode::SharedLowLatency => {
            tr!(text, "settings-audio-output-mode-low-latency")
        }
    }
}

pub(super) fn audio_buffer_size_mode_label(mode: &AudioBufferSizeMode, text: Localizer) -> String {
    match mode {
        AudioBufferSizeMode::Auto => tr!(text, "common-auto"),
        AudioBufferSizeMode::Fixed => tr!(text, "common-fixed"),
    }
}

/// 出力チャンネルペア(0 始まり)を "1-2ch" のような表示文字列にする。
pub(super) fn audio_channel_pair_label(pair: u32) -> String {
    let left = pair * 2 + 1;
    format!("{}-{}ch", left, left + 1)
}

/// サンプルレート(Hz)を "48kHz" / "44.1kHz" のような表示文字列にする。
pub(super) fn audio_sample_rate_label(hz: u32) -> String {
    if hz.is_multiple_of(1000) {
        format!("{}kHz", hz / 1000)
    } else {
        format!("{:.1}kHz", hz as f64 / 1000.0)
    }
}

pub(super) fn update_channel_label(channel: UpdateChannelConfig) -> &'static str {
    match channel {
        UpdateChannelConfig::Stable => "Stable",
        UpdateChannelConfig::Prerelease => "Prerelease",
    }
}

pub(super) fn window_mode_label(mode: &WindowMode, text: Localizer) -> String {
    match mode {
        WindowMode::Windowed => tr!(text, "settings-windowed"),
        WindowMode::BorderlessFullscreen => tr!(text, "settings-borderless-fullscreen"),
        WindowMode::ExclusiveFullscreen => tr!(text, "settings-exclusive-fullscreen"),
    }
}

pub(super) fn renderer_backend_label(backend: &RendererBackend, text: Localizer) -> String {
    match backend {
        RendererBackend::Auto => tr!(text, "common-auto-select"),
        RendererBackend::Vulkan => "Vulkan".to_owned(),
        RendererBackend::Metal => "Metal".to_owned(),
        RendererBackend::Dx12 => "DirectX 12".to_owned(),
        RendererBackend::Gl => "OpenGL".to_owned(),
    }
}

pub(super) fn internal_resolution_mode_label(
    mode: &InternalResolutionModeConfig,
    text: Localizer,
) -> String {
    match mode {
        InternalResolutionModeConfig::Native => {
            tr!(text, "settings-video-internal-resolution-native")
        }
        InternalResolutionModeConfig::Skin => {
            tr!(text, "settings-video-internal-resolution-skin")
        }
    }
}

pub(super) fn available_renderer_backends() -> Vec<RendererBackend> {
    bmz_render::available_wgpu_backends()
        .into_iter()
        .map(|backend| match backend {
            bmz_render::WgpuBackend::Auto => RendererBackend::Auto,
            bmz_render::WgpuBackend::Vulkan => RendererBackend::Vulkan,
            bmz_render::WgpuBackend::Metal => RendererBackend::Metal,
            bmz_render::WgpuBackend::Dx12 => RendererBackend::Dx12,
            bmz_render::WgpuBackend::Gl => RendererBackend::Gl,
        })
        .collect()
}

pub(super) fn vsync_mode_label(mode: &VsyncModeConfig) -> &'static str {
    match mode {
        VsyncModeConfig::Vsync => "Vsync (Fifo)",
        VsyncModeConfig::AdaptiveVsync => "Adaptive Vsync (Fifo Relaxed)",
        VsyncModeConfig::VsyncOff => "Vsync Off (Immediate)",
        VsyncModeConfig::FastVsync => "Fast Vsync (Mailbox)",
    }
}

pub(super) fn input_backend_label(backend: &InputBackendKind, text: Localizer) -> String {
    match backend {
        InputBackendKind::Auto => tr!(text, "common-auto-select"),
        InputBackendKind::Winit => "winit".to_owned(),
        InputBackendKind::RawInput => tr!(text, "settings-input-raw-input"),
        InputBackendKind::Hid => tr!(text, "settings-input-hid-unimplemented"),
        InputBackendKind::Midi => tr!(text, "settings-input-midi-unimplemented"),
    }
}

pub(super) fn gamepad_backend_label(backend: &GamepadBackendKind, text: Localizer) -> String {
    match backend {
        GamepadBackendKind::Auto => tr!(text, "common-auto-select"),
        GamepadBackendKind::Gilrs => "gilrs".to_owned(),
        GamepadBackendKind::GameInput => tr!(text, "settings-input-gameinput"),
    }
}

pub(super) fn log_level_label(level: &LogLevel) -> &'static str {
    match level {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    }
}

pub(super) fn add_difficulty_table_source(
    sources: &mut Vec<DifficultyTableSource>,
    url: &str,
    text: Localizer,
) -> Result<(), String> {
    if url.is_empty() {
        return Err(tr!(text, "settings-tables-url-required"));
    }
    if sources.iter().any(|source| source.url == url) {
        return Err(tr!(text, "settings-tables-url-duplicate"));
    }
    sources.push(DifficultyTableSource { url: url.to_string(), enabled: true });
    Ok(())
}
