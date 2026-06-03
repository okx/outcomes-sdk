//! EIP-712 Agent signing for OKX Outcomes transactions.
//!
//! Enabled by the `signing` Cargo feature. The pipeline is:
//! [`Action`] -> rmp-serde msgpack envelope ([`mod@msgpack`]) -> keccak256
//! ([`connection_id`](msgpack::connection_id)) -> EIP-712 Agent struct hash
//! ([`mod@eip712`]) -> ECDSA over k256 ([`mod@ecdsa_sign`] / [`signer`]).
//!
//! Pulls in `k256`, `rmp-serde`, and the minimum-feature alloy crates
//! (`alloy-primitives`, `alloy-sol-types`, `alloy-signer`,
//! `alloy-signer-local`) when this feature is active.

pub mod action;
pub mod chain_type;
pub mod client_order_id;
mod ecdsa_sign;
mod eip712;
mod hex;
mod msgpack;
pub mod signer;
pub mod tx_signature;
pub mod types;

// Re-export public API
pub use action::{
    action_cancel, action_cancel_all, action_place_order, action_prediction_merge,
    action_prediction_redeem, action_prediction_split, Action, Grouping,
};
pub use client_order_id::{
    generate_client_order_id, generate_client_order_id_default, parse_client_order_id_prefix,
    parse_env_str, parse_region_str, register_client_order_id_context, validate_client_order_id,
    ClientOrderIdPrefix, Env, Region, CLIENT_ORDER_ID_LEN,
};
pub use ecdsa_sign::{
    ecrecover, now_millis, parse_private_key, sign_action, sign_action_debug, sign_action_full,
    signer_address, SigningDebug,
};
pub use types::{
    CancelRequest, CancelTarget, LimitOrderType, LimitTif, OrderRequest, OrderType,
    SigningOrderSide, SizeType,
};

use crate::models::common::SignatureWrapper;

/// Sign an action and return a [`SignatureWrapper`] ready to embed in a request body.
///
/// Routes through the typed [`signer::sign_action`] pipeline and converts the
/// resulting [`tx_signature::TxSignature`] to the OKX wire format via
/// `From<TxSignature> for SignatureWrapper`.
pub fn sign_to_wrapper(
    action: &Action,
    nonce: u64,
    expires_after: Option<u64>,
    key: &k256::ecdsa::SigningKey,
) -> Result<SignatureWrapper, String> {
    let source_chain = eip712::chain_env();
    let tx_sig = signer::sign_action(action, key, nonce, expires_after, None, source_chain)?;
    Ok(tx_sig.into())
}

// -- Tests ----------------------------------------

#[cfg(test)]
mod tests {
    use super::action::*;
    use super::chain_type::ChainType;
    use super::ecdsa_sign::*;
    use super::eip712::*;
    use super::hex::hex_encode;
    use super::msgpack::*;
    use super::types::{
        CancelRequest, CancelTarget, LimitOrderType, LimitTif, OrderRequest, OrderType, SizeType,
    };
    use k256::ecdsa::SigningKey;

    fn test_key() -> SigningKey {
        let bytes = [0x01u8; 32];
        SigningKey::from_bytes((&bytes).into()).expect("test key")
    }

    /// Build a typed limit order with default fields, mirroring the JSON shape
    /// the old tests used. `tif` is the limit time-in-force string (`"gtc"`,
    /// `"ioc"`, etc.).
    fn make_limit_order(
        asset_id: &str,
        side: &str,
        price: &str,
        size: &str,
        tif: &str,
    ) -> OrderRequest {
        OrderRequest {
            asset_id: asset_id.to_string(),
            side: side.parse().expect("test helper got invalid side"),
            market_type: "prediction".to_string(),
            client_order_id: None,
            price: price.to_string(),
            reduce_only: false,
            size: size.to_string(),
            size_type: SizeType::Base,
            order_type: OrderType::Limit(LimitOrderType {
                tif: parse_tif(tif),
            }),
        }
    }

    fn parse_tif(s: &str) -> LimitTif {
        match s {
            "gtc" => LimitTif::Gtc,
            "ioc" => LimitTif::Ioc,
            "fok" => LimitTif::Fok,
            "alo" => LimitTif::Alo,
            other => panic!("test helper got unsupported tif: {other}"),
        }
    }

