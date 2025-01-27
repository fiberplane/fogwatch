use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Ok,
    Canceled,
    Exception,
    ExceededCpu,
    ExceededMemory,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Exception {
    pub name: String,
    pub message: serde_json::Value,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogEntry {
    pub message: Vec<serde_json::Value>,
    pub level: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TailEventMessage {
    pub outcome: Outcome,
    pub script_name: Option<String>,
    pub exceptions: Vec<Exception>,
    pub logs: Vec<LogEntry>,
    pub event_timestamp: i64,
    pub event: Option<serde_json::Value>,
    pub status: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TailCreationResponse {
    pub id: String,
    pub url: String,
    #[serde(rename = "expires_at")]
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TailRequest {
    pub filters: Vec<TailFilter>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(untagged)]
pub enum TailFilter {
    SamplingRate { sampling_rate: f64 },
    Outcome { outcome: Vec<String> },
    Method { method: Vec<String> },
    Query { query: String },
}

#[derive(Debug, Deserialize)]
pub struct CloudflareResponse<T> {
    pub result: T,
    #[allow(dead_code)]
    pub success: bool,
    #[allow(dead_code)]
    pub errors: Vec<CloudflareError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloudflareError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerScript {
    pub id: String,
    pub etag: String,
    pub script: Option<String>,
    pub size: Option<u64>,
    pub modified_on: String,
    pub created_on: String,
    pub usage_model: Option<String>,
    pub compatibility_date: Option<String>,
    pub compatibility_flags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerScriptResponse {
    pub result: Vec<WorkerScript>,
    pub success: bool,
    pub errors: Vec<CloudflareError>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Pretty,  // Current colored output
    Json,    // Raw JSON format
    Compact, // Single-line format
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pretty" => Ok(OutputFormat::Pretty),
            "json" => Ok(OutputFormat::Json),
            "compact" => Ok(OutputFormat::Compact),
            _ => Err(format!(
                "Unknown output format: {s}. Valid formats are: pretty, json, compact"
            )),
        }
    }
}
