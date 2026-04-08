use pump_agent_core::{RunInspectReport, StrategyRunRow, SweepBatchInspectReport};

use crate::config::lamports_str_to_sol;

pub fn render_dashboard_home(runs: &[StrategyRunRow], limit: i64) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "<div class=\"page-head\"><div><p class=\"eyebrow\">Pump Agent</p><h1>Run Dashboard</h1><p class=\"muted\">Recent strategy runs and live-paper sessions from PostgreSQL.</p></div><div class=\"pill\">limit {}</div></div>",
        limit
    ));
    body.push_str("<div class=\"card\"><table><thead><tr><th>ID</th><th>Mode</th><th>Strategy</th><th>Batch</th><th>Live</th><th>Events</th><th>Fills</th><th>Rejects</th><th>Equity</th><th>Started</th></tr></thead><tbody>");
    for run in runs {
        let batch_link = run
            .sweep_batch_id
            .as_deref()
            .map(|batch| {
                format!(
                    "<a href=\"/batches/{}\">{}</a>",
                    html_escape(batch),
                    html_escape(batch)
                )
            })
            .unwrap_or_else(|| "<span class=\"muted\">-</span>".to_string());
        let live_id = run
            .live_run_id
            .as_deref()
            .map(html_escape)
            .unwrap_or_else(|| "-".to_string());
        body.push_str(&format!(
            "<tr><td><a href=\"/runs/{id}\">#{id}</a></td><td>{mode}</td><td>{strategy}</td><td>{batch}</td><td>{live}</td><td>{events}</td><td>{fills}</td><td>{rejects}</td><td>{equity:.6} SOL</td><td>{started}</td></tr>",
            id = run.id,
            mode = html_escape(&run.run_mode),
            strategy = html_escape(&run.strategy_name),
            batch = batch_link,
            live = live_id,
            events = run.processed_events,
            fills = run.fills,
            rejects = run.rejections,
            equity = lamports_str_to_sol(&run.ending_equity_lamports).unwrap_or_default(),
            started = html_escape(&run.started_at),
        ));
    }
    body.push_str("</tbody></table></div>");
    render_dashboard_page("Pump Agent Dashboard", &body)
}

pub fn render_dashboard_run_detail(run_id: i64, report: RunInspectReport) -> String {
    let Some(run) = report.run else {
        return render_dashboard_error("Run Not Found", &format!("Run {} does not exist.", run_id));
    };

    let mut body = String::new();
    body.push_str(&format!(
        "<div class=\"page-head\"><div><p class=\"eyebrow\">Run Detail</p><h1>Run #{}</h1><p class=\"muted\">Strategy {}, mode {}, source {}.</p></div><div class=\"stack\"><a class=\"button\" href=\"/\">All runs</a></div></div>",
        run.id,
        html_escape(&run.strategy_name),
        html_escape(&run.run_mode),
        html_escape(&run.source_type),
    ));
    body.push_str("<div class=\"grid two\">");
    body.push_str(&metric_card(
        "Equity",
        &format!(
            "{:.6} SOL",
            lamports_str_to_sol(&run.ending_equity_lamports).unwrap_or_default()
        ),
    ));
    body.push_str(&metric_card(
        "Cash",
        &format!(
            "{:.6} SOL",
            lamports_str_to_sol(&run.ending_cash_lamports).unwrap_or_default()
        ),
    ));
    body.push_str(&metric_card("Events", &run.processed_events.to_string()));
    body.push_str(&metric_card(
        "Fills / Rejects",
        &format!("{} / {}", run.fills, run.rejections),
    ));
    body.push_str("</div>");

    body.push_str("<div class=\"grid two\">");
    body.push_str(&format!(
        "<div class=\"card\"><h2>Metadata</h2><dl class=\"meta\"><dt>Run mode</dt><dd>{}</dd><dt>Sweep batch</dt><dd>{}</dd><dt>Live run</dt><dd>{}</dd><dt>Source ref</dt><dd>{}</dd><dt>Started</dt><dd>{}</dd><dt>Finished</dt><dd>{}</dd></dl></div>",
        html_escape(&run.run_mode),
        run.sweep_batch_id.as_deref().map(html_escape).unwrap_or_else(|| "-".to_string()),
        run.live_run_id.as_deref().map(html_escape).unwrap_or_else(|| "-".to_string()),
        html_escape(&run.source_ref),
        html_escape(&run.started_at),
        run.finished_at.as_deref().map(html_escape).unwrap_or_else(|| "-".to_string()),
    ));
    body.push_str(&format!(
        "<div class=\"card\"><h2>Config</h2><pre>{}</pre></div>",
        html_escape(
            &serde_json::to_string_pretty(&run.config).unwrap_or_else(|_| "{}".to_string())
        )
    ));
    body.push_str("</div>");

    body.push_str("<div class=\"card\"><h2>Position Snapshots</h2><table><thead><tr><th>Kind</th><th>Event Seq</th><th>Slot</th><th>Cash</th><th>Equity</th><th>Open Pos</th><th>Pending</th><th>At</th></tr></thead><tbody>");
    for snapshot in report.position_snapshots {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.6} SOL</td><td>{:.6} SOL</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            html_escape(&snapshot.snapshot_kind),
            snapshot.event_seq.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
            snapshot.event_slot.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
            lamports_str_to_sol(&snapshot.cash_lamports).unwrap_or_default(),
            lamports_str_to_sol(&snapshot.equity_lamports).unwrap_or_default(),
            snapshot.open_positions,
            snapshot.pending_orders,
            snapshot.snapshot_at.as_deref().map(html_escape).unwrap_or_else(|| "-".to_string()),
        ));
    }
    body.push_str("</tbody></table></div>");

    body.push_str("<div class=\"card\"><h2>Fills</h2><table><thead><tr><th>Order</th><th>Side</th><th>Mint</th><th>Lamports</th><th>Token Amt</th><th>Fee</th><th>Price</th><th>Reason</th><th>Executed</th></tr></thead><tbody>");
    for fill in report.fills {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:.12}</td><td>{}</td><td>{}</td></tr>",
            fill.order_id,
            html_escape(&fill.side),
            html_escape(&fill.mint),
            html_escape(&fill.lamports),
            html_escape(&fill.token_amount),
            html_escape(&fill.fee_lamports),
            fill.execution_price_lamports_per_token,
            html_escape(&fill.reason),
            fill.executed_at.as_deref().map(html_escape).unwrap_or_else(|| "-".to_string()),
        ));
    }
    body.push_str("</tbody></table></div>");

    render_dashboard_page(&format!("Run #{}", run.id), &body)
}