    /// Build a cancel-by-oid request.
    fn make_cancel_oid(asset_id: &str, oid: &str) -> CancelRequest {
        CancelRequest {
            asset_id: asset_id.to_string(),
            market_type: "prediction".to_string(),
            target: CancelTarget::Oid(oid.to_string()),
        }
    }

    #[test]
    fn sign_split_produces_valid_signature() {
        let action = action_prediction_split("123", "100.5");
        let nonce = 1711094400000u64;
        let sig = sign_action(&action, nonce, None, None, &test_key());
        assert!(sig.is_ok());
        let sig = sig.unwrap_or_default();
        assert!(sig.starts_with("0x"));
        assert_eq!(sig.len(), 132); // 65 bytes = 130 hex + "0x"
    }

    #[test]
    fn sign_split_is_deterministic() {
        let action = action_prediction_split("1", "10");
        let nonce = 1000u64;
        let sig1 = sign_action(&action, nonce, None, None, &test_key());
        let sig2 = sign_action(&action, nonce, None, None, &test_key());
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let action = action_prediction_split("1", "10");
        let nonce = 1000u64;
        let key = test_key();
        let dbg = sign_action_debug(&action, nonce, None, None, &key);
        assert!(dbg.is_ok());
        let dbg = dbg.unwrap_or_else(|_| unreachable!());

        let recovered = ecrecover(&dbg.signing_hash, &dbg.signature);
        assert!(recovered.is_ok());
        assert_eq!(
            recovered.unwrap_or_default().to_lowercase(),
            dbg.signer_address.to_lowercase()
        );
    }

