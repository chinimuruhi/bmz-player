use super::*;

const LOCAL_COURSE_SOURCE: &str = "bmz:local";
pub(super) const SELECT_COURSE_MAX_ENTRIES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectCourseEntryError {
    Full,
    UnknownMode,
    ModeMismatch { expected: KeyMode, actual: KeyMode },
}

impl WinitApp {
    pub(super) fn begin_select_course_builder(&mut self) {
        if self.select.course_builder.is_some() {
            return;
        }
        let courses = match self.boot.library_db.list_courses_by_source(LOCAL_COURSE_SOURCE) {
            Ok(courses) => courses,
            Err(error) => {
                tracing::error!(%error, "failed to load local courses before course creation");
                self.show_left_overlay_toast(
                    Localizer::new(self.boot.profile_config.ui.locale())
                        .text("toast-select-course-builder-start-failed"),
                );
                return;
            }
        };
        let key =
            next_local_course_key(courses.iter().map(|course| course.definition.key.as_str()));
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let state = SelectCourseBuilderState {
            definition: bmz_core::course::CourseDefinition {
                key,
                title: text.text("select-course-builder-default-name"),
                kind: bmz_core::course::CourseKind::Course,
                entries: Vec::new(),
                constraints: bmz_core::course::CourseConstraints::default(),
                trophies: Vec::new(),
                release: true,
            },
            key_mode: None,
            return_folder_stack: std::mem::take(&mut self.select.folder_stack),
            return_selected_index_stack: std::mem::take(&mut self.select.selected_index_stack),
            return_selected_index: self.select.selected_index,
        };
        self.set_search_mode(false);
        self.select.course_builder = Some(state);
        self.select.selected_index = 0;
        self.reload_select_items();
        self.restart_select_bar_timer_without_scroll(Instant::now());
        self.play_system_sound(crate::system_sound::SoundType::FolderOpen);
        self.show_left_overlay_toast(text.text("toast-select-course-builder-started"));
        tracing::info!("started LR2-style course creation from song select");
    }

    pub(super) fn add_chart_to_select_course(&mut self, row: &SelectChartRow) {
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let Some(chart) = row.chart.as_ref() else {
            self.show_left_overlay_toast(text.text("toast-select-course-builder-local-only"));
            return;
        };
        let current_len = self
            .select
            .course_builder
            .as_ref()
            .map(|builder| builder.definition.entries.len())
            .unwrap_or(0);
        let expected_mode =
            self.select.course_builder.as_ref().and_then(|builder| builder.key_mode);
        let candidate_mode = KeyMode::from_str_opt(&chart.mode);
        match validate_select_course_entry(current_len, expected_mode, candidate_mode) {
            Ok(mode) => {
                let entry_count = {
                    let Some(builder) = self.select.course_builder.as_mut() else {
                        return;
                    };
                    builder.key_mode = Some(mode);
                    builder.definition.entries.push(bmz_core::course::CourseEntry {
                        title_hint: chart.title.clone(),
                        md5: Some(hash_to_hex(&chart.md5)),
                        sha256: Some(hash_to_hex(&chart.sha256)),
                        chart_id: Some(chart.chart_id),
                    });
                    builder.definition.entries.len()
                };
                let mut args = FluentArgs::new();
                args.set("title", chart.title.clone());
                args.set("count", entry_count as i64);
                args.set("max", SELECT_COURSE_MAX_ENTRIES as i64);
                self.show_left_overlay_toast(
                    text.format("toast-select-course-builder-added", &args),
                );
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                tracing::info!(
                    chart_id = chart.chart_id,
                    title = %chart.title,
                    entries = entry_count,
                    "added chart to select course"
                );
            }
            Err(SelectCourseEntryError::Full) => {
                self.show_left_overlay_toast(text.text("toast-select-course-builder-full"));
            }
            Err(SelectCourseEntryError::UnknownMode) => {
                self.show_left_overlay_toast(text.text("toast-select-course-builder-mode-unknown"));
            }
            Err(SelectCourseEntryError::ModeMismatch { expected, actual }) => {
                let mut args = FluentArgs::new();
                args.set("expected", expected.as_str());
                args.set("actual", actual.as_str());
                self.show_left_overlay_toast(
                    text.format("toast-select-course-builder-mode-mismatch", &args),
                );
            }
        }
    }

    pub(super) fn apply_select_course_builder_action(&mut self, action: SelectCourseBuilderAction) {
        match action {
            SelectCourseBuilderAction::Remove(index) => {
                let Some(builder) = self.select.course_builder.as_mut() else {
                    return;
                };
                if index < builder.definition.entries.len() {
                    builder.definition.entries.remove(index);
                    if builder.definition.entries.is_empty() {
                        builder.key_mode = None;
                    }
                    self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                }
            }
            SelectCourseBuilderAction::Undo => self.undo_select_course_entry(),
            SelectCourseBuilderAction::Save => self.save_select_course_builder(),
            SelectCourseBuilderAction::Cancel => self.cancel_select_course_builder(),
        }
    }

