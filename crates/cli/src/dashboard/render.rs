use pump_agent_app::api::{CompareRunsOutput, WalletDossierOutput};
use pump_agent_core::{RunInspectReport, StrategyRunRow, SweepBatchInspectReport};

use crate::config::lamports_str_to_sol;

pub fn render_dashboard_home(runs: &[StrategyRunRow], limit: i64) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "<div class=\"page-head\"><div><p class=\"eyebrow\">Pump Agent</p><h1>Run Dashboard</h1><p class=\"muted\">Recent strategy runs and live-paper sessions from PostgreSQL.</p></div><div class=\"pill\">limit {}</div></div>",
        limit
    ));
    body.push_str("<div class=\"grid two\">");
    body.push_str("<div class=\"card\"><h2>Compare Strategies</h2><p class=\"muted\">Inspect field-level strategy differences and run-performance deltas.</p><form class=\"inline-form\" action=\"/compare\" method=\"get\"><label>Left run<input type=\"number\" name=\"left_id\" placeholder=\"101\" required></label><label>Right run<input type=\"number\" name=\"right_id\" placeholder=\"202\" required></label><label>Fills<input type=\"number\" name=\"fill_limit\" value=\"20\" min=\"0\" max=\"500\"></label><button class=\"button\" type=\"submit\">Compare</button></form></div>");
    body.push_str("<div class=\"card\"><h2>Wallet Research</h2><p class=\"muted\">Open a wallet dossier to compare a wallet's behavior against the built-in strategy families.</p><form class=\"inline-form\" action=\"/wallet\" method=\"get\"><label>Address<input type=\"text\" name=\"address\" placeholder=\"wallet address\" required></label><label>Top mints<input type=\"number\" name=\"top_mints_limit\" value=\"10\" min=\"1\" max=\"100\"></label><label>Samples<input type=\"number\" name=\"sample_limit\" value=\"5\" min=\"1\" max=\"20\"></label><button class=\"button\" type=\"submit\">Open dossier</button></form></div>");
    body.push_str("</div>");
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

pub fn render_dashboard_compare(
    output: Option<CompareRunsOutput>,
    left_id: Option<i64>,
    right_id: Option<i64>,
) -> String {
    let mut body = String::new();
    body.push_str("<div class=\"page-head\"><div><p class=\"eyebrow\">Strategy Diff</p><h1>Compare Runs</h1><p class=\"muted\">Field-level strategy deltas and run-performance changes between two saved runs.</p></div><a class=\"button\" href=\"/\">All runs</a></div>");
    body.push_str(&format!(
        "<div class=\"card\"><form class=\"inline-form\" action=\"/compare\" method=\"get\"><label>Left run<input type=\"number\" name=\"left_id\" value=\"{}\" placeholder=\"101\" required></label><label>Right run<input type=\"number\" name=\"right_id\" value=\"{}\" placeholder=\"202\" required></label><label>Fills<input type=\"number\" name=\"fill_limit\" value=\"20\" min=\"0\" max=\"500\"></label><button class=\"button\" type=\"submit\">Refresh diff</button></form></div>",
        left_id.map(|value| value.to_string()).unwrap_or_default(),
        right_id.map(|value| value.to_string()).unwrap_or_default(),
    ));

    let Some(output) = output else {
        body.push_str("<div class=\"card\"><p class=\"muted\">Pick two run IDs to compare strategy configs and performance deltas.</p></div>");
        return render_dashboard_page("Compare Runs", &body);
    };

    body.push_str("<div class=\"grid two\">");
    body.push_str(&metric_card(
        "Equity Delta",
        &format!("{:+.6} SOL", output.deltas.equity_sol),
    ));
    body.push_str(&metric_card(
        "Cash Delta",
        &format!("{:+.6} SOL", output.deltas.cash_sol),
    ));
    body.push_str(&metric_card(
        "Fills Delta",
        &format!("{:+}", output.deltas.fills),
    ));
    body.push_str(&metric_card(
        "Reject Delta",
        &format!("{:+}", output.deltas.rejections),
    ));
    body.push_str("</div>");
    body.push_str(&format!(
        "<div class=\"card\"><h2>Run Pair</h2><p class=\"muted\">Left: <a href=\"/runs/{left}\">#{left}</a> {left_strategy}. Right: <a href=\"/runs/{right}\">#{right}</a> {right_strategy}.</p><p class=\"muted\">Events delta {events:+}, snapshots {left_snap} vs {right_snap}, fills loaded {left_fill} vs {right_fill}.</p></div>",
        left = output.left_run.id,
        right = output.right_run.id,
        left_strategy = html_escape(&output.left_run.strategy_name),
        right_strategy = html_escape(&output.right_run.strategy_name),
        events = output.deltas.events,
        left_snap = output.loaded_position_snapshots.left,
        right_snap = output.loaded_position_snapshots.right,
        left_fill = output.loaded_fills.left,
        right_fill = output.loaded_fills.right,
    ));
    body.push_str("<div class=\"card\"><h2>Strategy Diff</h2><table><thead><tr><th>Field</th><th>Left</th><th>Right</th><th>Delta</th></tr></thead><tbody>");
    for field in output.strategy_diff.changed_fields {
        body.push_str(&format!(
            "<tr><td>{}</td><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>",
            html_escape(field.field),
            html_escape(&field.left.to_string()),
            html_escape(&field.right.to_string()),
            field
                .numeric_delta
                .map(|value| format!("{:+.6}", value))
                .unwrap_or_else(|| "-".to_string()),
        ));
    }
    body.push_str("</tbody></table></div>");
    render_dashboard_page("Compare Runs", &body)
}

