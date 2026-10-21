use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Configuration,
    Authorization,
    Network,
    RateLimit,
    Server,
    Response,
    Pending,
    Local,
}
impl ErrorCode {
    pub fn message(self) -> &'static str {
        match self {
            Self::Configuration => "Could not read the WakaTime API key",
            Self::Authorization => "WakaTime rejected the API key",
            Self::Network => "WakaTime is unavailable: network error or timeout",
            Self::RateLimit => "WakaTime rate limit reached",
            Self::Server => "WakaTime server error",
            Self::Response => "Unexpected WakaTime response",
            Self::Pending => "WakaTime is preparing today's total",
            Self::Local => "Could not read or save the local cache",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct FetchError {
    pub code: ErrorCode,
    pub retry_at: f64,
}
impl From<ErrorCode> for FetchError {
    fn from(code: ErrorCode) -> Self {
        Self {
            code,
            retry_at: 0.0,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Data {
    pub date: String,
    pub timezone: String,
    pub total_seconds: f64,
    pub fetched_at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_at: Option<String>,
}
pub fn timestamp(at: f64) -> Option<DateTime<Utc>> {
    if !at.is_finite() {
        return None;
    }
    DateTime::from_timestamp_millis((at * 1000.0) as i64)
}
pub fn instant(at: &str) -> Option<f64> {
    DateTime::parse_from_rfc3339(at)
        .ok()
        .map(|t| t.timestamp_millis() as f64 / 1000.0)
}
impl Data {
    pub fn valid_today(&self, now: f64) -> bool {
        let Ok(zone) = self.timezone.parse::<Tz>() else {
            return false;
        };
        let Some(now) = timestamp(now) else {
            return false;
        };
        self.date == now.with_timezone(&zone).format("%Y-%m-%d").to_string()
    }
    pub fn valid_value(&self) -> bool {
        self.total_seconds.is_finite()
            && self.total_seconds >= 0.0
            && timestamp(self.fetched_at).is_some()
    }
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Data>,
    #[serde(default)]
    pub attempt_at: f64,
    #[serde(default)]
    pub next_request_at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorCode>,
}
impl State {
    pub fn local_error() -> Self {
        Self {
            error: Some(ErrorCode::Local),
            ..Self::default()
        }
    }
}
pub fn parse_response(payload: &Value, now: f64) -> Result<Data, FetchError> {
    let span = &payload["data"]["range"];
    if span.is_object() && span.get("date").is_none() {
        return Err(ErrorCode::Pending.into());
    }
    let string = |v: &Value| {
        v.as_str()
            .map(str::to_owned)
            .ok_or(FetchError::from(ErrorCode::Response))
    };
    let date = string(&span["date"])?;
    let timezone = string(&span["timezone"])?;
    // Validate the day before the duration, matching the existing API contract.
    let mut data = Data {
        date,
        timezone,
        total_seconds: 0.0,
        fetched_at: now,
        cached_at: None,
    };
    if !data.valid_today(now) {
        return Err(ErrorCode::Pending.into());
    }
    data.total_seconds = payload["data"]["grand_total"]["total_seconds"]
        .as_f64()
        .ok_or(FetchError::from(ErrorCode::Response))?;
    if !data.valid_value() {
        return Err(ErrorCode::Response.into());
    }
    if let Some(at) = payload.get("cached_at").filter(|v| !v.is_null()) {
        let at = string(at)?;
        instant(&at).ok_or(FetchError::from(ErrorCode::Response))?;
        data.cached_at = Some(at);
    }
    Ok(data)
}
pub fn format_duration(seconds: f64) -> String {
    let minutes = (seconds / 60.0).floor() as u64;
    if minutes >= 60 {
        format!("{}h {}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub text: String,
    pub tooltip: String,
    pub class: Vec<String>,
}
pub fn render(state: &State, now: f64, busy: bool) -> Output {
    let current = state
        .data
        .as_ref()
        .filter(|d| d.valid_today(now) && d.valid_value());
    let mut details = vec!["WakaTime · Today · Account total".to_owned()];
    let (mut text, mut class) = if let Some(data) = current {
        let zone = data.timezone.parse::<Tz>().expect("validated timezone");
        let updated = timestamp(data.fetched_at)
            .expect("validated timestamp")
            .with_timezone(&zone)
            .format("%H:%M:%S")
            .to_string();
        details.push(format!("{} · {}", data.date, data.timezone));
        details.push(format!("Updated: {updated}"));
        (format_duration(data.total_seconds), "ready")
    } else {
        (
            "—".to_owned(),
            if busy || state.error == Some(ErrorCode::Pending) {
                "loading"
            } else {
                "error"
            },
        )
    };
    if current.is_some() && state.error.is_some() {
        text.push_str(" !");
        class = "stale";
    }
    if let Some(code) = state.error {
        details.push(code.message().to_owned());
    } else if busy {
        details.push("Updating…".to_owned());
    }
    // Diagnostics remain available in JSON; the module disables hover tooltips.
    Output {
        text,
        tooltip: escape(&details.join("\n")),
        class: vec![class.to_owned()],
    }
}
