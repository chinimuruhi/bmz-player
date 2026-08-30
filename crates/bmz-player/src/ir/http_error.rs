use std::fmt;

use reqwest::{StatusCode, header};

#[derive(Debug)]
pub(crate) struct IrHttpResponseError {
    summary: String,
    status: StatusCode,
    retry_after_seconds: Option<u64>,
}

impl fmt::Display for IrHttpResponseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.summary)
    }
}

impl std::error::Error for IrHttpResponseError {}

pub(crate) fn retry_after_seconds_from_error(error: &anyhow::Error) -> Option<u64> {
    error.chain().find_map(|cause| {
        cause.downcast_ref::<IrHttpResponseError>().and_then(|error| error.retry_after_seconds)
    })
}

pub(crate) fn status_code_from_error(error: &anyhow::Error) -> Option<StatusCode> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<IrHttpResponseError>().map(|error| error.status))
}

pub(crate) fn http_response_error(
    label: &str,
    status: StatusCode,
    body: &str,
    retry_after: Option<&str>,
) -> anyhow::Error {
    anyhow::Error::new(IrHttpResponseError {
        summary: format!("{label} failed: {}", response_error_summary(status, body, retry_after)),
        status,
        retry_after_seconds: retry_after.and_then(parse_retry_after_seconds),
    })
}

pub(crate) fn parse_retry_after_seconds(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}

/// Error bodies can contain credentials or upstream diagnostics. Only copy a
/// short, structured message into logs and omit every other body verbatim.
pub(crate) fn response_error_summary(
    status: StatusCode,
    body: &str,
    retry_after: Option<&str>,
) -> String {
    const MAX_MESSAGE_CHARS: usize = 200;

    #[derive(serde::Deserialize)]
    struct ErrorBody {
        #[serde(rename = "statusMessage")]
        status_message: Option<String>,
        message: Option<String>,
        error: Option<String>,
    }

    let message = serde_json::from_str::<ErrorBody>(body)
        .ok()
        .and_then(|body| body.status_message.or(body.message).or(body.error))
        .map(|message| message.trim().to_string())
        .filter(|message| !message.is_empty());
    let mut summary = match message {
        Some(message) => {
            let truncated: String = message.chars().take(MAX_MESSAGE_CHARS).collect();
            format!("{status} {truncated}")
        }
        None => format!("{status} (response body omitted, {} bytes)", body.len()),
    };
    if let Some(retry_after) = retry_after.filter(|value| !value.trim().is_empty()) {
        summary.push_str(" (retry after ");
        summary.push_str(retry_after.trim());
        summary.push('s');
        summary.push(')');
    }
    summary
}

pub(crate) fn retry_after_header(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