pub fn render_dashboard_wallet(
    dossier: Option<WalletDossierOutput>,
    address: Option<&str>,
) -> String {
    let mut body = String::new();
    body.push_str("<div class=\"page-head\"><div><p class=\"eyebrow\">Wallet Research</p><h1>Wallet vs Strategy</h1><p class=\"muted\">See how a wallet's observed behavior maps onto the built-in strategy families.</p></div><a class=\"button\" href=\"/\">All runs</a></div>");
    body.push_str(&format!(
        "<div class=\"card\"><form class=\"inline-form\" action=\"/wallet\" method=\"get\"><label>Address<input type=\"text\" name=\"address\" value=\"{}\" placeholder=\"wallet address\" required></label><label>Top mints<input type=\"number\" name=\"top_mints_limit\" value=\"10\" min=\"1\" max=\"100\"></label><label>Samples<input type=\"number\" name=\"sample_limit\" value=\"5\" min=\"1\" max=\"20\"></label><button class=\"button\" type=\"submit\">Refresh dossier</button></form></div>",
        address.map(html_escape).unwrap_or_default(),
    ));

    let Some(dossier) = dossier else {
        body.push_str("<div class=\"card\"><p class=\"muted\">Enter a wallet address to load a dossier, family recommendation, and next experiment plan.</p></div>");
        return render_dashboard_page("Wallet Dossier", &body);
    };

    body.push_str("<div class=\"grid two\">");
    body.push_str(&metric_card(
        "Trades",
        &dossier.overview.total_trades.to_string(),
    ));
    body.push_str(&metric_card(
        "Distinct Mints",
        &dossier.overview.distinct_mints.to_string(),
    ));
    body.push_str(&metric_card(
        "Roundtrips",
        &dossier.wallet_summary.roundtrip_count.to_string(),
    ));
    body.push_str(&metric_card(
        "Recommended Family",
        &dossier.clone_report.recommended_base_family,
    ));
    body.push_str("</div>");
    body.push_str(&format!(
        "<div class=\"card\"><h2>Recommendation</h2><p class=\"muted\">{}</p><p><strong>Confidence:</strong> {}. <strong>Runner-up:</strong> {}.</p></div>",
        html_escape(&dossier.explain_why.decision_summary),
        html_escape(&dossier.explain_why.confidence),
        html_escape(&dossier.explain_why.runner_up_family),
    ));
    body.push_str("<div class=\"grid two\">");
    body.push_str("<div class=\"card\"><h2>Top Strengths</h2><ul>");
    for line in dossier.explain_why.strengths.iter().take(4) {
        body.push_str(&format!("<li>{}</li>", html_escape(line)));
    }
    body.push_str("</ul></div>");
    body.push_str("<div class=\"card\"><h2>Warnings</h2><ul>");
    for line in dossier.explain_why.warnings.iter().take(4) {
        body.push_str(&format!("<li>{}</li>", html_escape(line)));
    }
    body.push_str("</ul></div>");
    body.push_str("</div>");
    body.push_str("<div class=\"grid two\">");
    body.push_str("<div class=\"card\"><h2>Top Mints</h2><table><thead><tr><th>Mint</th><th>Trades</th><th>Buys</th><th>Sells</th></tr></thead><tbody>");
    for mint in dossier.top_mints.iter().take(8) {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            html_escape(&mint.mint),
            mint.trade_count,
            mint.buy_count,
            mint.sell_count,
        ));
    }
    body.push_str("</tbody></table></div>");
    body.push_str("<div class=\"card\"><h2>Next Experiments</h2><table><thead><tr><th>Priority</th><th>Title</th><th>Family</th><th>Objective</th></tr></thead><tbody>");
    for proposal in dossier.suggest_next_experiment.proposals.iter().take(5) {
        body.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            html_escape(&proposal.priority),
            html_escape(&proposal.title),
            html_escape(&proposal.family),
            html_escape(&proposal.objective),
        ));
    }
    body.push_str("</tbody></table></div>");
    body.push_str("</div>");
    render_dashboard_page("Wallet Dossier", &body)
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
    .inline-form { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 12px; align-items: end; }
    .inline-form label { display: flex; flex-direction: column; gap: 8px; color: var(--muted); font-size: 12px; text-transform: uppercase; letter-spacing: 0.08em; }
    .inline-form input {
      border: 1px solid var(--line); border-radius: 14px; padding: 12px 14px;
      background: rgba(255,255,255,0.7); color: var(--ink); font: inherit;
    }
    ul { margin: 0; padding-left: 18px; }
    li { margin-bottom: 8px; }
    code { font-family: "SFMono-Regular", "Menlo", monospace; font-size: 12px; }
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
    use pump_agent_app::api::{
        CloneReportOutput, CloneScoreBreakdownOutput, CompareRunsDeltasOutput, CompareRunsOutput,
        ExperimentProposalOutput, ExplainWhyOutput, FitSummary, LoadedCountOutput, ParamsSeed,
        StrategyDiffOutput, StrategyFieldDiffOutput, SuggestNextExperimentOutput,
        WalletDossierOutput,
    };
    use pump_agent_app::strategy::{StrategyConfig, SweepConfig};
    use pump_agent_core::{
        AddressMintSummary, AddressOverview, AddressRoundtrip, PositionSnapshotRow, RunFillRow,
        RunInspectReport, StrategyRunDetail, StrategyRunRow, SweepBatchInspectReport,
        SweepBatchRunRow,
    };

    use super::{
        html_escape, render_dashboard_batch_detail, render_dashboard_compare,
        render_dashboard_error, render_dashboard_home, render_dashboard_run_detail,
        render_dashboard_wallet,
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

    #[test]
    fn compare_render_shows_strategy_diff_rows() {
        let output = CompareRunsOutput {
            left_run: StrategyRunDetail {
                id: 1,
                strategy_name: "early_flow".to_string(),
                run_mode: "backtest".to_string(),
                sweep_batch_id: None,
                live_run_id: None,
                config: serde_json::json!({"strategy":"early_flow"}),
                source_type: "postgres".to_string(),
                source_ref: "pump_event_envelopes".to_string(),
                started_at: "2026-04-08T12:00:00Z".to_string(),
                finished_at: None,
                processed_events: 100,
                fills: 4,
                rejections: 0,
                ending_cash_lamports: "1000000000".to_string(),
                ending_equity_lamports: "1100000000".to_string(),
            },
            right_run: StrategyRunDetail {
                id: 2,
                strategy_name: "breakout".to_string(),
                run_mode: "backtest".to_string(),
                sweep_batch_id: None,
                live_run_id: None,
                config: serde_json::json!({"strategy":"breakout"}),
                source_type: "postgres".to_string(),
                source_ref: "pump_event_envelopes".to_string(),
                started_at: "2026-04-08T12:10:00Z".to_string(),
                finished_at: None,
                processed_events: 120,
                fills: 5,
                rejections: 1,
                ending_cash_lamports: "1200000000".to_string(),
                ending_equity_lamports: "1300000000".to_string(),
            },
            left_strategy: sample_strategy("early_flow"),
            right_strategy: sample_strategy("breakout"),
            loaded_fills: LoadedCountOutput { left: 4, right: 5 },
            loaded_position_snapshots: LoadedCountOutput { left: 1, right: 1 },
            deltas: CompareRunsDeltasOutput {
                events: 20,
                fills: 1,
                rejections: 1,
                cash_sol: 0.2,
                equity_sol: 0.2,
            },
            strategy_diff: StrategyDiffOutput {
                family_changed: true,
                changed_field_count: 2,
                changed_fields: vec![
                    StrategyFieldDiffOutput {
                        field: "strategy",
                        left: serde_json::json!("early_flow"),
                        right: serde_json::json!("breakout"),
                        numeric_delta: None,
                    },
                    StrategyFieldDiffOutput {
                        field: "buy_sol",
                        left: serde_json::json!(0.15),
                        right: serde_json::json!(0.18),
                        numeric_delta: Some(0.03),
                    },
                ],
            },
        };

        let html = render_dashboard_compare(Some(output), Some(1), Some(2));
        assert!(html.contains("Compare Runs"));
        assert!(html.contains("buy_sol"));
        assert!(html.contains("/runs/1"));
        assert!(html.contains("/runs/2"));
    }

    #[test]
    fn wallet_render_shows_recommendation_and_experiments() {
        let dossier = WalletDossierOutput {
            address: "Wallet111".to_string(),
            experiment_id: None,
            overview: AddressOverview {
                address: "Wallet111".to_string(),
                total_trades: 12,
                buy_count: 7,
                sell_count: 5,
                distinct_mints: 3,
                first_trade_seq: Some(1),
                first_trade_at: Some("2026-04-08T12:00:00Z".to_string()),
                last_trade_seq: Some(9),
                last_trade_at: Some("2026-04-08T12:10:00Z".to_string()),
                gross_buy_lamports: "1000000000".to_string(),
                gross_sell_lamports: "1200000000".to_string(),
                net_cash_flow_lamports: "200000000".to_string(),
                roundtrip_count: 4,
                closed_roundtrip_count: 3,
                open_roundtrip_count: 1,
                orphan_sell_count: 0,
                realized_pnl_lamports: "150000000".to_string(),
                win_rate_closed: Some(0.66),
                avg_hold_secs_closed: Some(42),
            },
            top_mints: vec![AddressMintSummary {
                mint: "Mint111".to_string(),
                trade_count: 4,
                buy_count: 3,
                sell_count: 1,
                gross_buy_lamports: "1000".to_string(),
                gross_sell_lamports: "1200".to_string(),
                net_cash_flow_lamports: "200".to_string(),
                first_seq: 1,
                last_seq: 4,
                last_trade_at: Some("2026-04-08T12:05:00Z".to_string()),
            }],
            recent_roundtrips: vec![AddressRoundtrip {
                mint: "Mint111".to_string(),
                status: "closed".to_string(),
                opened_seq: 1,
                opened_slot: 1,
                opened_at: Some("2026-04-08T12:00:00Z".to_string()),
                closed_seq: Some(4),
                closed_slot: Some(4),
                closed_at: Some("2026-04-08T12:05:00Z".to_string()),
                hold_secs: Some(300),
                entry_count: 1,
                exit_count: 1,
                bought_token_amount: "5000".to_string(),
                sold_token_amount: "5000".to_string(),
                gross_buy_lamports: "1000".to_string(),
                gross_sell_lamports: "1200".to_string(),
                total_fees_lamports: "10".to_string(),
                total_cashback_lamports: "0".to_string(),
                net_entry_lamports: "1000".to_string(),
                net_exit_lamports: "1200".to_string(),
                realized_pnl_lamports: Some("200".to_string()),
                roi_bps: Some(2000),
            }],
            wallet_summary: sample_wallet_summary(),
            sample_entries: vec![],
            sample_roundtrips: vec![],
            clone_report: CloneReportOutput {
                address: "Wallet111".to_string(),
                recommended_base_family: "breakout".to_string(),
                recommended_next_strategy_name: "breakout_plus".to_string(),
                base_fit: sample_fit("breakout", 0.71),
                runner_up: sample_fit("momentum", 0.61),
                confirmed_rules: vec!["fast confirmation".to_string()],
                tentative_rules: vec!["sell pressure tolerance".to_string()],
                anti_patterns: vec!["not a sniper".to_string()],
                recommended_params_seed: sample_seed(),
                export: None,
            },
            explain_why: ExplainWhyOutput {
                address: "Wallet111".to_string(),
                recommended_family: "breakout".to_string(),
                runner_up_family: "momentum".to_string(),
                confidence: "moderate".to_string(),
                decision_summary: "breakout is ahead because entry timing fits better".to_string(),
                family_gap: 0.1,
                wallet_summary: sample_wallet_summary(),
                base_clone_score: 0.71,
                runner_up_clone_score: 0.61,
                base_breakdown: sample_breakdown(),
                runner_up_breakdown: sample_breakdown(),
                strengths: vec!["entry timing is a relative strength".to_string()],
                weaknesses: vec!["exit behavior is still weak".to_string()],
                warnings: vec!["sample is still small".to_string()],
                next_actions: vec!["tune exit_on_sell_count".to_string()],
            },
            suggest_next_experiment: SuggestNextExperimentOutput {
                address: "Wallet111".to_string(),
                experiment_id: None,
                recommended_family: "breakout".to_string(),
                confidence: "moderate".to_string(),
                history_summary: None,
                proposals: vec![ExperimentProposalOutput {
                    priority: "p0".to_string(),
                    title: "fit breakout".to_string(),
                    family: "breakout".to_string(),
                    objective: "tune breakout".to_string(),
                    rationale: "best family".to_string(),
                    expected_learning: "learn entry timing".to_string(),
                    strategy: sample_strategy("breakout"),
                    sweep: SweepConfig::default(),
                }],
                skipped_families: vec![],
            },
        };

        let html = render_dashboard_wallet(Some(dossier), Some("Wallet111"));
        assert!(html.contains("Wallet vs Strategy"));
        assert!(html.contains("breakout"));
        assert!(html.contains("fit breakout"));
    }

    fn sample_strategy(strategy: &str) -> StrategyConfig {
        StrategyConfig {
            strategy: strategy.to_string(),
            strategy_config: None,
            starting_sol: 10.0,
            buy_sol: 0.15,
            max_age_secs: 20,
            min_buy_count: 4,
            min_unique_buyers: 4,
            min_net_buy_sol: 0.3,
            take_profit_bps: 1800,
            stop_loss_bps: 900,
            max_hold_secs: 45,
            min_total_buy_sol: 0.8,
            max_sell_count: 1,
            min_buy_sell_ratio: 4.0,
            max_concurrent_positions: 3,
            exit_on_sell_count: 3,
            trading_fee_bps: 100,
            slippage_bps: 50,
        }
    }

    fn sample_breakdown() -> CloneScoreBreakdownOutput {
        CloneScoreBreakdownOutput {
            entry_timing_similarity: 0.7,
            hold_time_similarity: 0.6,
            size_profile_similarity: 0.65,
            token_selection_similarity: 0.55,
            exit_behavior_similarity: 0.5,
            count_alignment: 0.75,
        }
    }

    fn sample_fit(family: &str, score: f64) -> FitSummary {
        FitSummary {
            family: family.to_string(),
            clone_score: score,
            f1: score,
            precision: score,
            recall: score,
            breakdown: sample_breakdown(),
        }
    }

    fn sample_seed() -> ParamsSeed {
        ParamsSeed {
            buy_sol: 0.15,
            max_age_secs: 20,
            min_buy_count: 4,
            min_unique_buyers: 4,
            min_net_buy_sol: 0.3,
            min_total_buy_sol: 0.8,
            max_sell_count: 1,
            min_buy_sell_ratio: 4.0,
            max_hold_secs: 45,
            max_concurrent_positions: 3,
            exit_on_sell_count: 3,
            take_profit_bps: 1800,
            stop_loss_bps: 900,
        }
    }

    fn sample_wallet_summary() -> pump_agent_app::clone::WalletBehaviorSummary {
        pump_agent_app::clone::WalletBehaviorSummary {
            entry_count: 4,
            roundtrip_count: 4,
            closed_roundtrip_count: 3,
            open_roundtrip_count: 1,
            orphan_sell_count: 0,
            avg_entry_age_secs: Some(12.0),
            avg_entry_buy_count_before: Some(4.0),
            avg_entry_sell_count_before: Some(1.0),
            avg_entry_unique_buyers_before: Some(4.0),
            avg_entry_total_buy_sol_before: Some(1.0),
            avg_entry_net_flow_sol_before: Some(0.6),
            avg_entry_buy_sell_ratio_before: Some(4.0),
            avg_entry_buy_sol: Some(0.15),
            avg_hold_secs_closed: Some(45.0),
        }
    }
}