pub fn render_dashboard_batch_detail(report: SweepBatchInspectReport) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "<div class=\"page-head\"><div><p class=\"eyebrow\">Sweep Batch</p><h1>{}</h1><p class=\"muted\">Ranked runs from a single parameter sweep.</p></div><a class=\"button\" href=\"/\">All runs</a></div>",
        html_escape(&report.sweep_batch_id)
    ));
    body.push_str("<div class=\"card\"><table><thead><tr><th>Rank</th><th>Run</th><th>Strategy</th><th>Equity</th><th>Cash</th><th>Fills</th><th>Rejects</th><th>Events</th><th>buy_sol</th><th>min_total_buy_sol</th><th>max_sell_count</th><th>ratio</th><th>concurrent</th><th>exit_on_sell_count</th></tr></thead><tbody>");
    for (index, run) in report.runs.iter().enumerate() {
        body.push_str(&format!(
            "<tr><td>{}</td><td><a href=\"/runs/{}\">#{}</a></td><td>{}</td><td>{:.6} SOL</td><td>{:.6} SOL</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            index + 1,
            run.id,
            run.id,
            html_escape(&run.strategy_name),
            lamports_str_to_sol(&run.ending_equity_lamports).unwrap_or_default(),
            lamports_str_to_sol(&run.ending_cash_lamports).unwrap_or_default(),
            run.fills,
            run.rejections,
            run.processed_events,
            json_num_string(&run.config, "buy_sol"),
            json_num_string(&run.config, "min_total_buy_sol"),
            json_num_string(&run.config, "max_sell_count"),
            json_num_string(&run.config, "min_buy_sell_ratio"),
            json_num_string(&run.config, "max_concurrent_positions"),
            json_num_string(&run.config, "exit_on_sell_count"),
        ));
    }
    body.push_str("</tbody></table></div>");
    render_dashboard_page(&format!("Sweep Batch {}", report.sweep_batch_id), &body)
}

pub fn render_dashboard_error(title: &str, message: &str) -> String {
    render_dashboard_page(
        title,
        &format!(
            "<div class=\"page-head\"><div><p class=\"eyebrow\">Pump Agent</p><h1>{}</h1><p class=\"muted\">{}</p></div><a class=\"button\" href=\"/\">All runs</a></div>",
            html_escape(title),
            html_escape(message)
        ),
    )
}

fn render_dashboard_page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title><style>{}</style></head><body><main class=\"shell\">{}</main></body></html>",
        html_escape(title),
        dashboard_css(),
        body
    )
}

