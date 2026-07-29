use super::*;

pub(super) fn convert_score_ranking(
    chart_sha256: &str,
    resources: Vec<RianRankingResource>,
    limit: u32,
    self_player_id: Option<&str>,
) -> IrRankingResult {
    let entries: Vec<_> = resources
        .into_iter()
        .take(limit as usize)
        .enumerate()
        .map(|(index, resource)| score_ranking_entry(index, resource))
        .collect();
    let self_summary = self_player_id.and_then(|player_id| {
        entries
            .iter()
            .find(|entry| entry.player.id == player_id)
            .map(|entry| IrRankingSelfRef { rank: entry.rank, score_id: None })
    });
    let total = entries.len() as u32;
    IrRankingResult {
        chart: IrRankingChartRef { sha256: chart_sha256.to_string() },
        ranking: IrRankingBody {
            scope: IrRankingScope::Global,
            entries,
            clear_rate: None,
            self_summary,
            pagination: Some(IrRankingPagination {
                limit,
                offset: 0,
                total: Some(total),
                has_more: false,
            }),
        },
    }
}

pub(super) fn score_ranking_entry(index: usize, resource: RianRankingResource) -> IrRankingEntry {
    let attributes = resource.attributes;
    let player_id = string_attr(&attributes, "player_name");
    let display_name =
        non_empty_attr(&attributes, "display_name").unwrap_or_else(|| player_id.clone());
    IrRankingEntry {
        rank: index as u32 + 1,
        scope_rank: None,
        player: IrRankingPlayer { id: player_id, display_name },
        score: IrRankingScore {
            clear: clear_type_name(int_attr(&attributes, "clear_type")),
            ex_score: uint_attr(&attributes, "ex_score"),
            max_combo: uint_attr(&attributes, "max_combo"),
            min_bp: uint_attr(&attributes, "min_bp"),
            min_cb: uint_attr(&attributes, "min_bp"),
            judges: Some(ranking_judges(&attributes)),
            device_type: None,
            played_at: non_empty_attr(&attributes, "play_date"),
        },
    }
}

pub(super) fn convert_course_ranking(
    course_hash: &str,
    resources: Vec<RianRankingResource>,
    limit: u32,
) -> IrCourseRankingResult {
    let entries = resources
        .into_iter()
        .take(limit as usize)
        .enumerate()
        .map(|(index, resource)| {
            let attributes = resource.attributes;
            let player_id = string_attr(&attributes, "player_name");
            let display_name =
                non_empty_attr(&attributes, "display_name").unwrap_or_else(|| player_id.clone());
            IrCourseRankingEntry {
                rank: index as u32 + 1,
                player: IrRankingPlayer { id: player_id, display_name },
                score: IrCourseRankingScore {
                    course_score_id: resource.id,
                    clear: clear_type_name(int_attr(&attributes, "clear_type")),
                    course_clear: int_attr(&attributes, "clear_type") > 1,
                    ex_score: uint_attr(&attributes, "ex_score"),
                    max_combo: uint_attr(&attributes, "max_combo"),
                    bp: uint_attr(&attributes, "min_bp"),
                    device_type: None,
                    played_at: non_empty_attr(&attributes, "play_date"),
                    verification: None,
                },
            }
        })
        .collect();
    IrCourseRankingResult {
        course: IrCourseRankingCourseRef { course_hash: course_hash.to_string() },
        rule: None,
        ranking: IrCourseRankingBody { scope: IrRankingScope::Global, entries },
    }
}

pub(super) fn empty_ranking(chart_sha256: &str, limit: u32) -> IrRankingResult {
    IrRankingResult {
        chart: IrRankingChartRef { sha256: chart_sha256.to_string() },
        ranking: IrRankingBody {
            scope: IrRankingScope::Global,
            entries: Vec::new(),
            clear_rate: None,
            self_summary: None,
            pagination: Some(IrRankingPagination {
                limit,
                offset: 0,
                total: Some(0),
                has_more: false,
            }),
        },
    }
}

pub(super) fn empty_course_ranking(course_hash: &str) -> IrCourseRankingResult {
    IrCourseRankingResult {
        course: IrCourseRankingCourseRef { course_hash: course_hash.to_string() },
        rule: None,
        ranking: IrCourseRankingBody { scope: IrRankingScope::Global, entries: Vec::new() },
    }
}

pub(super) fn ranking_judges(attributes: &Map<String, Value>) -> IrJudgePayload {
    IrJudgePayload {
        fast: IrJudgeSidePayload {
            pgreat: uint_attr(attributes, "pgreat"),
            great: uint_attr(attributes, "great"),
            good: uint_attr(attributes, "good"),
            bad: uint_attr(attributes, "bad"),
            poor: uint_attr(attributes, "poor"),
            empty_poor: uint_attr(attributes, "miss"),
        },
        slow: IrJudgeSidePayload { pgreat: 0, great: 0, good: 0, bad: 0, poor: 0, empty_poor: 0 },
    }
}

pub(super) fn string_attr(attributes: &Map<String, Value>, key: &str) -> String {
    attributes.get(key).and_then(value_as_string).unwrap_or_default()
}

pub(super) fn non_empty_attr(attributes: &Map<String, Value>, key: &str) -> Option<String> {
    let value = string_attr(attributes, key);
    (!value.is_empty() && value != "0").then_some(value)
}

pub(super) fn int_attr(attributes: &Map<String, Value>, key: &str) -> i64 {
    attributes
        .get(key)
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .unwrap_or(0)
}

pub(super) fn uint_attr(attributes: &Map<String, Value>, key: &str) -> u32 {
    int_attr(attributes, key).max(0).min(i64::from(u32::MAX)) as u32
}

pub(super) fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn required_str<'a>(object: &'a Value, key: &str) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("course payload is missing string '{key}'"))
}

pub(super) fn required_i64(object: &Value, key: &str) -> Result<i64> {
    object
        .get(key)
        .and_then(Value::as_i64)
        .with_context(|| format!("course payload is missing integer '{key}'"))
}

pub(super) fn required_u64(object: &Value, key: &str) -> Result<u64> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .with_context(|| format!("course payload is missing unsigned integer '{key}'"))
}

pub(super) fn judge_value(judges: Option<&Map<String, Value>>, key: &str) -> u64 {
    judges.and_then(|judges| judges.get(key)).and_then(Value::as_u64).unwrap_or(0)
}

pub(super) fn constraint_names(value: &Value) -> Vec<String> {
    match value {
        Value::Array(values) => values.iter().filter_map(value_as_string).collect(),
        Value::Object(values) => values
            .iter()
            .filter_map(|(key, value)| match value {
                Value::Bool(true) => Some(key.clone()),
                Value::String(value) if !value.is_empty() => Some(value.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn redacted_request_json(request: &Value) -> Result<String> {
    let mut redacted = request.clone();
    if let Some(object) = redacted.as_object_mut() {
        for key in ["api_token", "signature"] {
            if object.contains_key(key) {
                object.insert(key.to_string(), Value::String("[REDACTED]".to_string()));
            }
        }
    }
    Ok(serde_json::to_string(&redacted)?)
}
