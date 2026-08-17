use super::*;

const LOCAL_COURSE_SOURCE: &str = "bmz:local";

#[derive(Default)]
pub(super) struct CourseEditorDataCache {
    pub(super) data: CourseEditorData,
    courses_valid: bool,
    charts_valid: bool,
    loaded_query: Option<String>,
    was_visible: bool,
}

impl CourseEditorDataCache {
    pub(super) fn reload_requirements(&mut self, visible: bool, query: &str) -> (bool, bool) {
        if !visible {
            self.was_visible = false;
            return (false, false);
        }

        let opened = !self.was_visible;
        self.was_visible = true;
        let reload_courses = opened || !self.courses_valid;
        let reload_charts =
            opened || !self.charts_valid || self.loaded_query.as_deref() != Some(query);
        (reload_courses, reload_charts)
    }

    pub(super) fn set_courses(&mut self, courses: Vec<crate::storage::library_db::StoredCourse>) {
        self.data.courses = courses;
        self.courses_valid = true;
    }

    pub(super) fn set_charts(&mut self, query: String, charts: Vec<CourseEditorChart>) {
        self.data.charts = charts;
        self.loaded_query = Some(query);
        self.charts_valid = true;
    }

    pub(super) fn invalidate(&mut self) {
        self.courses_valid = false;
        self.charts_valid = false;
    }
}

impl WinitApp {
    pub(super) fn apply_course_editor_action(&mut self, action: CourseEditorAction) {
        let result: Result<(String, bool, Option<i64>)> = match action {
            CourseEditorAction::Save(definition) => self
                .save_local_course(&definition)
                .map(|id| (format!("コースを保存しました (ID {id})"), true, None)),
            CourseEditorAction::SaveAndTest(definition) => {
                self.save_local_course(&definition).map(|id| {
                    (
                        format!("コースを保存してテストプレイを開始しました (ID {id})"),
                        true,
                        Some(id),
                    )
                })
            }
            CourseEditorAction::Delete(course_id) => {
                self.delete_local_course(course_id).map(|message| (message, true, None))
            }
            CourseEditorAction::Export { path, definition } => {
                self.export_local_course(path, &definition).map(|message| (message, false, None))
            }
            CourseEditorAction::Import { path } => {
                self.import_local_courses(path).map(|message| (message, true, None))
            }
        };
        let (message, error) = match result {
            Ok((message, refresh_select, test_course_id)) => {
                if refresh_select {
                    self.reload_select_items();
                }
                if let Some(course_id) = test_course_id {
                    self.start_course(course_id);
                }
                (message, false)
            }
            Err(error) => {
                tracing::error!(%error, "course editor action failed");
                (format!("コース操作に失敗しました: {error}"), true)
            }
        };
        if let Some(egui) = self.ui.egui.as_mut() {
            egui.set_course_editor_status(message, error);
        }
    }

    pub(super) fn save_local_course(
        &mut self,
        definition: &bmz_core::course::CourseDefinition,
    ) -> Result<i64> {
        let mut definition = definition.clone();
        crate::course::normalize_course_definition(&mut definition);
        if definition.entries.is_empty() {
            anyhow::bail!("コースには1曲以上必要です");
        }
        if definition.entries.iter().any(|entry| entry.chart_id.is_none()) {
            anyhow::bail!("未解決の譜面が含まれています");
        }
        let position =
            self.boot.library_db.list_courses_by_source(LOCAL_COURSE_SOURCE)?.len() as i64;
        self.boot.library_db.upsert_course(
            LOCAL_COURSE_SOURCE,
            &definition,
            position,
            course_editor_unix_now(),
        )
    }

    fn delete_local_course(&mut self, course_id: i64) -> Result<String> {
        let course = self
            .boot
            .library_db
            .course_by_id(course_id)?
            .context("削除対象のコースが見つかりません")?;
        if course.source != LOCAL_COURSE_SOURCE {
            anyhow::bail!(
                "インポート元のコースは削除できません。ローカルへ保存したコピーを編集してください"
            );
        }
        self.boot.library_db.delete_course(course_id)?;
        Ok(format!("{} を削除しました", course.definition.title))
    }

    fn export_local_course(
        &self,
        path: PathBuf,
        definition: &bmz_core::course::CourseDefinition,
    ) -> Result<String> {
        let path = self.course_editor_path(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json =
            crate::course::serialize_beatoraja_course_json(std::slice::from_ref(definition))?;
        std::fs::write(&path, json)
            .with_context(|| format!("write course JSON: {}", path.display()))?;
        Ok(format!("JSON を書き出しました: {}", path.display()))
    }

    fn import_local_courses(&mut self, path: PathBuf) -> Result<String> {
        let path = self.course_editor_path(path);
        let json = std::fs::read_to_string(&path)
            .with_context(|| format!("read course JSON: {}", path.display()))?;
        let source_key = path.to_string_lossy().replace('\\', "/");
        let courses = crate::course::parse_beatoraja_course_json(&source_key, &json)?;
        for (position, course) in courses.iter().enumerate() {
            self.boot.library_db.upsert_course(
                LOCAL_COURSE_SOURCE,
                course,
                position as i64,
                course_editor_unix_now(),
            )?;
        }
        Ok(format!("{} 件のコースを取り込みました", courses.len()))
    }

    fn course_editor_path(&self, path: PathBuf) -> PathBuf {
        if path.is_absolute() { path } else { self.boot.profile_paths.root_dir.join(path) }
    }
}

fn course_editor_unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn course_editor_cache_reloads_only_when_needed() {
        let mut cache = CourseEditorDataCache::default();

        assert_eq!(cache.reload_requirements(false, ""), (false, false));
        assert_eq!(cache.reload_requirements(true, ""), (true, true));
        cache.set_courses(Vec::new());
        cache.set_charts(String::new(), Vec::new());
        assert_eq!(cache.reload_requirements(true, ""), (false, false));
        assert_eq!(cache.reload_requirements(true, "blue"), (false, true));
        cache.set_charts("blue".to_string(), Vec::new());
        assert_eq!(cache.reload_requirements(true, "blue"), (false, false));

        cache.invalidate();
        assert_eq!(cache.reload_requirements(true, "blue"), (true, true));
    }

    #[test]
    fn course_editor_cache_reloads_after_reopening() {
        let mut cache = CourseEditorDataCache::default();
        assert_eq!(cache.reload_requirements(true, ""), (true, true));
        cache.set_courses(Vec::new());
        cache.set_charts(String::new(), Vec::new());

        assert_eq!(cache.reload_requirements(false, ""), (false, false));
        assert_eq!(cache.reload_requirements(true, ""), (true, true));
    }
}
