use std::{
    process::ExitCode,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Error;
use serde::Serialize;
use serde_json::{Value, json};

use crate::args::OutputFormat;

pub const SCHEMA_VERSION: &str = "v1";

pub type CommandResult<T> = Result<T, CommandError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ValidationError,
    ConfigMissing,
    NotFound,
    ExternalDependencyError,
    InternalError,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ValidationError => "VALIDATION_ERROR",
            Self::ConfigMissing => "CONFIG_MISSING",
            Self::NotFound => "NOT_FOUND",
            Self::ExternalDependencyError => "EXTERNAL_DEPENDENCY_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }

    pub fn exit_code(self) -> u8 {
        match self {
            Self::ValidationError => 2,
            Self::ConfigMissing => 3,
            Self::NotFound => 5,
            Self::ExternalDependencyError => 4,
            Self::InternalError => 7,
        }
    }
}

#[derive(Debug)]
pub struct CommandError {
    code: ErrorCode,
    message: String,
    details: Option<Value>,
    retryable: bool,
}

impl CommandError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::ValidationError,
            message: message.into(),
            details: None,
            retryable: false,
        }
    }

    pub fn config_missing(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::ConfigMissing,
            message: message.into(),
            details: None,
            retryable: false,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::NotFound,
            message: message.into(),
            details: None,
            retryable: false,
        }
    }

    pub fn external_dependency(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::ExternalDependencyError,
            message: message.into(),
            details: None,
            retryable: true,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InternalError,
            message: message.into(),
            details: None,
            retryable: false,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl From<Error> for CommandError {
    fn from(error: Error) -> Self {
        let message = error.to_string();

        if message.starts_with("missing configuration:") {
            return Self::config_missing(message);
        }

        if message.starts_with("invalid ") || message.starts_with("unsupported ") {
            return Self::validation(message);
        }

        if message.contains("failed to connect to postgres")
            || message.contains("failed to connect to Yellowstone")
            || message.contains("failed to subscribe to Pump transactions")
        {
            return Self::external_dependency(message);
        }

        Self::internal(message)
    }
}

#[derive(Debug, Serialize)]
struct ResponseMeta {
    generated_at_unix_ms: u64,
    duration_ms: u64,
}

#[derive(Debug, Serialize)]
struct SuccessEnvelope<T> {
    schema_version: &'static str,
    ok: bool,
    command: String,
    data: T,
    meta: ResponseMeta,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    schema_version: &'static str,
    ok: bool,
    command: String,
    error: ErrorInfo,
    meta: ResponseMeta,
}

#[derive(Debug, Serialize)]
struct ErrorInfo {
    code: &'static str,
    message: String,
    retryable: bool,
    details: Option<Value>,
}

pub fn emit_json_success<T: Serialize>(
    command: &str,
    data: &T,
    started: Instant,
) -> CommandResult<()> {
    let envelope = SuccessEnvelope {
        schema_version: SCHEMA_VERSION,
        ok: true,
        command: command.to_string(),
        data,
        meta: response_meta(started),
    };

    let body = serde_json::to_string_pretty(&envelope).map_err(|error| {
        CommandError::internal(format!("failed to serialize JSON response: {error}"))
    })?;
    println!("{body}");
    Ok(())
}

pub fn render_error(
    command: &str,
    format: OutputFormat,
    error: CommandError,
    started: Instant,
) -> ExitCode {
    let code = error.code;

    match format {
        OutputFormat::Human => {
            eprintln!("{}: {}", code.as_str(), error.message);
            if let Some(details) = error.details {
                eprintln!("details: {}", details);
            }
        }
        OutputFormat::Json => {
            let envelope = ErrorEnvelope {
                schema_version: SCHEMA_VERSION,
                ok: false,
                command: command.to_string(),
                error: ErrorInfo {
                    code: code.as_str(),
                    message: error.message,
                    retryable: error.retryable,
                    details: error.details,
                },
                meta: response_meta(started),
            };

            match serde_json::to_string_pretty(&envelope) {
                Ok(body) => println!("{body}"),
                Err(serialize_error) => {
                    let fallback = json!({
                        "schema_version": SCHEMA_VERSION,
                        "ok": false,
                        "command": command,
                        "error": {
                            "code": ErrorCode::InternalError.as_str(),
                            "message": format!("failed to serialize JSON error response: {serialize_error}"),
                            "retryable": false,
                        },
                        "meta": response_meta(started),
                    });
                    println!("{fallback}");
                }
            }
        }
    }

    ExitCode::from(code.exit_code())
}

pub fn require_json_support(command: &str, format: OutputFormat) -> CommandResult<()> {
    if format.is_json() {
        return Err(CommandError::validation(format!(
            "JSON output is not yet supported for '{command}'"
        ))
        .with_details(json!({ "command": command, "format": "json" })));
    }

    Ok(())
}

pub fn wants_json(format: OutputFormat, legacy_json_flag: bool) -> bool {
    format.is_json() || legacy_json_flag
}

fn response_meta(started: Instant) -> ResponseMeta {
    ResponseMeta {
        generated_at_unix_ms: unix_timestamp_ms(),
        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
    }
}

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;

    use super::{CommandError, ErrorCode, wants_json};
    use crate::args::OutputFormat;

    #[test]
    fn legacy_json_flag_or_format_json_enable_json_mode() {
        assert!(wants_json(OutputFormat::Human, true));
        assert!(wants_json(OutputFormat::Json, false));
        assert!(!wants_json(OutputFormat::Human, false));
    }

    #[test]
    fn maps_missing_configuration_to_config_error() {
        let error = CommandError::from(anyhow!(
            "missing configuration: --database-url or DATABASE_URL"
        ));
        assert_eq!(error.code, ErrorCode::ConfigMissing);
    }

    #[test]
    fn maps_connectivity_failures_to_external_dependency_error() {
        let error = CommandError::from(anyhow!(
            "failed to connect to postgres at postgres://local/test"
        ));
        assert_eq!(error.code, ErrorCode::ExternalDependencyError);
    }
}
