use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::model::{CurveCompletedEvent, MintCreatedEvent, PumpEvent, TradeEvent};

const PROGRAM_DATA_PREFIX: &str = "Program data: ";
const CREATE_EVENT_DISCRIMINATOR: [u8; 8] = [27, 114, 169, 77, 222, 235, 99, 118];
const TRADE_EVENT_DISCRIMINATOR: [u8; 8] = [189, 219, 127, 211, 78, 230, 97, 238];
const COMPLETE_EVENT_DISCRIMINATOR: [u8; 8] = [95, 114, 97, 156, 212, 46, 152, 8];

pub fn decode_anchor_events_from_logs(logs: &[String]) -> Result<Vec<PumpEvent>> {
    let mut events = Vec::new();

    for log in logs {
        let Some(payload) = log.strip_prefix(PROGRAM_DATA_PREFIX) else {
            continue;
        };

        if let Some(event) = decode_anchor_event_payload(payload)? {
            events.push(event);
        }
    }

    Ok(events)
}

fn decode_anchor_event_payload(payload: &str) -> Result<Option<PumpEvent>> {
    let raw = match STANDARD.decode(payload.trim()) {
        Ok(raw) => raw,
        Err(_) => return Ok(None),
    };

    if raw.len() < 8 {
        return Ok(None);
    }

    let (discriminator, data) = raw.split_at(8);
    let event = if discriminator == CREATE_EVENT_DISCRIMINATOR.as_slice() {
        let event = CreateEventWire::try_from_slice(data)
            .context("failed to decode Pump CreateEvent payload")?;
        Some(PumpEvent::MintCreated(event.into_model()))
    } else if discriminator == TRADE_EVENT_DISCRIMINATOR.as_slice() {
        let event = TradeEventWire::try_from_slice(data)
            .context("failed to decode Pump TradeEvent payload")?;
        Some(PumpEvent::Trade(event.into_model()))
    } else if discriminator == COMPLETE_EVENT_DISCRIMINATOR.as_slice() {
        let event = CompleteEventWire::try_from_slice(data)
            .context("failed to decode Pump CompleteEvent payload")?;
        Some(PumpEvent::CurveCompleted(event.into_model()))
    } else {
        None
    };

    Ok(event)
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
struct CreateEventWire {
    name: String,
    symbol: String,
    uri: String,
    mint: [u8; 32],
    bonding_curve: [u8; 32],
    user: [u8; 32],
    creator: [u8; 32],
    timestamp: i64,
    virtual_token_reserves: u64,
    virtual_sol_reserves: u64,
    real_token_reserves: u64,
    token_total_supply: u64,
    token_program: [u8; 32],
    is_mayhem_mode: bool,
    is_cashback_enabled: bool,
}

impl CreateEventWire {
    fn into_model(self) -> MintCreatedEvent {
        MintCreatedEvent {
            mint: encode_pubkey(&self.mint),
            bonding_curve: encode_pubkey(&self.bonding_curve),
            user: encode_pubkey(&self.user),
            creator: encode_pubkey(&self.creator),
            name: self.name,
            symbol: self.symbol,
            uri: self.uri,
            timestamp: self.timestamp,
            virtual_token_reserves: self.virtual_token_reserves,
            virtual_sol_reserves: self.virtual_sol_reserves,
            real_token_reserves: self.real_token_reserves,
            token_total_supply: self.token_total_supply,
            token_program: encode_pubkey(&self.token_program),
            is_mayhem_mode: self.is_mayhem_mode,
            is_cashback_enabled: self.is_cashback_enabled,
        }
    }
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
struct TradeEventWire {
    mint: [u8; 32],
    sol_amount: u64,
    token_amount: u64,
    is_buy: bool,
    user: [u8; 32],
    timestamp: i64,
    virtual_sol_reserves: u64,
    virtual_token_reserves: u64,
    real_sol_reserves: u64,
    real_token_reserves: u64,
    fee_recipient: [u8; 32],
    fee_basis_points: u64,
    fee: u64,
    creator: [u8; 32],
    creator_fee_basis_points: u64,
    creator_fee: u64,
    track_volume: bool,
    total_unclaimed_tokens: u64,
    total_claimed_tokens: u64,
    current_sol_volume: u64,
    last_update_timestamp: i64,
    ix_name: String,
    mayhem_mode: bool,
    cashback_fee_basis_points: u64,
    cashback: u64,
}

impl TradeEventWire {
    fn into_model(self) -> TradeEvent {
        TradeEvent {
            mint: encode_pubkey(&self.mint),
            sol_amount: self.sol_amount,
            token_amount: self.token_amount,
            is_buy: self.is_buy,
            user: encode_pubkey(&self.user),
            timestamp: self.timestamp,
            virtual_sol_reserves: self.virtual_sol_reserves,
            virtual_token_reserves: self.virtual_token_reserves,
            real_sol_reserves: self.real_sol_reserves,
            real_token_reserves: self.real_token_reserves,
            fee_recipient: encode_pubkey(&self.fee_recipient),
            fee_basis_points: self.fee_basis_points,
            fee: self.fee,
            creator: encode_pubkey(&self.creator),
            creator_fee_basis_points: self.creator_fee_basis_points,
            creator_fee: self.creator_fee,
            track_volume: self.track_volume,
            total_unclaimed_tokens: self.total_unclaimed_tokens,
            total_claimed_tokens: self.total_claimed_tokens,
            current_sol_volume: self.current_sol_volume,
            last_update_timestamp: self.last_update_timestamp,
            ix_name: self.ix_name,
            mayhem_mode: self.mayhem_mode,
            cashback_fee_basis_points: self.cashback_fee_basis_points,
            cashback: self.cashback,
        }
    }
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
struct CompleteEventWire {
    user: [u8; 32],
    mint: [u8; 32],
    bonding_curve: [u8; 32],
    timestamp: i64,
}

impl CompleteEventWire {
    fn into_model(self) -> CurveCompletedEvent {
        CurveCompletedEvent {
            mint: encode_pubkey(&self.mint),
            bonding_curve: encode_pubkey(&self.bonding_curve),
            user: encode_pubkey(&self.user),
            timestamp: self.timestamp,
        }
    }
}

fn encode_pubkey(bytes: &[u8; 32]) -> String {
    bs58::encode(bytes).into_string()
}

#[cfg(test)]
mod tests {
    use super::{
        COMPLETE_EVENT_DISCRIMINATOR, CREATE_EVENT_DISCRIMINATOR, CompleteEventWire,
        CreateEventWire, PROGRAM_DATA_PREFIX, TRADE_EVENT_DISCRIMINATOR, TradeEventWire,
        decode_anchor_events_from_logs,
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use borsh::BorshSerialize;

    use crate::model::PumpEvent;

    #[test]
    fn decodes_create_event_from_anchor_log() {
        let wire = CreateEventWire {
            name: "Foo".to_string(),
            symbol: "FOO".to_string(),
            uri: "https://example.com".to_string(),
            mint: [1; 32],
            bonding_curve: [2; 32],
            user: [3; 32],
            creator: [4; 32],
            timestamp: 100,
            virtual_token_reserves: 10,
            virtual_sol_reserves: 20,
            real_token_reserves: 30,
            token_total_supply: 40,
            token_program: [5; 32],
            is_mayhem_mode: false,
            is_cashback_enabled: true,
        };
        let log = encode_log(CREATE_EVENT_DISCRIMINATOR, &wire);

        let events = decode_anchor_events_from_logs(&[log]).expect("decoder should succeed");
        assert_eq!(events.len(), 1);
        match &events[0] {
            PumpEvent::MintCreated(event) => {
                assert_eq!(event.name, "Foo");
                assert_eq!(event.symbol, "FOO");
                assert_eq!(event.timestamp, 100);
                assert!(event.is_cashback_enabled);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn decodes_trade_and_complete_events_from_anchor_logs() {
        let trade = TradeEventWire {
            mint: [1; 32],
            sol_amount: 100,
            token_amount: 50,
            is_buy: true,
            user: [2; 32],
            timestamp: 101,
            virtual_sol_reserves: 1000,
            virtual_token_reserves: 2000,
            real_sol_reserves: 10,
            real_token_reserves: 20,
            fee_recipient: [3; 32],
            fee_basis_points: 100,
            fee: 1,
            creator: [4; 32],
            creator_fee_basis_points: 50,
            creator_fee: 1,
            track_volume: true,
            total_unclaimed_tokens: 0,
            total_claimed_tokens: 0,
            current_sol_volume: 100,
            last_update_timestamp: 101,
            ix_name: "buy".to_string(),
            mayhem_mode: false,
            cashback_fee_basis_points: 0,
            cashback: 0,
        };
        let complete = CompleteEventWire {
            user: [9; 32],
            mint: [1; 32],
            bonding_curve: [8; 32],
            timestamp: 102,
        };

        let events = decode_anchor_events_from_logs(&[
            encode_log(TRADE_EVENT_DISCRIMINATOR, &trade),
            encode_log(COMPLETE_EVENT_DISCRIMINATOR, &complete),
        ])
        .expect("decoder should succeed");

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], PumpEvent::Trade(_)));
        assert!(matches!(events[1], PumpEvent::CurveCompleted(_)));
    }

    fn encode_log<T>(discriminator: [u8; 8], value: &T) -> String
    where
        T: BorshSerialize,
    {
        let mut bytes = discriminator.to_vec();
        bytes.extend_from_slice(&borsh::to_vec(value).expect("borsh encode"));
        format!("{PROGRAM_DATA_PREFIX}{}", STANDARD.encode(bytes))
    }
}
