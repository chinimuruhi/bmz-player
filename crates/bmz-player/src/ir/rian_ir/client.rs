use super::*;

impl RianIrClient {
    pub fn new(base_url: &str) -> Result<Self> {
        Ok(Self {
            base_url: parse_base_url(base_url)?,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .context("failed to build rianIR HTTP client")?,
        })
    }

    pub async fn login(&self, login_id: &str, password: &str) -> Result<IrAuthTokens> {
        let response = self
            .http
            .post(self.endpoint("auth/login.php")?)
            .json(&json!({ "id": login_id, "pass": password }))
            .send()
            .await
            .context("failed to send rianIR login request")?;
        let decoded: RianLoginResponse = decode_response(response, "rianIR login").await?;
        if decoded.data.meta.api_token.is_empty() {
            bail!("rianIR login response did not contain api_token");
        }
        Ok(IrAuthTokens {
            provider_key: RIAN_IR_PROVIDER.to_string(),
            access_token: decoded.data.meta.api_token,
            refresh_token: String::new(),
            expires_at: None,
            // rianIR submit API requires the login ID as player_name. The response's
            // attributes.player_name is the display name, not the login ID.
            player: IrPlayerInfo {
                id: login_id.to_string(),
                email: None,
                display_name: Some(decoded.data.attributes.player_name),
            },
        })
    }

    pub async fn submit_score(
        &self,
        payload: &IrScoreSubmission,
        player_id: &str,
        api_token: &str,
    ) -> Result<RianSubmitOutcome> {
        ensure_score_payload_supported(payload)?;
        let request = score_request(payload, player_id, api_token)?;
        let redacted_request_json = redacted_request_json(&request)?;
        let response = self
            .http
            .post(self.endpoint("score/score.php")?)
            .json(&request)
            .send()
            .await
            .context("failed to send rianIR score request")?;
        let response_value: Value = decode_response(response, "rianIR score submission").await?;
        ensure_success_status(&response_value, "rianIR score submission")?;
        Ok(RianSubmitOutcome {
            redacted_request_json,
            response_json: serde_json::to_string(&IrSubmitResponse {
                accepted: true,
                score_id: None,
                best_updated: false,
                previous_best: None,
                rankings: BTreeMap::new(),
            })?,
        })
    }

    pub async fn submit_course_score(
        &self,
        payload: &Value,
        player_id: &str,
        api_token: &str,
    ) -> Result<RianSubmitOutcome> {
        let request = course_request(payload, player_id, api_token)?;
        let redacted_request_json = redacted_request_json(&request)?;
        let response = self
            .http
            .post(self.endpoint("score/course_score.php")?)
            .json(&request)
            .send()
            .await
            .context("failed to send rianIR course score request")?;
        let response_value: Value =
            decode_response(response, "rianIR course score submission").await?;
        ensure_success_status(&response_value, "rianIR course score submission")?;
        Ok(RianSubmitOutcome {
            redacted_request_json,
            response_json: serde_json::to_string(&json!({
                "status": "success",
                "course_score_id": Value::Null,
            }))?,
        })
    }

    pub async fn fetch_ranking(
        &self,
        chart_sha256: &str,
        body: &str,
        scope: IrRankingScope,
        limit: u32,
        self_player_id: Option<&str>,
    ) -> Result<IrRankingResult> {
        if scope != IrRankingScope::Global {
            bail!("rianIR supports global ranking scope only");
        }
        let mut url = self.endpoint("score/get_score.php")?;
        url.query_pairs_mut().append_pair("sha256", chart_sha256).append_pair("body", body);
        let response = self.http.get(url).send().await.context("failed to fetch rianIR ranking")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(empty_ranking(chart_sha256, limit));
        }
        let decoded: RianRankingResponse = decode_response(response, "rianIR ranking").await?;
        Ok(convert_score_ranking(chart_sha256, decoded.data, limit, self_player_id))
    }

    pub async fn fetch_course_ranking(
        &self,
        course_hash: &str,
        body: &str,
        limit: u32,
    ) -> Result<IrCourseRankingResult> {
        let mut url = self.endpoint("score/get_course_score.php")?;
        url.query_pairs_mut().append_pair("course_sha256", course_hash).append_pair("body", body);
        let response =
            self.http.get(url).send().await.context("failed to fetch rianIR course ranking")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(empty_course_ranking(course_hash));
        }
        let decoded: RianRankingResponse =
            decode_response(response, "rianIR course ranking").await?;
        Ok(convert_course_ranking(course_hash, decoded.data, limit))
    }

    pub async fn fetch_tables(&self, player_id: &str) -> Result<Vec<RianTableResource>> {
        let mut url = self.endpoint("common/get_tables.php")?;
        if !player_id.trim().is_empty() {
            url.query_pairs_mut().append_pair("id", player_id);
        }
        let response = self.http.get(url).send().await.context("failed to fetch rianIR tables")?;
        let decoded: RianTablesResponse = decode_response(response, "rianIR tables").await?;
        Ok(decoded.data)
    }

    fn endpoint(&self, relative: &str) -> Result<Url> {
        self.base_url.join(relative).context("failed to build rianIR endpoint URL")
    }
}

pub(super) fn parse_base_url(base_url: &str) -> Result<Url> {
    let mut url = Url::parse(base_url).context("invalid rianIR base URL")?;
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    if !url.path().ends_with("/api/") {
        url = url.join("api/").context("failed to normalize rianIR API base URL")?;
    }
    Ok(url)
}

pub(super) async fn decode_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    context: &str,
) -> Result<T> {
    let status = response.status();
    let body = response.text().await.context("failed to read rianIR response body")?;
    if !status.is_success() {
        let detail = error_detail(&body).unwrap_or_else(|| body.chars().take(500).collect());
        bail!("{context} failed with HTTP {status}: {detail}");
    }
    serde_json::from_str(&body).with_context(|| format!("{context} returned invalid JSON"))
}

pub(super) fn error_detail(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    value
        .get("errors")
        .and_then(Value::as_array)
        .and_then(|errors| errors.first())
        .and_then(|error| {
            error.get("detail").or_else(|| error.get("title")).and_then(Value::as_str)
        })
        .or_else(|| value.get("message").and_then(Value::as_str))
        .map(str::to_string)
}

pub(super) fn ensure_success_status(value: &Value, context: &str) -> Result<()> {
    if value.get("status").and_then(Value::as_str) == Some("success") {
        Ok(())
    } else {
        bail!(
            "{context} was not accepted: {}",
            error_detail(&value.to_string()).unwrap_or_else(|| value.to_string())
        )
    }
}
