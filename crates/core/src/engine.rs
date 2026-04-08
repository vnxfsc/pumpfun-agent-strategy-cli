use crate::{
    broker::{BrokerSnapshot, PaperBroker},
    model::{EventEnvelope, ExecutionReport, FillReport, RejectedOrder},
    state::MarketState,
    strategy::{Strategy, StrategyMetadata},
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct BacktestReport {
    pub strategy: StrategyMetadata,
    pub processed_events: u64,
    pub fills: u64,
    pub rejections: u64,
    pub ending_cash_lamports: u64,
    pub ending_equity_lamports: u64,
    pub open_positions: usize,
}

#[derive(Debug, Clone)]
pub struct BacktestRunResult {
    pub report: BacktestReport,
    pub fills: Vec<FillReport>,
    pub rejections: Vec<RejectedOrder>,
}

pub struct Engine<S> {
    strategy: S,
    market_state: MarketState,
    broker: PaperBroker,
    processed_events: u64,
    fills: u64,
    rejections: u64,
    fill_reports: Vec<FillReport>,
    rejection_reports: Vec<RejectedOrder>,
}

impl<S> Engine<S>
where
    S: Strategy,
{
    pub fn new(strategy: S, broker: PaperBroker) -> Self {
        Self {
            strategy,
            market_state: MarketState::default(),
            broker,
            processed_events: 0,
            fills: 0,
            rejections: 0,
            fill_reports: Vec::new(),
            rejection_reports: Vec::new(),
        }
    }

    pub fn step(&mut self, event: EventEnvelope) -> Vec<ExecutionReport> {
        self.processed_events += 1;

        let execution_reports = self.broker.process_event(&event);
        count_reports(&execution_reports, &mut self.fills, &mut self.rejections);
        collect_reports(
            &execution_reports,
            &mut self.fill_reports,
            &mut self.rejection_reports,
        );
        self.strategy.on_execution_reports(
            &execution_reports,
            &self.market_state,
            &self.broker.snapshot(),
        );

        self.market_state.apply(&event);
        let snapshot = self.broker.snapshot();
        let orders = self
            .strategy
            .on_event(&event, &self.market_state, &snapshot);
        self.broker.submit_orders(&event, orders);

        execution_reports
    }

    pub fn run<I>(&mut self, events: I) -> BacktestRunResult
    where
        I: IntoIterator<Item = EventEnvelope>,
    {
        for event in events {
            self.step(event);
        }

        self.finish()
    }

    pub fn finish(&self) -> BacktestRunResult {
        BacktestRunResult {
            report: self.report_snapshot(),
            fills: self.fill_reports.clone(),
            rejections: self.rejection_reports.clone(),
        }
    }

    pub fn report_snapshot(&self) -> BacktestReport {
        let final_snapshot = self.broker.snapshot();
        BacktestReport {
            strategy: self.strategy.metadata(),
            processed_events: self.processed_events,
            fills: self.fills,
            rejections: self.rejections,
            ending_cash_lamports: final_snapshot.cash_lamports,
            ending_equity_lamports: self.broker.mark_to_market_lamports(&self.market_state),
            open_positions: final_snapshot.positions.len(),
        }
    }

    pub fn market_state(&self) -> &MarketState {
        &self.market_state
    }

    pub fn broker_snapshot(&self) -> BrokerSnapshot {
        self.broker.snapshot()
    }
}

fn count_reports(reports: &[ExecutionReport], fills: &mut u64, rejections: &mut u64) {
    for report in reports {
        match report {
            ExecutionReport::Filled(_) => *fills += 1,
            ExecutionReport::Rejected(_) => *rejections += 1,
        }
    }
}

fn collect_reports(
    reports: &[ExecutionReport],
    fills: &mut Vec<FillReport>,
    rejections: &mut Vec<RejectedOrder>,
) {
    for report in reports {
        match report {
            ExecutionReport::Filled(fill) => fills.push(fill.clone()),
            ExecutionReport::Rejected(rejection) => rejections.push(rejection.clone()),
        }
    }
}
