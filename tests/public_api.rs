//! Public-API reachability check.
//!
//! This file is compiled as a *separate crate*, so it can only name items that
//! `okx_outcomes_sdk` actually exports. It mirrors the documented caller flows
//! (client builder, signing, WebSocket) — if a type used in a public signature
//! or a README example is not reachable, this test fails to compile. It builds
//! values only; nothing here performs network I/O.

use okx_outcomes_sdk::{
    ApiCredentials, OutcomesSdkClient, OutcomesSdkClientBuilder, SdkError, TradingMode,
};

fn sample_credentials() -> ApiCredentials {
    ApiCredentials {
        api_key: "ak".into(),
        secret_key: "sk".into(),
        passphrase: "pp".into(),
    }
}

#[test]
fn client_builder_surface_is_reachable() {
    // Full builder path with every option — exercises TradingMode + builder type.
    let _builder: OutcomesSdkClientBuilder = OutcomesSdkClient::builder();
    let _client = OutcomesSdkClient::builder()
        .credentials(sample_credentials())
        .base_url("https://www.okx.com")
        .mode(TradingMode::Points)
        .accept_language("en-US")
        .timeout_secs(20)
        .debug(true)
        .build();

    // A mode-only builder path.
    let _points = OutcomesSdkClient::builder()
        .credentials(sample_credentials())
        .mode(TradingMode::Points)
        .build();

    // Constructor shortcuts.
    let _ = OutcomesSdkClient::with_credentials(sample_credentials());
    let _ =
        OutcomesSdkClient::with_credentials_and_url(sample_credentials(), "https://www.okx.com");
}

#[test]
fn error_variants_are_matchable_by_callers() {
    let err = SdkError::Api {
        code: 50105,
        message: "x".into(),
    };
    // #[non_exhaustive] forces a wildcard arm — confirm callers can still match.
    match err {
        SdkError::Api { code, .. } => assert_eq!(code, 50105),
        SdkError::UnexpectedStatus { .. } => {}
        _ => {}
    }
}

#[cfg(feature = "signing")]
#[test]
fn signing_surface_is_reachable() {
    use okx_outcomes_sdk::models::order::{OrderItem, PlaceOrderAction, PlaceOrderRequest};
    use okx_outcomes_sdk::signing::{
        action_cancel, action_place_order, generate_client_order_id_default, now_millis,
        parse_private_key, sign_to_wrapper, CancelRequest, CancelTarget, ChainType, LimitOrderType,
        LimitTif, OrderRequest, OrderType, SigningOrderSide, SizeType,
    };

    let key =
        parse_private_key("0x0101010101010101010101010101010101010101010101010101010101010101")
            .expect("valid test key");

    let order = OrderRequest {
        asset_id: "1".into(),
        side: SigningOrderSide::Buy,
        market_type: "prediction".into(),
        client_order_id: generate_client_order_id_default().expect("cloid"),
        price: "0.65".into(),
        reduce_only: false,
        size: "100".into(),
        size_type: SizeType::Base,
        order_type: OrderType::Limit(LimitOrderType { tif: LimitTif::Gtc }),
    };

    let order_item: OrderItem = (&order).into();
    let action = action_place_order(vec![order]);
    let nonce = now_millis();
    let signature = sign_to_wrapper(&action, nonce, None, ChainType::Mainnet, &key).expect("sign");

    let _req = PlaceOrderRequest {
        action: PlaceOrderAction {
            action_type: "placeOrder".into(),
            grouping: "na".into(),
            orders: vec![order_item],
        },
        nonce: nonce as i64,
        signature,
    };

    // Cancel flow types are reachable.
    let _cancel_target = CancelTarget::ClientOrderId("0x00".into());
    let _: fn(Vec<CancelRequest>) -> _ = action_cancel;
}

#[cfg(feature = "websocket")]
#[test]
fn websocket_surface_is_reachable() {
    use okx_outcomes_sdk::ws::{
        OutcomesWsClient, OutcomesWsClientBuilder, WsConnectionStateCallback, WsDataCallback,
    };
    use std::sync::Arc;

    let _builder: OutcomesWsClientBuilder = OutcomesWsClient::builder();
    let _ws = OutcomesWsClient::builder()
        .host(okx_outcomes_sdk::ws::endpoints::EU_WS_HOST)
        .debug(true)
        .build();
    // Bearer-token WS auth (sent as the Authorization header on the handshake).
    let _ws_bearer = OutcomesWsClient::builder().bearer_token("token").build();
    let _ = OutcomesWsClient::new();
    let _ = OutcomesWsClient::with_host(okx_outcomes_sdk::ws::endpoints::US_WS_HOST);

    // Callback type aliases are nameable by callers.
    let _on_data: WsDataCallback = Arc::new(|_msg| {});
    let _on_state: WsConnectionStateCallback = Arc::new(|_chan: &str, _ok: bool| {});
}