fn metric_card(label: &str, value: &str) -> String {
    format!(
        "<div class=\"metric\"><p class=\"metric-label\">{}</p><p class=\"metric-value\">{}</p></div>",
        html_escape(label),
        html_escape(value)
    )
}

fn json_num_string(value: &serde_json::Value, key: &str) -> String {
    match value.get(key) {
        Some(serde_json::Value::Number(number)) => number.to_string(),
        Some(serde_json::Value::String(string)) => html_escape(string),
        Some(other) => html_escape(&other.to_string()),
        None => "-".to_string(),
    }
}

fn dashboard_css() -> &'static str {
    r#"
    :root {
      --bg: #f4efe6;
      --panel: rgba(255, 251, 245, 0.88);
      --ink: #1f1d1a;
      --muted: #6f675d;
      --line: #d4c4ae;
      --accent: #b24c2c;
      --accent-2: #1f6f78;
      --shadow: 0 20px 60px rgba(53, 39, 24, 0.10);
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      color: var(--ink);
      background:
        radial-gradient(circle at top left, rgba(178, 76, 44, 0.18), transparent 28%),
        radial-gradient(circle at top right, rgba(31, 111, 120, 0.20), transparent 24%),
        linear-gradient(180deg, #f6f0e8 0%, #efe4d4 100%);
      font-family: "Iowan Old Style", "Palatino Linotype", "Book Antiqua", Georgia, serif;
    }
    .shell { max-width: 1400px; margin: 0 auto; padding: 32px 20px 48px; }
    .page-head {
      display: flex; justify-content: space-between; gap: 16px; align-items: flex-start;
      margin-bottom: 24px;
    }
    .eyebrow {
      text-transform: uppercase; letter-spacing: 0.18em; font-size: 12px; margin: 0 0 8px;
      color: var(--accent-2);
    }
    h1 { margin: 0 0 8px; font-size: clamp(32px, 5vw, 54px); line-height: 0.95; }
    h2 { margin: 0 0 16px; font-size: 20px; }
    .muted { color: var(--muted); max-width: 70ch; }
    .pill, .button {
      display: inline-flex; align-items: center; justify-content: center;
      padding: 10px 14px; border-radius: 999px; text-decoration: none;
      border: 1px solid var(--line); color: var(--ink); background: rgba(255,255,255,0.55);
      box-shadow: var(--shadow);
    }
    .button { background: var(--accent); color: #fff8f2; border-color: transparent; }
    .stack { display: flex; gap: 12px; align-items: center; }
    .card, .metric {
      background: var(--panel); backdrop-filter: blur(12px);
      border: 1px solid rgba(212, 196, 174, 0.75); border-radius: 24px;
      box-shadow: var(--shadow); padding: 18px;
    }
    .grid { display: grid; gap: 18px; margin-bottom: 18px; }
    .grid.two { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .metric-label { margin: 0 0 8px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.14em; font-size: 12px; }
    .metric-value { margin: 0; font-size: 28px; color: var(--accent); }
    table { width: 100%; border-collapse: collapse; font-size: 14px; }
    th, td { text-align: left; padding: 12px 10px; border-bottom: 1px solid rgba(212, 196, 174, 0.7); vertical-align: top; }
    th { color: var(--muted); font-size: 12px; text-transform: uppercase; letter-spacing: 0.12em; }
    a { color: var(--accent-2); text-decoration: none; }
    a:hover { text-decoration: underline; }
    pre {
      margin: 0; overflow: auto; background: rgba(34, 29, 23, 0.95); color: #f9e7d0;
      padding: 16px; border-radius: 16px; font-size: 13px; line-height: 1.5;
    }
    .meta { display: grid; grid-template-columns: 140px 1fr; gap: 10px 12px; margin: 0; }
    .meta dt { color: var(--muted); }
    .meta dd { margin: 0; word-break: break-word; }
    @media (max-width: 900px) {
      .page-head { flex-direction: column; }
      .grid.two { grid-template-columns: 1fr; }
      .shell { padding: 20px 14px 32px; }
      table { display: block; overflow-x: auto; }
    }
    "#
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use pump_agent_core::{
        PositionSnapshotRow, RunFillRow, RunInspectReport, StrategyRunDetail, StrategyRunRow,
        SweepBatchInspectReport, SweepBatchRunRow,
    };

    use super::{
        html_escape, render_dashboard_batch_detail, render_dashboard_error, render_dashboard_home,
        render_dashboard_run_detail,
    };

    #[test]
    fn html_escape_escapes_dangerous_characters() {
        assert_eq!(
            html_escape("<tag a=\"1\">'&</tag>"),
            "&lt;tag a=&quot;1&quot;&gt;&#39;&amp;&lt;/tag&gt;"
        );
    }

    #[test]
    fn home_render_escapes_strategy_names_and_links_batches() {
        let runs = vec![StrategyRunRow {
            id: 7,
            strategy_name: "<script>alert(1)</script>".to_string(),
            run_mode: "live".to_string(),
            sweep_batch_id: Some("batch-1".to_string()),
            live_run_id: Some("live-1".to_string()),
            source_type: "postgres".to_string(),
            source_ref: "pump_event_envelopes".to_string(),
            started_at: "2026-04-08T12:00:00Z".to_string(),
            finished_at: Some("2026-04-08T12:01:00Z".to_string()),
            processed_events: 10,
            fills: 2,
            rejections: 0,
            ending_cash_lamports: "1000000000".to_string(),
            ending_equity_lamports: "1250000000".to_string(),
        }];

        let html = render_dashboard_home(&runs, 50);
        assert!(html.contains("Run Dashboard"));
        assert!(html.contains("/runs/7"));
        assert!(html.contains("/batches/batch-1"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
    }

    #[test]
    fn run_detail_render_shows_snapshots_and_fills() {
        let report = RunInspectReport {
            run: Some(StrategyRunDetail {
                id: 9,
                strategy_name: "early_flow".to_string(),
                run_mode: "backtest".to_string(),
                sweep_batch_id: Some("batch-x".to_string()),
                live_run_id: None,
                config: serde_json::json!({"strategy":"early_flow","buy_sol":0.2}),
                source_type: "postgres".to_string(),
                source_ref: "pump_event_envelopes".to_string(),
                started_at: "2026-04-08T12:00:00Z".to_string(),
                finished_at: Some("2026-04-08T12:01:00Z".to_string()),
                processed_events: 120,
                fills: 3,
                rejections: 1,
                ending_cash_lamports: "1000000000".to_string(),
                ending_equity_lamports: "1100000000".to_string(),
            }),
            fills: vec![RunFillRow {
                order_id: 1,
                mint: "Mint111".to_string(),
                side: "buy".to_string(),
                lamports: "1000".to_string(),
                token_amount: "2000".to_string(),
                fee_lamports: "10".to_string(),
                execution_price_lamports_per_token: 0.5,
                reason: "signal <fast>".to_string(),
                executed_at: Some("2026-04-08T12:00:30Z".to_string()),
            }],
            position_snapshots: vec![PositionSnapshotRow {
                snapshot_kind: "heartbeat".to_string(),
                event_seq: Some(12),
                event_slot: Some(34),
                snapshot_at: Some("2026-04-08T12:00:40Z".to_string()),
                cash_lamports: "1000000000".to_string(),
                equity_lamports: "1100000000".to_string(),
                pending_orders: 0,
                open_positions: 1,
                positions: serde_json::json!([]),
            }],
        };

        let html = render_dashboard_run_detail(9, report);
        assert!(html.contains("Run #9"));
        assert!(html.contains("Position Snapshots"));
        assert!(html.contains("Mint111"));
        assert!(html.contains("signal &lt;fast&gt;"));
    }

    #[test]
    fn batch_detail_render_shows_ranked_runs() {
        let report = SweepBatchInspectReport {
            sweep_batch_id: "batch-123".to_string(),
            runs: vec![SweepBatchRunRow {
                id: 3,
                strategy_name: "early_flow".to_string(),
                run_mode: "sweep".to_string(),
                sweep_batch_id: "batch-123".to_string(),
                started_at: "2026-04-08T12:00:00Z".to_string(),
                processed_events: 500,
                fills: 7,
                rejections: 0,
                ending_cash_lamports: "1000000000".to_string(),
                ending_equity_lamports: "1200000000".to_string(),
                config: serde_json::json!({
                    "buy_sol": 0.2,
                    "min_total_buy_sol": 0.8,
                    "max_sell_count": 1,
                    "min_buy_sell_ratio": 4.0,
                    "max_concurrent_positions": 3,
                    "exit_on_sell_count": 3
                }),
            }],
        };

        let html = render_dashboard_batch_detail(report);
        assert!(html.contains("Sweep Batch"));
        assert!(html.contains("/runs/3"));
        assert!(html.contains("batch-123"));
        assert!(html.contains("early_flow"));
    }

    #[test]
    fn error_render_escapes_message() {
        let html = render_dashboard_error("Oops", "<broken>");
        assert!(html.contains("Oops"));
        assert!(html.contains("&lt;broken&gt;"));
        assert!(!html.contains("<broken>"));
    }
}
