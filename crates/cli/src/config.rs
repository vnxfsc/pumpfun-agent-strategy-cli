use anyhow::{Result, bail};
use pump_agent_core::CommitmentLevel;

pub fn parse_commitment(value: &str) -> Result<CommitmentLevel> {
    match value {
        "processed" => Ok(CommitmentLevel::Processed),
        "confirmed" => Ok(CommitmentLevel::Confirmed),
        "finalized" => Ok(CommitmentLevel::Finalized),
        other => bail!("unsupported commitment: {other}"),
    }
}

pub fn required_config(cli_value: Option<String>, env_key: &str) -> Result<String> {
    cli_value
        .or_else(|| std::env::var(env_key).ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "missing configuration: --{} or {}",
                kebab_case(env_key),
                env_key
            )
        })
}

pub fn optional_config(cli_value: Option<String>, env_key: &str) -> Option<String> {
    cli_value
        .or_else(|| std::env::var(env_key).ok())
        .filter(|value| !value.trim().is_empty())
}

pub fn string_config(cli_value: Option<String>, env_key: &str, default: &str) -> String {
    cli_value
        .or_else(|| std::env::var(env_key).ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub fn u64_config(cli_value: Option<u64>, env_key: &str, default: u64) -> Result<u64> {
    match cli_value {
        Some(value) => Ok(value),
        None => match std::env::var(env_key) {
            Ok(value) if !value.trim().is_empty() => value.parse::<u64>().map_err(|error| {
                anyhow::anyhow!("invalid {} value '{}': {}", env_key, value, error)
            }),
            _ => Ok(default),
        },
    }
}

pub fn usize_config(cli_value: Option<usize>, env_key: &str, default: usize) -> Result<usize> {
    match cli_value {
        Some(value) => Ok(value),
        None => match std::env::var(env_key) {
            Ok(value) if !value.trim().is_empty() => value.parse::<usize>().map_err(|error| {
                anyhow::anyhow!("invalid {} value '{}': {}", env_key, value, error)
            }),
            _ => Ok(default),
        },
    }
}

pub fn kebab_case(value: &str) -> String {
    value.to_ascii_lowercase().replace('_', "-")
}

pub fn blank_to_na(value: &str) -> &str {
    if value.trim().is_empty() {
        "n/a"
    } else {
        value
    }
}

pub fn sol_to_lamports(sol: f64) -> u64 {
    (sol * 1_000_000_000.0).round() as u64
}

pub fn lamports_to_sol(lamports: u64) -> f64 {
    lamports as f64 / 1_000_000_000.0
}

pub fn lamports_str_to_sol(lamports: &str) -> Result<f64> {
    let parsed = lamports
        .parse::<f64>()
        .map_err(|error| anyhow::anyhow!("invalid lamports value '{}': {}", lamports, error))?;
    Ok(parsed / 1_000_000_000.0)
}

pub fn lamports_u128_to_sol(lamports: u128) -> f64 {
    lamports as f64 / 1_000_000_000.0
}

pub fn lamports_i128_to_sol(lamports: i128) -> f64 {
    lamports as f64 / 1_000_000_000.0
}
