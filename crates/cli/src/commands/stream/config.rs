use anyhow::{Result, bail};
use pump_agent_core::{CommitmentLevel, YellowstoneConfig};

use crate::{
    args::{IngestArgs, LivePaperArgs},
    config::{
        optional_config, parse_commitment, required_config, string_config, u64_config, usize_config,
    },
};

#[derive(Debug, Clone)]
pub struct StreamRuntimeConfig {
    pub yellowstone: YellowstoneConfig,
    pub database_url: Option<String>,
    pub heartbeat_secs: u64,
    pub reconnect_delay_secs: u64,
    pub max_db_connections: u32,
    pub apply_schema: bool,
}

#[derive(Debug)]
struct YellowstoneResolved {
    endpoint: String,
    x_token: Option<String>,
    commitment: CommitmentLevel,
    x_request_snapshot: bool,
    max_decoding_message_size: usize,
    connect_timeout_secs: u64,
    request_timeout_secs: u64,
    http2_keep_alive_interval_secs: u64,
    keep_alive_timeout_secs: u64,
    tcp_keepalive_secs: u64,
}

#[derive(Debug)]
struct StreamRuntimeInput {
    endpoint: Option<String>,
    database_url: Option<String>,
    x_token: Option<String>,
    commitment: Option<String>,
    x_request_snapshot: bool,
    max_decoding_message_size: Option<usize>,
    connect_timeout_secs: Option<u64>,
    request_timeout_secs: Option<u64>,
    max_db_connections: u32,
    heartbeat_secs: Option<u64>,
    reconnect_delay_secs: Option<u64>,
    http2_keep_alive_interval_secs: Option<u64>,
    keep_alive_timeout_secs: Option<u64>,
    tcp_keepalive_secs: Option<u64>,
    apply_schema: bool,
}

fn build_yellowstone_config(config: YellowstoneResolved) -> YellowstoneConfig {
    YellowstoneConfig {
        endpoint: config.endpoint,
        x_token: config.x_token,
        commitment: config.commitment,
        x_request_snapshot: config.x_request_snapshot,
        max_decoding_message_size: config.max_decoding_message_size,
        connect_timeout_secs: config.connect_timeout_secs,
        request_timeout_secs: config.request_timeout_secs,
        http2_keep_alive_interval_secs: config.http2_keep_alive_interval_secs,
        keep_alive_timeout_secs: config.keep_alive_timeout_secs,
        tcp_keepalive_secs: config.tcp_keepalive_secs,
    }
}

fn resolve_common(
    input: StreamRuntimeInput,
    database_env_mode: DatabaseEnvMode,
) -> Result<StreamRuntimeConfig> {
    let endpoint = required_config(input.endpoint, "YELLOWSTONE_ENDPOINT")?;
    let database_url = match database_env_mode {
        DatabaseEnvMode::Required => Some(required_config(input.database_url, "DATABASE_URL")?),
        DatabaseEnvMode::Optional => optional_config(input.database_url, "DATABASE_URL"),
    };
    let x_token = optional_config(input.x_token, "YELLOWSTONE_X_TOKEN");
    let commitment = parse_commitment(&string_config(
        input.commitment,
        "YELLOWSTONE_COMMITMENT",
        "processed",
    ))?;
    let max_decoding_message_size = usize_config(
        input.max_decoding_message_size,
        "YELLOWSTONE_MAX_DECODING_MESSAGE_SIZE",
        64 * 1024 * 1024,
    )?;
    let connect_timeout_secs = u64_config(
        input.connect_timeout_secs,
        "YELLOWSTONE_CONNECT_TIMEOUT_SECS",
        10,
    )?;
    let request_timeout_secs = u64_config(
        input.request_timeout_secs,
        "YELLOWSTONE_REQUEST_TIMEOUT_SECS",
        30,
    )?;
    let heartbeat_secs = u64_config(input.heartbeat_secs, "YELLOWSTONE_HEARTBEAT_SECS", 15)?;
    let reconnect_delay_secs = u64_config(
        input.reconnect_delay_secs,
        "YELLOWSTONE_RECONNECT_DELAY_SECS",
        3,
    )?;
    let http2_keep_alive_interval_secs = u64_config(
        input.http2_keep_alive_interval_secs,
        "YELLOWSTONE_HTTP2_KEEP_ALIVE_INTERVAL_SECS",
        15,
    )?;
    let keep_alive_timeout_secs = u64_config(
        input.keep_alive_timeout_secs,
        "YELLOWSTONE_KEEP_ALIVE_TIMEOUT_SECS",
        10,
    )?;
    let tcp_keepalive_secs = u64_config(
        input.tcp_keepalive_secs,
        "YELLOWSTONE_TCP_KEEPALIVE_SECS",
        30,
    )?;

    Ok(StreamRuntimeConfig {
        yellowstone: build_yellowstone_config(YellowstoneResolved {
            endpoint,
            x_token,
            commitment,
            x_request_snapshot: input.x_request_snapshot,
            max_decoding_message_size,
            connect_timeout_secs,
            request_timeout_secs,
            http2_keep_alive_interval_secs,
            keep_alive_timeout_secs,
            tcp_keepalive_secs,
        }),
        database_url,
        heartbeat_secs,
        reconnect_delay_secs,
        max_db_connections: input.max_db_connections,
        apply_schema: input.apply_schema,
    })
}