    pub(super) fn undo_select_course_entry(&mut self) {
        let Some(builder) = self.select.course_builder.as_mut() else {
            return;
        };
        if builder.definition.entries.pop().is_some() {
            if builder.definition.entries.is_empty() {
                builder.key_mode = None;
            }
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
        }
    }

    pub(super) fn cancel_select_course_builder(&mut self) {
        if self.finish_select_course_builder(None) {
            let text = Localizer::new(self.boot.profile_config.ui.locale());
            self.show_left_overlay_toast(text.text("toast-select-course-builder-cancelled"));
            tracing::info!("cancelled select course creation");
        }
    }

    pub(super) fn show_select_course_builder_chart_required(&mut self) {
        self.show_left_overlay_toast(
            Localizer::new(self.boot.profile_config.ui.locale())
                .text("toast-select-course-builder-chart-required"),
        );
    }

    fn save_select_course_builder(&mut self) {
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let Some(mut definition) =
            self.select.course_builder.as_ref().map(|builder| builder.definition.clone())
        else {
            return;
        };
        definition.title = definition.title.trim().to_string();
        if definition.title.is_empty() {
            definition.title = text.text("select-course-builder-default-name");
        }
        match self.save_local_course(&definition) {
            Ok(course_id) => {
                self.finish_select_course_builder(Some(course_id));
                self.show_left_overlay_toast(text.text("toast-select-course-builder-saved"));
                tracing::info!(course_id, title = %definition.title, "saved select course");
            }
            Err(error) => {
                tracing::error!(%error, "failed to save select course");
                let mut args = FluentArgs::new();
                args.set("error", error.to_string());
                self.show_left_overlay_toast(
                    text.format("toast-select-course-builder-save-failed", &args),
                );
            }
        }
    }

    fn finish_select_course_builder(&mut self, selected_course_id: Option<i64>) -> bool {
        let Some(state) = self.select.course_builder.take() else {
            return false;
        };
        self.set_search_mode(false);
        self.select.folder_stack = state.return_folder_stack;
        self.select.selected_index_stack = state.return_selected_index_stack;
        self.select.selected_index = state.return_selected_index;
        // reload 前の一覧は作成中に開いていた別フォルダのものなので、そこから
        // 選択キーを復元せず、開始時に保存した index をそのまま使う。
        self.select.select_items.clear();
        self.reload_select_items();
        if let Some(course_id) = selected_course_id
            && let Some(index) = self.select.select_items.iter().position(
                |item| matches!(item, SelectItem::Course(row) if row.course_id == course_id),
            )
        {
            self.select.selected_index = index;
        }
        self.sync_selected_play_mode();
        self.restart_select_bar_timer_without_scroll(Instant::now());
        self.play_system_sound(crate::system_sound::SoundType::FolderClose);
        true
    }
}

fn validate_select_course_entry(
    entry_count: usize,
    expected_mode: Option<KeyMode>,
    candidate_mode: Option<KeyMode>,
) -> Result<KeyMode, SelectCourseEntryError> {
    if entry_count >= SELECT_COURSE_MAX_ENTRIES {
        return Err(SelectCourseEntryError::Full);
    }
    let candidate_mode = candidate_mode.ok_or(SelectCourseEntryError::UnknownMode)?;
    if let Some(expected) = expected_mode
        && expected != candidate_mode
    {
        return Err(SelectCourseEntryError::ModeMismatch { expected, actual: candidate_mode });
    }
    Ok(candidate_mode)
}

fn next_local_course_key<'a>(keys: impl IntoIterator<Item = &'a str>) -> String {
    let keys: HashSet<&str> = keys.into_iter().collect();
    (1..)
        .map(|index| format!("local-course-{index}"))
        .find(|candidate| !keys.contains(candidate.as_str()))
        .expect("course key sequence is finite in practice")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn course_builder_accepts_duplicate_charts_until_lr2_limit() {
        for entry_count in 0..SELECT_COURSE_MAX_ENTRIES {
            assert_eq!(
                validate_select_course_entry(entry_count, Some(KeyMode::K7), Some(KeyMode::K7)),
                Ok(KeyMode::K7)
            );
        }
        assert_eq!(
            validate_select_course_entry(
                SELECT_COURSE_MAX_ENTRIES,
                Some(KeyMode::K7),
                Some(KeyMode::K7),
            ),
            Err(SelectCourseEntryError::Full)
        );
    }

    #[test]
    fn course_builder_rejects_mixed_or_unknown_key_modes() {
        assert_eq!(
            validate_select_course_entry(1, Some(KeyMode::K7), Some(KeyMode::K14)),
            Err(SelectCourseEntryError::ModeMismatch {
                expected: KeyMode::K7,
                actual: KeyMode::K14,
            })
        );
        assert_eq!(
            validate_select_course_entry(0, None, None),
            Err(SelectCourseEntryError::UnknownMode)
        );
    }

    #[test]
    fn course_builder_uses_first_available_local_key() {
        assert_eq!(next_local_course_key(["local-course-1", "local-course-3"]), "local-course-2");
    }
}
