//! Smoke-test the unauthenticated client against the live OKX Outcomes API.
//!
//! Run with:
//!     cargo run --example unauthenticated_reads
//!
//! It performs two public reads (events list + a ticker) with no credentials,
//! then shows that a private read (balance) is rejected client-side with
//! `NotAuthenticated`.

use okx_outcomes_sdk::{OutcomesSdkClient, SdkError};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OutcomesSdkClient::unauthenticated();

    // --- Public read: events list -------------------------------------------
    let page = client
        .get_events(
            Some("active"), // status
            None,           // category
            None,           // tag
            None,           // league_id
            None,           // sort
            None,           // cursor
            Some(5),        // page_size
        )
        .await?;

    println!("✓ get_events returned {} event(s):", page.events.len());
    for ev in page.events.iter().take(5) {
        println!(
            "  - [{}] {} ({} markets)",
            ev.event_id, ev.event_title, ev.total_markets_count
        );
    }

    // --- Public read: a ticker for the first event's first market -----------
    if let Some(asset_id) = page
        .events
        .iter()
        .flat_map(|e| e.markets.iter())
        .find_map(|m| m.yes_outcome.asset_id.clone())
    {
        match client.get_ticker(&asset_id).await {
            Ok(t) => println!("✓ get_ticker({asset_id}) → last={:?}", t.last),
            Err(e) => println!("• get_ticker({asset_id}) error: {e}"),
        }
    }

    // --- Private read must be rejected without credentials ------------------
    match client.get_balance().await {
        Err(SdkError::NotAuthenticated { .. }) => {
            println!("✓ get_balance correctly rejected with NotAuthenticated");
        }
        Ok(_) => println!("✗ unexpected: get_balance succeeded on an unauthenticated client"),
        Err(e) => println!("✗ unexpected error from get_balance: {e}"),
    }

    Ok(())
}