    #[test]
    fn different_actions_produce_different_signatures() {
        let split = action_prediction_split("1", "10");
        let merge = action_prediction_merge("1", "10");
        let nonce = 1000u64;
        let key = test_key();
        let sig_split = sign_action(&split, nonce, None, None, &key);
        let sig_merge = sign_action(&merge, nonce, None, None, &key);
        assert_ne!(sig_split, sig_merge);
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    #[test]
    fn msgpack_serialization_includes_type_tag() {
        let action = action_prediction_split("42", "100");
        let bytes = build_unsigned_tx_msgpack(&action, 999, None, None);
        assert!(find_bytes(&bytes, b"predictionSplit").is_some());
        assert!(find_bytes(&bytes, b"marketId").is_some());
        assert!(find_bytes(&bytes, b"42").is_some());
    }

    #[test]
    fn domain_separator_snapshot_mainnet() {
        let ds = domain_separator(ChainType::Mainnet.chain_id());
        let ds_hex = format!("0x{}", hex_encode(&ds));
        assert_eq!(ds_hex.len(), 66);
        let expected = "0x0f8677bc4b76654f0e82c0daa2fd3feb46384b8f7e86a9a2dc1cb93c0d7871cb";
        assert_eq!(
            ds_hex, expected,
            "mainnet domain separator changed - verify against OKX OpenAPI spec"
        );
    }

    #[test]
    fn agent_struct_hash_snapshot() {
        let test_conn_id = keccak256(b"test");
        let sh = agent_struct_hash("Mainnet", &test_conn_id);
        let sh_hex = format!("0x{}", hex_encode(&sh));
        assert_eq!(sh_hex.len(), 66);
        let expected = "0xbac975248faf52d6af1f0d8a76f9830810b919fe5200af7d3e40663ca9b24d6a";
        assert_eq!(
            sh_hex, expected,
            "agent struct hash changed - verify against OKX OpenAPI spec"
        );
    }

    #[test]
    fn signer_address_from_test_key() {
        let addr = signer_address(&test_key());
        assert_eq!(addr, "0x1a642f0e3c3af545e7acbd38b07251b3990914f1");
    }

    #[test]
    fn connection_id_is_msgpack_hash() {
        let action = action_prediction_split("1", "10");
        let nonce = 1000u64;
        let conn = connection_id(&action, nonce, None, None).expect("connection_id");
        let bytes = build_unsigned_tx_msgpack(&action, nonce, None, None);
        let expected_hash = keccak256(&bytes);
        assert_eq!(
            conn, expected_hash,
            "connectionId mismatch - msgpack serialization diverged"
        );
        assert!(bytes.len() > 20, "msgpack too short: {} bytes", bytes.len());
    }

    #[test]
    fn parse_private_key_works() {
        let hex = "0x0101010101010101010101010101010101010101010101010101010101010101";
        assert!(parse_private_key(hex).is_ok());
        let hex2 = "0101010101010101010101010101010101010101010101010101010101010101";
        assert!(parse_private_key(hex2).is_ok());
    }

    #[test]
    fn place_order_msgpack_has_correct_field_order() {
        let order = make_limit_order("0", "sell", "0.5", "10.0", "gtc");
        let action = action_place_order(vec![order]);
        let bytes = build_unsigned_tx_msgpack(&action, 1774937221778u64, None, None);

        let type_pos = find_bytes(&bytes, b"placeOrder").expect("placeOrder missing");
        let grouping_pos = find_bytes(&bytes, b"grouping").expect("grouping missing");
        let orders_pos = find_bytes(&bytes, b"orders").expect("orders missing");
        let asset_pos = find_bytes(&bytes, b"assetId").expect("assetId missing");
        let side_pos = find_bytes(&bytes, b"side").expect("side missing");
        let market_pos = find_bytes(&bytes, b"marketType").expect("marketType missing");
        let price_pos = find_bytes(&bytes, b"price").expect("price missing");
        let reduce_pos = find_bytes(&bytes, b"reduceOnly").expect("reduceOnly missing");
        let size_pos = find_bytes(&bytes, b"10.0").expect("size value missing");
        let order_type_pos = find_bytes(&bytes, b"orderType").expect("orderType missing");

        assert!(type_pos < grouping_pos, "type before grouping");
        assert!(grouping_pos < orders_pos, "grouping before orders");
        assert!(orders_pos < asset_pos, "orders before assetId");
        assert!(asset_pos < side_pos, "assetId before side");
        assert!(side_pos < market_pos, "side before marketType");
        assert!(market_pos < price_pos, "marketType before price");
        assert!(price_pos < reduce_pos, "price before reduceOnly");
        assert!(reduce_pos < size_pos, "reduceOnly before size");
        assert!(size_pos < order_type_pos, "size before orderType");

        assert!(find_bytes(&bytes, b"limit").is_some(), "limit missing");
        assert!(find_bytes(&bytes, b"tif").is_some(), "tif missing");
        assert!(find_bytes(&bytes, b"gtc").is_some(), "gtc missing");
    }

    #[test]
    fn cancel_msgpack_has_correct_field_order() {
        let cancel_item = make_cancel_oid("0", "123456789");
        let action = action_cancel(vec![cancel_item]);
        let bytes = build_unsigned_tx_msgpack(&action, 1000u64, None, None);

        let type_pos = find_bytes(&bytes, b"cancel").expect("cancel missing");
        let cancels_pos = find_bytes(&bytes, b"cancels").expect("cancels missing");
        let asset_pos = find_bytes(&bytes, b"assetId").expect("assetId missing");
        let market_pos = find_bytes(&bytes, b"marketType").expect("marketType missing");

        assert!(type_pos < cancels_pos, "type before cancels");
        assert!(cancels_pos < asset_pos, "cancels before assetId");
        assert!(asset_pos < market_pos, "assetId before marketType");
        assert!(find_bytes(&bytes, b"oid").is_some(), "oid key missing");
        assert!(
            find_bytes(&bytes, b"123456789").is_some(),
            "oid string value missing"
        );
    }

    #[test]
    fn place_order_signing_is_deterministic() {
        let order = make_limit_order("0", "buy", "0.4", "5.0", "gtc");
        let key = test_key();
        let nonce = 1774937221778u64;

        let action1 = action_place_order(vec![order.clone()]);
        let action2 = action_place_order(vec![order]);

        let r1 = sign_action_full(&action1, nonce, None, None, &key).expect("sign1");
        let r2 = sign_action_full(&action2, nonce, None, None, &key).expect("sign2");
        assert_eq!(r1, r2, "signing must be deterministic");
    }

    // -- Action constructor tests ------------------------------------

    #[test]
    fn action_constructors_produce_correct_variants() {
        let split = action_prediction_split("1", "100");
        assert!(matches!(split, Action::PredictionSplit { .. }));

        let merge = action_prediction_merge("1", "50");
        assert!(matches!(merge, Action::PredictionMerge { .. }));

        let redeem = action_prediction_redeem("1");
        assert!(matches!(redeem, Action::PredictionRedeem { .. }));

        let place = action_place_order(vec![make_limit_order("0", "buy", "0.5", "1", "gtc")]);
        assert!(matches!(place, Action::PlaceOrder { .. }));

        let cancel = action_cancel(vec![make_cancel_oid("0", "123")]);
        assert!(matches!(cancel, Action::Cancel { .. }));

        let cancel_all = action_cancel_all(vec![], "prediction");
        assert!(matches!(cancel_all, Action::CancelAll { .. }));
    }

    // -- Msgpack tests for missing action types ----------------------

    #[test]
    fn merge_msgpack_has_correct_fields() {
        let action = action_prediction_merge("7", "250");
        let bytes = build_unsigned_tx_msgpack(&action, 2000, None, None);
        assert!(find_bytes(&bytes, b"predictionMerge").is_some());
        assert!(find_bytes(&bytes, b"marketId").is_some());
        assert!(find_bytes(&bytes, b"7").is_some());
        assert!(find_bytes(&bytes, b"250").is_some());
    }

    #[test]
    fn redeem_msgpack_has_correct_fields() {
        let action = action_prediction_redeem("99");
        let bytes = build_unsigned_tx_msgpack(&action, 3000, None, None);
        assert!(find_bytes(&bytes, b"predictionRedeem").is_some());
        assert!(find_bytes(&bytes, b"marketId").is_some());
        assert!(find_bytes(&bytes, b"99").is_some());
        // redeem has no "size" field
        assert!(
            find_bytes(&bytes, b"size").is_none(),
            "redeem should not have size field"
        );
    }

    #[test]
    fn cancel_all_msgpack_has_correct_fields_empty_asset_ids() {
        // All-markets cancel (assetIds = []) - type tag, empty assetIds array,
        // and marketType must all be present in the signed bytes.
        let action = action_cancel_all(vec![], "prediction");
        let bytes = build_unsigned_tx_msgpack(&action, 4000, None, None);
        assert!(
            find_bytes(&bytes, b"cancelAll").is_some(),
            "type tag missing"
        );
        assert!(
            find_bytes(&bytes, b"assetIds").is_some(),
            "assetIds missing"
        );
        assert!(
            find_bytes(&bytes, b"marketType").is_some(),
            "marketType missing"
        );
        assert!(
            find_bytes(&bytes, b"prediction").is_some(),
            "marketType value missing"
        );
    }

    #[test]
    fn cancel_all_msgpack_has_correct_fields_with_asset_ids() {
        // Per-market cancel (assetIds = ["100288000", "100328000"]) - both IDs
        // must appear in the signed bytes; order matches input order.
        let action = action_cancel_all(
            vec!["100288000".to_string(), "100328000".to_string()],
            "prediction",
        );
        let bytes = build_unsigned_tx_msgpack(&action, 4000, None, None);
        assert!(
            find_bytes(&bytes, b"cancelAll").is_some(),
            "type tag missing"
        );
        assert!(
            find_bytes(&bytes, b"100288000").is_some(),
            "first assetId missing"
        );
        assert!(
            find_bytes(&bytes, b"100328000").is_some(),
            "second assetId missing"
        );
        let first_pos = find_bytes(&bytes, b"100288000").expect("first");
        let second_pos = find_bytes(&bytes, b"100328000").expect("second");
        assert!(first_pos < second_pos, "assetIds order must be preserved");
    }

    // -- Order type variant tests ------------------------------------

    #[test]
    fn place_order_gtd_order_type() {
        let order = OrderRequest {
            order_type: OrderType::Limit(LimitOrderType {
                tif: LimitTif::Gtd {
                    expires_after: 1800000000000u64,
                },
            }),
            ..make_limit_order("1", "buy", "0.5", "10", "gtc")
        };
        let action = action_place_order(vec![order]);
        let bytes = build_unsigned_tx_msgpack(&action, 1000, None, None);
        assert!(find_bytes(&bytes, b"limit").is_some(), "limit missing");
        assert!(find_bytes(&bytes, b"tif").is_some(), "tif missing");
        assert!(find_bytes(&bytes, b"gtd").is_some(), "gtd missing");
        assert!(
            find_bytes(&bytes, b"expiresAfter").is_some(),
            "expiresAfter missing"
        );
    }

    // -- Optional field tests ----------------------------------------

    #[test]
    fn place_order_with_client_order_id() {
        let order = OrderRequest {
            client_order_id: Some("my-custom-id-123".to_string()),
            ..make_limit_order("1", "buy", "0.5", "10", "gtc")
        };
        let action = action_place_order(vec![order]);
        let bytes = build_unsigned_tx_msgpack(&action, 1000, None, None);
        assert!(
            find_bytes(&bytes, b"clientOrderId").is_some(),
            "clientOrderId key missing"
        );
        assert!(
            find_bytes(&bytes, b"my-custom-id-123").is_some(),
            "clientOrderId value missing"
        );
        // Verify field order: clientOrderId comes after marketType, before price
        let market_pos = find_bytes(&bytes, b"marketType").expect("marketType");
        let client_order_id_pos = find_bytes(&bytes, b"clientOrderId").expect("clientOrderId");
        let price_pos = find_bytes(&bytes, b"price").expect("price");
        assert!(
            market_pos < client_order_id_pos,
            "marketType before clientOrderId"
        );
        assert!(
            client_order_id_pos < price_pos,
            "clientOrderId before price"
        );
    }

    #[test]
    fn place_order_with_size_type_quote() {
        let order = OrderRequest {
            size_type: SizeType::Quote,
            ..make_limit_order("1", "buy", "0.5", "100", "gtc")
        };
        let action = action_place_order(vec![order]);
        let bytes = build_unsigned_tx_msgpack(&action, 1000, None, None);
        assert!(
            find_bytes(&bytes, b"sizeType").is_some(),
            "sizeType key missing"
        );
        assert!(
            find_bytes(&bytes, b"quote").is_some(),
            "sizeType quote value missing"
        );
    }

    #[test]
    fn place_order_base_size_type_is_skipped() {
        let order = OrderRequest {
            size_type: SizeType::Base,
            ..make_limit_order("1", "buy", "0.5", "10", "gtc")
        };
        let action = action_place_order(vec![order]);
        let bytes = build_unsigned_tx_msgpack(&action, 1000, None, None);
        // "base" is the default - should be skipped
        assert!(
            find_bytes(&bytes, b"sizeType").is_none(),
            "sizeType=base should be skipped"
        );
    }

    // -- Cancel with client order id ----------------------------------------

    #[test]
    fn cancel_with_client_order_id() {
        let cancel_item = CancelRequest {
            asset_id: "0".to_string(),
            market_type: "prediction".to_string(),
            target: CancelTarget::ClientOrderId("my-order-42".to_string()),
        };
        let action = action_cancel(vec![cancel_item]);
        let bytes = build_unsigned_tx_msgpack(&action, 1000, None, None);
        assert!(
            find_bytes(&bytes, b"clientOrderId").is_some(),
            "clientOrderId key missing"
        );
        assert!(
            find_bytes(&bytes, b"my-order-42").is_some(),
            "client order id value missing"
        );
    }

    // -- Unsigned tx with optional fields -----------------------------

    #[test]
    fn unsigned_tx_with_expires_after() {
        let action = action_prediction_split("1", "10");
        let bytes = build_unsigned_tx_msgpack(&action, 1000, Some(2000), None);
        assert!(
            find_bytes(&bytes, b"expiresAfter").is_some(),
            "expiresAfter missing"
        );
    }

    #[test]
    fn unsigned_tx_with_user() {
        let action = action_prediction_split("1", "10");
        let bytes = build_unsigned_tx_msgpack(&action, 1000, None, Some("0xabc123"));
        assert!(find_bytes(&bytes, b"user").is_some(), "user field missing");
        assert!(
            find_bytes(&bytes, b"0xabc123").is_some(),
            "user value missing"
        );
    }

    #[test]
    fn unsigned_tx_without_optional_fields() {
        let action = action_prediction_split("1", "10");
        let bytes = build_unsigned_tx_msgpack(&action, 1000, None, None);
        assert!(
            find_bytes(&bytes, b"expiresAfter").is_none(),
            "expiresAfter should be absent"
        );
        assert!(
            find_bytes(&bytes, b"user").is_none(),
            "user should be absent"
        );
    }

    // hex encode/decode tests live in `super::hex` (the canonical module).

    // -- now_millis ----------------------------------------

    #[test]
    fn now_millis_returns_reasonable_timestamp() {
        let ms = now_millis();
        // Should be after 2020-01-01 and before 2100-01-01
        assert!(ms > 1_577_836_800_000, "timestamp too small: {ms}");
        assert!(ms < 4_102_444_800_000, "timestamp too large: {ms}");
    }

    // -- ecrecover with known signature ------------------------------

    #[test]
    fn ecrecover_recovers_correct_address() {
        // Sign a known message with test key, then recover
        let key = test_key();
        let expected_addr = signer_address(&key);
        let action = action_prediction_split("1", "10");
        let dbg = sign_action_debug(&action, 1000, None, None, &key).expect("sign");

        let recovered = ecrecover(&dbg.signing_hash, &dbg.signature).expect("ecrecover");
        assert_eq!(recovered.to_lowercase(), expected_addr.to_lowercase());
    }

    #[test]
    fn ecrecover_invalid_hash_length() {
        assert!(ecrecover("0xdead", &("0x".to_owned() + &"00".repeat(65))).is_err());
    }

    #[test]
    fn ecrecover_invalid_sig_length() {
        assert!(ecrecover(&("0x".to_owned() + &"00".repeat(32)), "0xdead").is_err());
    }

    // -- parse_private_key edge cases --------------------------------

    #[test]
    fn parse_private_key_rejects_invalid_hex() {
        assert!(parse_private_key("not-valid-hex").is_err());
        assert!(parse_private_key(
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"
        )
        .is_err());
    }

    // -- OrderItem -> JSON -> msgpack flow ---------------------------

    /// Ensures the new `client_order_id` field on `OrderItem` survives the
    /// model -> JSON -> `action_place_order` -> msgpack pipeline as the
    /// camelCase `clientOrderId` key, so signatures cover the client order ID.
    #[test]
    fn order_item_client_order_id_flows_through_to_msgpack() {
        use crate::models::order::{OrderItem, OrderTypeSpec};

        let item = OrderItem {
            asset_id: "63000".to_string(),
            side: crate::models::order::SigningOrderSide::Buy,
            market_type: "prediction".to_string(),
            client_order_id: Some("0x00deadbeefcafe1234567890abcdef0123".to_string()),
            price: "0.5".to_string(),
            reduce_only: false,
            size: "1.0".to_string(),
            size_type: SizeType::Base,
            order_type: OrderTypeSpec {
                limit: LimitOrderType { tif: LimitTif::Gtc },
            },
        };
        let order_json = serde_json::to_value(&item).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            order_json.get("clientOrderId").and_then(|v| v.as_str()),
            Some("0x00deadbeefcafe1234567890abcdef0123"),
            "client_order_id must serialize as camelCase clientOrderId",
        );

        // Pin the struct field order to the canonical wire order. serde_json
        // emits keys in struct declaration order; if someone reorders the
        // struct again, this test fails before the divergence ships.
        let order_str = serde_json::to_string(&item).unwrap_or_else(|_| unreachable!());
        let canonical_keys = [
            "assetId",
            "side",
            "marketType",
            "clientOrderId",
            "price",
            "reduceOnly",
            "size",
            "orderType",
        ];
        let mut last_pos = 0;
        for key in canonical_keys {
            let pos = order_str
                .find(&format!("\"{key}\""))
                .unwrap_or_else(|| panic!("key {key} missing from {order_str}"));
            assert!(
                pos > last_pos,
                "key {key} out of canonical order at {pos} (prev {last_pos}) in {order_str}",
            );
            last_pos = pos;
        }

        // Confirm the client order ID value survives into the signed msgpack bytes when
        // the OrderItem is fed through the typed signing pipeline. All field types
        // now match across the wire/signing boundary, so it's a direct field copy.
        let signing_order = OrderRequest {
            asset_id: item.asset_id,
            side: item.side,
            market_type: item.market_type,
            client_order_id: item.client_order_id,
            price: item.price,
            reduce_only: item.reduce_only,
            size: item.size,
            size_type: item.size_type,
            order_type: OrderType::Limit(item.order_type.limit),
        };
        let action = action_place_order(vec![signing_order]);
        let bytes = build_unsigned_tx_msgpack(&action, 1u64, None, None);
        assert!(
            find_bytes(&bytes, b"clientOrderId").is_some(),
            "clientOrderId key missing from msgpack",
        );
        assert!(
            find_bytes(&bytes, b"0x00deadbeefcafe1234567890abcdef0123").is_some(),
            "clientOrderId value missing from msgpack",
        );
    }