enum DatabaseEnvMode {
    Required,
    Optional,
}

pub fn resolve_ingest_runtime_config(args: &IngestArgs) -> Result<StreamRuntimeConfig> {
    resolve_common(
        StreamRuntimeInput {
            endpoint: args.endpoint.clone(),
            database_url: args.database_url.clone(),
            x_token: args.x_token.clone(),
            commitment: args.commitment.clone(),
            x_request_snapshot: args.x_request_snapshot,
            max_decoding_message_size: args.max_decoding_message_size,
            connect_timeout_secs: args.connect_timeout_secs,
            request_timeout_secs: args.request_timeout_secs,
            max_db_connections: args.max_db_connections,
            heartbeat_secs: args.heartbeat_secs,
            reconnect_delay_secs: args.reconnect_delay_secs,
            http2_keep_alive_interval_secs: args.http2_keep_alive_interval_secs,
            keep_alive_timeout_secs: args.keep_alive_timeout_secs,
            tcp_keepalive_secs: args.tcp_keepalive_secs,
            apply_schema: args.apply_schema,
        },
        DatabaseEnvMode::Required,
    )
}

pub fn resolve_live_paper_runtime_config(args: &LivePaperArgs) -> Result<StreamRuntimeConfig> {
    let config = resolve_common(
        StreamRuntimeInput {
            endpoint: args.endpoint.clone(),
            database_url: args.database_url.clone(),
            x_token: args.x_token.clone(),
            commitment: args.commitment.clone(),
            x_request_snapshot: args.x_request_snapshot,
            max_decoding_message_size: args.max_decoding_message_size,
            connect_timeout_secs: args.connect_timeout_secs,
            request_timeout_secs: args.request_timeout_secs,
            max_db_connections: args.max_db_connections,
            heartbeat_secs: args.heartbeat_secs,
            reconnect_delay_secs: args.reconnect_delay_secs,
            http2_keep_alive_interval_secs: args.http2_keep_alive_interval_secs,
            keep_alive_timeout_secs: args.keep_alive_timeout_secs,
            tcp_keepalive_secs: args.tcp_keepalive_secs,
            apply_schema: args.apply_schema,
        },
        DatabaseEnvMode::Optional,
    )?;

    if (args.persist_events || args.save_run || args.resume_from_db)
        && config.database_url.is_none()
    {
        bail!(
            "live-paper with --persist-events, --save-run, or --resume-from-db requires DATABASE_URL or --database-url"
        );
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::{resolve_ingest_runtime_config, resolve_live_paper_runtime_config};
    use crate::args::{IngestArgs, LivePaperArgs, StrategyArgs};

    fn sample_strategy_args() -> StrategyArgs {
        StrategyArgs {
            strategy: "early_flow".to_string(),
            strategy_config: None,
            starting_sol: 10.0,
            buy_sol: 0.2,
            max_age_secs: 45,
            min_buy_count: 3,
            min_unique_buyers: 3,
            min_net_buy_sol: 0.3,
            take_profit_bps: 2500,
            stop_loss_bps: 1200,
            max_hold_secs: 90,
            min_total_buy_sol: 0.8,
            max_sell_count: 1,
            min_buy_sell_ratio: 4.0,
            max_concurrent_positions: 3,
            exit_on_sell_count: 3,
            trading_fee_bps: 100,
            slippage_bps: 50,
        }
    }

    #[test]
    fn resolves_ingest_runtime_from_explicit_args() {
        let args = IngestArgs {
            endpoint: Some("https://example.com".to_string()),
            database_url: Some("postgres://local/test".to_string()),
            x_token: Some("token-123".to_string()),
            commitment: Some("confirmed".to_string()),
            apply_schema: true,
            x_request_snapshot: true,
            max_decoding_message_size: Some(1234),
            connect_timeout_secs: Some(11),
            request_timeout_secs: Some(22),
            max_db_connections: 7,
            heartbeat_secs: Some(33),
            reconnect_delay_secs: Some(44),
            http2_keep_alive_interval_secs: Some(55),
            keep_alive_timeout_secs: Some(66),
            tcp_keepalive_secs: Some(77),
            resume_from_db: true,
        };

        let runtime = resolve_ingest_runtime_config(&args).expect("ingest config should resolve");
        assert_eq!(runtime.yellowstone.endpoint, "https://example.com");
        assert_eq!(
            runtime.database_url.as_deref(),
            Some("postgres://local/test")
        );
        assert_eq!(runtime.yellowstone.x_token.as_deref(), Some("token-123"));
        assert!(matches!(
            runtime.yellowstone.commitment,
            pump_agent_core::CommitmentLevel::Confirmed
        ));
        assert!(runtime.yellowstone.x_request_snapshot);
        assert_eq!(runtime.yellowstone.max_decoding_message_size, 1234);
        assert_eq!(runtime.yellowstone.connect_timeout_secs, 11);
        assert_eq!(runtime.yellowstone.request_timeout_secs, 22);
        assert_eq!(runtime.yellowstone.http2_keep_alive_interval_secs, 55);
        assert_eq!(runtime.yellowstone.keep_alive_timeout_secs, 66);
        assert_eq!(runtime.yellowstone.tcp_keepalive_secs, 77);
        assert_eq!(runtime.heartbeat_secs, 33);
        assert_eq!(runtime.reconnect_delay_secs, 44);
        assert_eq!(runtime.max_db_connections, 7);
        assert!(runtime.apply_schema);
    }

    #[test]
    fn live_paper_allows_missing_database_when_not_persisting() {
        let args = LivePaperArgs {
            endpoint: Some("https://example.com".to_string()),
            database_url: None,
            x_token: None,
            commitment: None,
            apply_schema: false,
            x_request_snapshot: false,
            max_decoding_message_size: None,
            connect_timeout_secs: None,
            request_timeout_secs: None,
            max_db_connections: 5,
            heartbeat_secs: None,
            reconnect_delay_secs: None,
            http2_keep_alive_interval_secs: None,
            keep_alive_timeout_secs: None,
            tcp_keepalive_secs: None,
            resume_from_db: false,
            save_run: false,
            persist_events: false,
            summary_every_events: 100,
            dashboard_top_mints: 5,
            dashboard_position_limit: 5,
            dashboard_activity_limit: 8,
            strategy: sample_strategy_args(),
        };

        let runtime =
            resolve_live_paper_runtime_config(&args).expect("live-paper config should resolve");
        assert_eq!(runtime.database_url, None);
        assert_eq!(runtime.heartbeat_secs, 15);
        assert_eq!(runtime.reconnect_delay_secs, 3);
    }

    #[test]
    fn live_paper_requires_database_when_persisting() {
        let args = LivePaperArgs {
            endpoint: Some("https://example.com".to_string()),
            database_url: None,
            x_token: None,
            commitment: None,
            apply_schema: false,
            x_request_snapshot: false,
            max_decoding_message_size: None,
            connect_timeout_secs: None,
            request_timeout_secs: None,
            max_db_connections: 5,
            heartbeat_secs: None,
            reconnect_delay_secs: None,
            http2_keep_alive_interval_secs: None,
            keep_alive_timeout_secs: None,
            tcp_keepalive_secs: None,
            resume_from_db: false,
            save_run: true,
            persist_events: false,
            summary_every_events: 100,
            dashboard_top_mints: 5,
            dashboard_position_limit: 5,
            dashboard_activity_limit: 8,
            strategy: sample_strategy_args(),
        };

        let error =
            resolve_live_paper_runtime_config(&args).expect_err("missing db should be rejected");
        assert!(
            error
                .to_string()
                .contains("requires DATABASE_URL or --database-url")
        );
    }

    #[test]
    fn live_paper_requires_database_when_resume_from_db_is_enabled() {
        let args = LivePaperArgs {
            endpoint: Some("https://example.com".to_string()),
            database_url: None,
            x_token: None,
            commitment: None,
            apply_schema: false,
            x_request_snapshot: false,
            max_decoding_message_size: None,
            connect_timeout_secs: None,
            request_timeout_secs: None,
            max_db_connections: 5,
            heartbeat_secs: None,
            reconnect_delay_secs: None,
            http2_keep_alive_interval_secs: None,
            keep_alive_timeout_secs: None,
            tcp_keepalive_secs: None,
            resume_from_db: true,
            save_run: false,
            persist_events: false,
            summary_every_events: 100,
            dashboard_top_mints: 5,
            dashboard_position_limit: 5,
            dashboard_activity_limit: 8,
            strategy: sample_strategy_args(),
        };

        let error =
            resolve_live_paper_runtime_config(&args).expect_err("missing db should be rejected");
        assert!(
            error
                .to_string()
                .contains("requires DATABASE_URL or --database-url")
        );
    }
}
