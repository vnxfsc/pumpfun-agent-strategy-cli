create table if not exists raw_transactions (
    id bigserial primary key,
    slot bigint not null,
    signature text not null,
    tx_index integer not null,
    program_id text not null,
    block_time timestamptz null,
    logs jsonb not null default '[]'::jsonb,
    raw_base64 text not null,
    ingested_at timestamptz not null default now(),
    unique (slot, signature, tx_index)
);

create table if not exists pump_event_envelopes (
    id bigserial primary key,
    seq bigint not null unique,
    slot bigint not null,
    tx_signature text not null,
    tx_index integer not null,
    event_index integer not null,
    event_kind text not null,
    envelope jsonb not null,
    ingested_at timestamptz not null default now(),
    unique (slot, tx_signature, tx_index, event_index)
);

create table if not exists pump_mints (
    mint text primary key,
    seq bigint not null,
    slot bigint not null,
    tx_signature text not null,
    tx_index integer not null,
    event_index integer not null,
    bonding_curve text not null,
    creator text not null,
    name text not null,
    symbol text not null,
    uri text not null,
    created_slot bigint not null,
    created_at timestamptz null,
    is_mayhem_mode boolean not null default false,
    is_cashback_enabled boolean not null default false,
    virtual_token_reserves numeric(39, 0) not null,
    virtual_sol_reserves numeric(39, 0) not null,
    real_token_reserves numeric(39, 0) not null,
    token_total_supply numeric(39, 0) not null,
    token_program text not null,
    raw_event jsonb not null
);

create table if not exists pump_trades (
    id bigserial primary key,
    slot bigint not null,
    tx_signature text not null,
    tx_index integer not null,
    event_index integer not null,
    seq bigint not null,
    mint text not null references pump_mints (mint),
    user_address text not null,
    side text not null check (side in ('buy', 'sell')),
    ix_name text not null,
    timestamp timestamptz null,
    sol_amount numeric(39, 0) not null,
    token_amount numeric(39, 0) not null,
    fee numeric(39, 0) not null,
    fee_basis_points bigint not null,
    creator text not null,
    creator_fee numeric(39, 0) not null,
    creator_fee_basis_points bigint not null,
    cashback numeric(39, 0) not null,
    cashback_fee_basis_points bigint not null,
    virtual_sol_reserves numeric(39, 0) not null,
    virtual_token_reserves numeric(39, 0) not null,
    real_sol_reserves numeric(39, 0) not null,
    real_token_reserves numeric(39, 0) not null,
    track_volume boolean not null,
    raw_event jsonb not null,
    unique (slot, tx_signature, tx_index, event_index)
);

create index if not exists pump_trades_mint_seq_idx on pump_trades (mint, seq);
create index if not exists pump_trades_slot_idx on pump_trades (slot, tx_index, event_index);
create index if not exists pump_trades_user_seq_idx on pump_trades (user_address, seq);
create index if not exists pump_trades_user_mint_seq_idx on pump_trades (user_address, mint, seq);

create table if not exists pump_curve_completions (
    id bigserial primary key,
    slot bigint not null,
    tx_signature text not null,
    tx_index integer not null,
    event_index integer not null,
    seq bigint not null,
    mint text not null,
    bonding_curve text not null,
    user_address text not null,
    timestamp timestamptz null,
    raw_event jsonb not null,
    unique (slot, tx_signature, tx_index, event_index)
);

create table if not exists strategy_runs (
    id bigserial primary key,
    strategy_name text not null,
    config jsonb not null,
    source_type text not null,
    source_ref text not null,
    started_at timestamptz not null default now(),
    finished_at timestamptz null,
    processed_events bigint not null default 0,
    fills bigint not null default 0,
    rejections bigint not null default 0,
    ending_cash_lamports numeric(39, 0) not null default 0,
    ending_equity_lamports numeric(39, 0) not null default 0
);

alter table strategy_runs add column if not exists run_mode text not null default 'backtest';
alter table strategy_runs add column if not exists sweep_batch_id text null;
alter table strategy_runs add column if not exists live_run_id text null;

