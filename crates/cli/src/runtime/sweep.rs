use std::{fmt::Display, str::FromStr};

use anyhow::{Result, bail};

use crate::args::{StrategyArgs, SweepDbArgs};

pub fn build_sweep_variants(base: &StrategyArgs, sweep: &SweepDbArgs) -> Result<Vec<StrategyArgs>> {
    let mut variants = vec![StrategyArgs {
        strategy_config: None,
        ..base.clone()
    }];

    let buy_sol_values = parse_csv_values::<f64>(&sweep.buy_sol_values, "buy_sol_values")?
        .unwrap_or_else(|| vec![base.buy_sol]);
    variants = expand_variants(variants, &buy_sol_values, |args, value| {
        args.buy_sol = value
    });

    let max_age_secs_values =
        parse_csv_values::<i64>(&sweep.max_age_secs_values, "max_age_secs_values")?
            .unwrap_or_else(|| vec![base.max_age_secs]);
    variants = expand_variants(variants, &max_age_secs_values, |args, value| {
        args.max_age_secs = value
    });

    let min_buy_count_values =
        parse_csv_values::<u64>(&sweep.min_buy_count_values, "min_buy_count_values")?
            .unwrap_or_else(|| vec![base.min_buy_count]);
    variants = expand_variants(variants, &min_buy_count_values, |args, value| {
        args.min_buy_count = value
    });

    let min_unique_buyers_values =
        parse_csv_values::<usize>(&sweep.min_unique_buyers_values, "min_unique_buyers_values")?
            .unwrap_or_else(|| vec![base.min_unique_buyers]);
    variants = expand_variants(variants, &min_unique_buyers_values, |args, value| {
        args.min_unique_buyers = value
    });

    let min_total_buy_sol_values =
        parse_csv_values::<f64>(&sweep.min_total_buy_sol_values, "min_total_buy_sol_values")?
            .unwrap_or_else(|| vec![base.min_total_buy_sol]);
    variants = expand_variants(variants, &min_total_buy_sol_values, |args, value| {
        args.min_total_buy_sol = value
    });

    let max_sell_count_values =
        parse_csv_values::<u64>(&sweep.max_sell_count_values, "max_sell_count_values")?
            .unwrap_or_else(|| vec![base.max_sell_count]);
    variants = expand_variants(variants, &max_sell_count_values, |args, value| {
        args.max_sell_count = value
    });

    let min_buy_sell_ratio_values = parse_csv_values::<f64>(
        &sweep.min_buy_sell_ratio_values,
        "min_buy_sell_ratio_values",
    )?
    .unwrap_or_else(|| vec![base.min_buy_sell_ratio]);
    variants = expand_variants(variants, &min_buy_sell_ratio_values, |args, value| {
        args.min_buy_sell_ratio = value
    });

    let take_profit_bps_values =
        parse_csv_values::<i64>(&sweep.take_profit_bps_values, "take_profit_bps_values")?
            .unwrap_or_else(|| vec![base.take_profit_bps]);
    variants = expand_variants(variants, &take_profit_bps_values, |args, value| {
        args.take_profit_bps = value
    });

    let stop_loss_bps_values =
        parse_csv_values::<i64>(&sweep.stop_loss_bps_values, "stop_loss_bps_values")?
            .unwrap_or_else(|| vec![base.stop_loss_bps]);
    variants = expand_variants(variants, &stop_loss_bps_values, |args, value| {
        args.stop_loss_bps = value
    });

    let max_concurrent_positions_values = parse_csv_values::<usize>(
        &sweep.max_concurrent_positions_values,
        "max_concurrent_positions_values",
    )?
    .unwrap_or_else(|| vec![base.max_concurrent_positions]);
    variants = expand_variants(variants, &max_concurrent_positions_values, |args, value| {
        args.max_concurrent_positions = value
    });

    let exit_on_sell_count_values = parse_csv_values::<u64>(
        &sweep.exit_on_sell_count_values,
        "exit_on_sell_count_values",
    )?
    .unwrap_or_else(|| vec![base.exit_on_sell_count]);
    variants = expand_variants(variants, &exit_on_sell_count_values, |args, value| {
        args.exit_on_sell_count = value
    });

    Ok(variants)
}

fn expand_variants<T, F>(
    variants: Vec<StrategyArgs>,
    values: &[T],
    mut apply: F,
) -> Vec<StrategyArgs>
where
    T: Clone,
    F: FnMut(&mut StrategyArgs, T),
{
    let mut expanded = Vec::with_capacity(variants.len() * values.len());
    for variant in variants {
        for value in values {
            let mut next = variant.clone();
            apply(&mut next, value.clone());
            expanded.push(next);
        }
    }
    expanded
}

fn parse_csv_values<T>(raw: &Option<String>, label: &str) -> Result<Option<Vec<T>>>
where
    T: FromStr,
    <T as FromStr>::Err: Display,
{
    let Some(raw) = raw else {
        return Ok(None);
    };

    let values = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<T>()
                .map_err(|error| anyhow::anyhow!("invalid {} entry '{}': {}", label, value, error))
        })
        .collect::<Result<Vec<_>>>()?;

    if values.is_empty() {
        bail!("{} cannot be empty", label);
    }

    Ok(Some(values))
}

#[cfg(test)]
mod tests {
    use crate::args::{StrategyArgs, SweepDbArgs};

    use super::build_sweep_variants;

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

    fn sample_sweep_args() -> SweepDbArgs {
        SweepDbArgs {
            database_url: None,
            max_db_connections: 5,
            top: 10,
            buy_sol_values: None,
            max_age_secs_values: None,
            min_buy_count_values: None,
            min_unique_buyers_values: None,
            min_total_buy_sol_values: None,
            max_sell_count_values: None,
            min_buy_sell_ratio_values: None,
            take_profit_bps_values: None,
            stop_loss_bps_values: None,
            max_concurrent_positions_values: None,
            exit_on_sell_count_values: None,
            strategy: sample_strategy_args(),
        }
    }

    #[test]
    fn build_sweep_variants_expands_cartesian_product() {
        let base = sample_strategy_args();
        let mut sweep = sample_sweep_args();
        sweep.buy_sol_values = Some("0.1,0.2".to_string());
        sweep.max_sell_count_values = Some("0,1".to_string());

        let variants = build_sweep_variants(&base, &sweep).expect("variants should build");
        assert_eq!(variants.len(), 4);
        assert!(
            variants
                .iter()
                .all(|variant| variant.strategy_config.is_none())
        );
        assert!(
            variants
                .iter()
                .any(|variant| variant.buy_sol == 0.1 && variant.max_sell_count == 0)
        );
        assert!(
            variants
                .iter()
                .any(|variant| variant.buy_sol == 0.1 && variant.max_sell_count == 1)
        );
        assert!(
            variants
                .iter()
                .any(|variant| variant.buy_sol == 0.2 && variant.max_sell_count == 0)
        );
        assert!(
            variants
                .iter()
                .any(|variant| variant.buy_sol == 0.2 && variant.max_sell_count == 1)
        );
    }

    #[test]
    fn build_sweep_variants_rejects_empty_csv() {
        let base = sample_strategy_args();
        let mut sweep = sample_sweep_args();
        sweep.buy_sol_values = Some(" , ".to_string());

        let error = build_sweep_variants(&base, &sweep).expect_err("empty csv should fail");
        assert!(error.to_string().contains("buy_sol_values cannot be empty"));
    }
}