    #[test]
    fn order_item_omits_client_order_id_when_none() {
        use crate::models::order::{OrderItem, OrderTypeSpec};

        let item = OrderItem {
            asset_id: "1".to_string(),
            side: crate::models::order::SigningOrderSide::Sell,
            market_type: "prediction".to_string(),
            client_order_id: None,
            price: "0.5".to_string(),
            reduce_only: false,
            size: "1".to_string(),
            size_type: SizeType::Base,
            order_type: OrderTypeSpec {
                limit: LimitOrderType { tif: LimitTif::Gtc },
            },
        };
        let order_json = serde_json::to_value(&item).unwrap_or_else(|_| unreachable!());
        assert!(
            order_json.get("clientOrderId").is_none(),
            "None client_order_id must be omitted entirely",
        );
    }

    // -- Specific request signature check ----------------------------

    /// Verify the signing pipeline against a known request payload + signature.
    ///
    /// We don't have the private key, so this isn't a sign-and-compare round-trip.
    /// Instead, we feed the request fields through the same EIP-712 pipeline and
    /// `ecrecover` the signer address from the supplied (r, s, v). If the msgpack
    /// serialization, struct hash, or signing hash logic ever drifts, the
    /// recovered address will change and this test will fail.
    #[test]
    fn ecrecover_matches_specific_place_order_request() {
        let order = make_limit_order("63000", "buy", "0.5", "1.0", "gtc");
        let action = action_place_order(vec![order]);
        let nonce = 1777030979418u64;
        let expires_after = Some(1777034579418u64);

        // Compute the EIP-712 signing hash directly (bypass env-driven chain_env).
        let conn_id =
            connection_id(&action, nonce, expires_after, None).unwrap_or_else(|_| unreachable!());
        let struct_hash = agent_struct_hash("Mainnet", &conn_id);
        let digest = eip712_signing_hash(ChainType::Mainnet.chain_id(), struct_hash);
        let digest_hex = format!("0x{}", hex_encode(&digest));

        // Reassemble 65-byte signature (r || s || v) from the request.
        let r = "3062926573fa8469207efb2db9c84895bbc9856a0cd2143e2b556999ac3f4dc1";
        let s = "1311976cf5130af88d4295674c196364bfd76974c41f85c2a298469781bbdb5f";
        let v: u8 = 1;
        let sig = format!("0x{r}{s}{v:02x}");

        // Pin the signing hash first - sharper failure signal if msgpack /
        // struct hash / domain separator changes.
        assert_eq!(
            digest_hex, "0x5d4d16c032be69891a190a7f26ac9ca2e4590d500f6fed69bcf234b2c6f294e7",
            "signing hash drift",
        );

        let recovered = ecrecover(&digest_hex, &sig).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            recovered.to_lowercase(),
            "0xcb3aae6816a93ea75cbcf5965aeb1f7302171872",
            "recovered signer drift (digest={digest_hex})",
        );
    }

    // -- Sample sanity check (client order id + signature, ad-hoc) -------------

    #[test]
    fn sample_request_client_order_id_and_signature_check() {
        use crate::signing::{
            parse_client_order_id_prefix, validate_client_order_id, CLIENT_ORDER_ID_LEN,
        };

        // Validate the client order ID against our spec impl.
        let client_order_id = "0x0197a98c91312671ca83f15ccbd5186f";
        assert_eq!(
            client_order_id.len(),
            CLIENT_ORDER_ID_LEN,
            "client order ID length must be 34"
        );
        assert!(
            validate_client_order_id(client_order_id),
            "client order ID must pass spec validation"
        );
        let prefix = parse_client_order_id_prefix(Some(client_order_id));
        assert_eq!((prefix.region, prefix.env), (0, 1), "expected HK-Prod");

        // Rebuild the action exactly as in the sample.
        let order = OrderRequest {
            client_order_id: Some(client_order_id.to_string()),
            ..make_limit_order("100170100", "buy", "0.41", "1.0", "gtc")
        };
        let action = action_place_order(vec![order]);
        let nonce = 1777392508038u64;
        let expires_after = Some(1777396108038u64);

        let conn_id =
            connection_id(&action, nonce, expires_after, None).unwrap_or_else(|_| unreachable!());
        let struct_hash = agent_struct_hash("Mainnet", &conn_id);
        let digest = eip712_signing_hash(ChainType::Mainnet.chain_id(), struct_hash);
        let digest_hex = format!("0x{}", hex_encode(&digest));

        let r = "563ad61b44c899649bc2f9381a80328655c7041355f25028a829315222a92813";
        let s = "73f29cd00db2c8d2658d50d87a1aad63de37a20dfb3004a83a5044d374fdc9ad";
        let v: u8 = 1;
        let sig = format!("0x{r}{s}{v:02x}");

        // Pin the EIP-712 signing hash for this exact request - sharper
        // failure signal if msgpack / struct hash / domain separator drifts.
        assert_eq!(
            digest_hex, "0xfd27f6d086ca25dcaacab0054f7c81560dcbf6fce174a4bf0dc9b0fe82dcb683",
            "signing hash drift",
        );

        let recovered = ecrecover(&digest_hex, &sig).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            recovered.to_lowercase(),
            "0x42b7281d1d9577213c442bd2a1af751f47b565ea",
            "recovered signer drift (digest={digest_hex})",
        );
    }

    // -- All action types produce unique signatures ------------------

    #[test]
    fn all_action_types_produce_distinct_signatures() {
        let key = test_key();
        let nonce = 1000u64;

        let actions = vec![
            action_prediction_split("1", "10"),
            action_prediction_merge("1", "10"),
            action_prediction_redeem("1"),
            action_place_order(vec![make_limit_order("0", "buy", "0.5", "10", "gtc")]),
            action_cancel(vec![make_cancel_oid("0", "1")]),
            action_cancel_all(vec![], "prediction"),
        ];

        let sigs: Vec<String> = actions
            .iter()
            .map(|a| sign_action(a, nonce, None, None, &key).expect("sign"))
            .collect();

        // Every signature should be unique
        for i in 0..sigs.len() {
            for j in (i + 1)..sigs.len() {
                assert_ne!(
                    sigs[i], sigs[j],
                    "actions {i} and {j} produced the same signature"
                );
            }
        }
    }

    // -- signer::* API round-trip ------------------------------------

    #[test]
    fn signer_module_round_trip() {
        use super::signer::{hash_action, sign_action as sign_action_typed, verify_action};

        let key = test_key();
        let action = action_prediction_split("1", "10");
        let nonce = 2000u64;

        // hash_action must match the existing pipeline's digest.
        let digest = hash_action(&action, nonce, None, None, ChainType::Mainnet)
            .unwrap_or_else(|_| unreachable!());
        let conn_id = connection_id(&action, nonce, None, None).unwrap_or_else(|_| unreachable!());
        let struct_hash = agent_struct_hash("Mainnet", &conn_id);
        let expected_digest = eip712_signing_hash(ChainType::Mainnet.chain_id(), struct_hash);
        assert_eq!(
            digest, expected_digest,
            "hash_action drifted from primitives"
        );

        // Sign through the trait, verify recovers the right address.
        let tx_sig = sign_action_typed(&action, &key, nonce, None, None, ChainType::Mainnet)
            .unwrap_or_else(|_| unreachable!());
        let recovered = verify_action(&tx_sig, &action, nonce, None, None, ChainType::Mainnet)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            recovered.to_lowercase(),
            signer_address(&key).to_lowercase()
        );
    }
}