create table if not exists paper_fills (
    id bigserial primary key,
    strategy_run_id bigint not null references strategy_runs (id) on delete cascade,
    order_id bigint not null,
    mint text not null,
    side text not null check (side in ('buy', 'sell')),
    lamports numeric(39, 0) not null,
    token_amount numeric(39, 0) not null,
    fee_lamports numeric(39, 0) not null,
    execution_price_lamports_per_token double precision not null,
    reason text not null,
    executed_at timestamptz null
);

create table if not exists paper_position_snapshots (
    id bigserial primary key,
    strategy_run_id bigint not null references strategy_runs (id) on delete cascade,
    snapshot_kind text not null,
    event_seq bigint null,
    event_slot bigint null,
    snapshot_at timestamptz null,
    cash_lamports numeric(39, 0) not null,
    equity_lamports numeric(39, 0) not null,
    pending_orders integer not null default 0,
    open_positions integer not null default 0,
    positions jsonb not null default '[]'::jsonb
);

create index if not exists strategy_runs_run_mode_idx on strategy_runs (run_mode, id desc);
create index if not exists strategy_runs_sweep_batch_idx on strategy_runs (sweep_batch_id, id desc);
create index if not exists strategy_runs_live_run_idx on strategy_runs (live_run_id, id desc);
create index if not exists paper_position_snapshots_run_id_idx on paper_position_snapshots (strategy_run_id, id asc);

create table if not exists task_runs (
    id bigserial primary key,
    task_id text not null unique,
    task_kind text not null,
    status text not null check (status in ('queued', 'running', 'cancelling', 'succeeded', 'failed', 'cancelled')),
    idempotency_key text null,
    cancellation_requested boolean not null default false,
    request_payload jsonb not null default '{}'::jsonb,
    result_payload jsonb null,
    error_payload jsonb null,
    submitted_at timestamptz not null default now(),
    started_at timestamptz null,
    finished_at timestamptz null
);

create unique index if not exists task_runs_idempotency_key_idx
    on task_runs (task_kind, idempotency_key)
    where idempotency_key is not null;

create index if not exists task_runs_status_idx on task_runs (status, submitted_at desc);
create index if not exists task_runs_kind_idx on task_runs (task_kind, submitted_at desc);

create table if not exists experiments (
    id bigserial primary key,
    experiment_id text not null unique,
    title text not null,
    target_wallet text not null,
    status text not null default 'active',
    thesis text null,
    notes jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists hypotheses (
    id bigserial primary key,
    hypothesis_id text not null unique,
    experiment_id text not null references experiments (experiment_id) on delete cascade,
    family text not null,
    description text not null,
    status text not null default 'open',
    strategy_config jsonb not null default '{}'::jsonb,
    sample_window jsonb not null default '{}'::jsonb,
    notes jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists evaluations (
    id bigserial primary key,
    evaluation_id text not null unique,
    experiment_id text not null references experiments (experiment_id) on delete cascade,
    hypothesis_id text null references hypotheses (hypothesis_id) on delete set null,
    strategy_run_id bigint null references strategy_runs (id) on delete set null,
    task_id text null references task_runs (task_id) on delete set null,
    target_wallet text not null,
    family text null,
    strategy_name text null,
    source_type text not null,
    source_ref text not null,
    score_overall double precision null,
    score_breakdown jsonb not null default '{}'::jsonb,
    metrics jsonb not null default '{}'::jsonb,
    failure_tags text[] not null default '{}'::text[],
    artifact_paths jsonb not null default '[]'::jsonb,
    notes jsonb not null default '{}'::jsonb,
    conclusion text null,
    created_at timestamptz not null default now()
);

create index if not exists experiments_target_wallet_idx on experiments (target_wallet, created_at desc);
create index if not exists hypotheses_experiment_idx on hypotheses (experiment_id, created_at desc);
create index if not exists evaluations_experiment_idx on evaluations (experiment_id, created_at desc);
create index if not exists evaluations_hypothesis_idx on evaluations (hypothesis_id, created_at desc);
create index if not exists evaluations_strategy_run_idx on evaluations (strategy_run_id);
create index if not exists evaluations_task_id_idx on evaluations (task_id);
