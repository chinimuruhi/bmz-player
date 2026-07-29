use super::*;

impl LibraryDatabase {
    pub fn upsert_difficulty_table(
        &mut self,
        table: &crate::difficulty_table::FetchedDifficultyTable,
    ) -> Result<i64> {
        super::super::difficulty_table_db::upsert_difficulty_table(&mut self.conn, table)
    }

    pub fn list_difficulty_tables(&self) -> Result<Vec<DifficultyTableRecord>> {
        super::super::difficulty_table_db::list_difficulty_tables(&self.conn)
    }

    pub fn delete_difficulty_tables_by_source_prefix(&self, source_prefix: &str) -> Result<usize> {
        super::super::difficulty_table_db::delete_difficulty_tables_by_source_prefix(
            &self.conn,
            source_prefix,
        )
    }

    pub fn delete_difficulty_table(&self, source_url: &str) -> Result<bool> {
        super::super::difficulty_table_db::delete_difficulty_table(&self.conn, source_url)
    }

    pub fn delete_courses_by_source(&self, source: &str) -> Result<usize> {
        super::super::course_db::delete_courses_by_source(&self.conn, source)
    }

    pub fn delete_table_courses_by_source_prefix(&self, source_prefix: &str) -> Result<usize> {
        super::super::course_db::delete_table_courses_by_source_prefix(&self.conn, source_prefix)
    }

    pub fn list_difficulty_table_sources_with_current_download_metadata(
        &self,
    ) -> Result<Vec<String>> {
        super::super::difficulty_table_db::list_difficulty_table_sources_with_current_download_metadata(
            &self.conn,
        )
    }

    pub fn list_difficulty_table_entries_by_md5s(
        &self,
        md5s: &[&str],
    ) -> Result<Vec<DifficultyTableEntryRecord>> {
        super::super::difficulty_table_db::list_entries_by_md5s(&self.conn, md5s)
    }

    pub fn list_difficulty_table_entries_by_sha256s(
        &self,
        sha256s: &[&str],
    ) -> Result<Vec<DifficultyTableEntryRecord>> {
        super::super::difficulty_table_db::list_entries_by_sha256s(&self.conn, sha256s)
    }

    /// Returns every entry of the given difficulty table, including entries that
    /// are not present in the local library.  Matched charts use MD5 first, then
    /// SHA-256.
    pub fn list_table_entries_with_chart(
        &self,
        source_url: &str,
    ) -> Result<Vec<TableEntryListItem>> {
        self.list_table_entries_with_chart_at_level(source_url, None)
    }

    pub fn list_table_entries_with_chart_at_level(
        &self,
        source_url: &str,
        level: Option<&str>,
    ) -> Result<Vec<TableEntryListItem>> {
        let rows = match level {
            Some(level) => super::super::difficulty_table_db::list_table_entries_at_level(
                &self.conn, source_url, level,
            )?,
            None => super::super::difficulty_table_db::list_table_entries(&self.conn, source_url)?,
        };
        let md5_refs: Vec<&str> =
            rows.iter().filter(|row| row.md5.len() >= 24).map(|row| row.md5.as_str()).collect();
        let sha256_refs: Vec<&str> = rows
            .iter()
            .filter(|row| row.sha256.len() >= 24)
            .map(|row| row.sha256.as_str())
            .collect();
        let md5_charts = self.charts_by_md5s(&md5_refs)?;
        let sha256_charts = self.charts_by_sha256s(&sha256_refs)?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let chart = md5_charts
                    .get(&row.md5)
                    .cloned()
                    .or_else(|| sha256_charts.get(&row.sha256).cloned());
                TableEntryListItem {
                    level: row.level,
                    md5: row.md5,
                    sha256: row.sha256,
                    title: row.title,
                    artist: row.artist,
                    comment: row.comment,
                    url: row.url,
                    append_url: row.append_url,
                    ipfs: row.ipfs,
                    append_ipfs: row.append_ipfs,
                    chart,
                }
            })
            .collect())
    }

    pub(super) fn charts_by_md5s(&self, md5s: &[&str]) -> Result<HashMap<String, ChartListItem>> {
        charts_by_hash_column(&self.conn, "md5", md5s)
    }

    pub(super) fn charts_by_sha256s(
        &self,
        sha256s: &[&str],
    ) -> Result<HashMap<String, ChartListItem>> {
        charts_by_hash_column(&self.conn, "sha256", sha256s)
    }

    pub fn upsert_course(
        &mut self,
        source: &str,
        course: &bmz_core::course::CourseDefinition,
        source_position: i64,
        imported_at: i64,
    ) -> Result<i64> {
        super::super::course_db::upsert_course(
            &mut self.conn,
            source,
            course,
            source_position,
            imported_at,
        )
    }

    pub fn list_courses(&self) -> Result<Vec<StoredCourse>> {
        super::super::course_db::list_courses(&self.conn)
    }

    pub fn list_courses_by_source(&self, source: &str) -> Result<Vec<StoredCourse>> {
        super::super::course_db::list_courses_by_source(&self.conn, source)
    }

    pub fn list_course_entries(&self, course_id: i64) -> Result<Vec<StoredCourseEntry>> {
        super::super::course_db::list_course_entries(&self.conn, course_id)
    }

    /// Returns `(ChartListItem, raw_level)` pairs for charts in the library that
    /// appear in the given difficulty table, matched first by MD5 then by SHA-256.
    /// Charts not present in the local library are omitted.
    ///
    /// Prefer [`Self::list_table_entries_with_chart`] when table entries without a
    /// local chart should be included.
    pub fn list_charts_with_level_in_table(
        &self,
        source_url: &str,
    ) -> Result<Vec<(ChartListItem, String)>> {
        // Use UNION (not UNION ALL) so that a chart matched by both MD5 and SHA-256
        // for the same entry only appears once.
        let sql = format!(
            "
            SELECT {CHART_LIST_ITEM_COLUMNS_C}, dte.level
            FROM difficulty_table_entries dte
            JOIN difficulty_tables dt ON dt.id = dte.table_id
            JOIN charts c ON c.md5 = dte.md5
            WHERE dt.source_url = ?1 AND length(dte.md5) >= 24
            UNION
            SELECT {CHART_LIST_ITEM_COLUMNS_C}, dte.level
            FROM difficulty_table_entries dte
            JOIN difficulty_tables dt ON dt.id = dte.table_id
            JOIN charts c ON c.sha256 = dte.sha256
            WHERE dt.source_url = ?1 AND length(dte.sha256) >= 24"
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![source_url], |row| {
            let chart = chart_list_item_from_row(row)?;
            let level: String = row.get(34)?;
            Ok((chart, level))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }
}
