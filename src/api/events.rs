//! Events and markets API methods.

use crate::{
    client::OutcomesSdkClient,
    endpoints,
    error::SdkError,
    models::event::{EventObject, EventsResponse, MarketObject, MarketsResponse},
};

impl OutcomesSdkClient {
    /// `GET /api/v5/predictions/events` — Retrieve a paginated list of outcome market events.
    ///
    /// # Parameters
    ///
    /// - `status` — Filter by event status: `"active"` (default) or `"resolved"`.
    /// - `category` — Filter by category: `"SPORTS"` or `"CURRENT_AFFAIRS"`.
    /// - `tag` — Filter sports events by sport tag ID.
    /// - `league_id` — Filter sports events by league ID.
    /// - `sort` — Sort order: `"volume"` / `"volume_24h"` (default) / `"ending_soon"` / `"newest"`.
    /// - `cursor` — Pagination cursor from the previous response; omit for the first page.
    /// - `page_size` — Items per page (max 50, default 10).
    #[allow(clippy::too_many_arguments)]
    pub async fn get_events(
        &self,
        status: Option<&str>,
        category: Option<&str>,
        tag: Option<&str>,
        league_id: Option<&str>,
        sort: Option<&str>,
        cursor: Option<&str>,
        page_size: Option<i32>,
    ) -> Result<EventsResponse, SdkError> {
        let page_size_str = page_size.map(|v| v.to_string());
        let mut params: Vec<(&str, &str)> = Vec::new();
        if let Some(v) = status {
            params.push(("status", v));
        }
        if let Some(v) = category {
            params.push(("category", v));
        }
        if let Some(v) = tag {
            params.push(("tag", v));
        }
        if let Some(v) = league_id {
            params.push(("leagueId", v));
        }
        if let Some(v) = sort {
            params.push(("sort", v));
        }
        if let Some(v) = cursor {
            params.push(("cursor", v));
        }
        if let Some(ref v) = page_size_str {
            params.push(("pageSize", v));
        }
        self.http_get_public(endpoints::EVENTS, &params).await
    }

    /// `GET /api/v5/predictions/events/search` — Search events and markets by keyword.
    ///
    /// # Parameters
    ///
    /// - `keyword` — Search keyword.
    /// - `cursor` — Pagination cursor from the previous response.
    /// - `page_size` — Items per page (default 10).
    pub async fn search(
        &self,
        keyword: &str,
        cursor: Option<&str>,
        page_size: Option<i32>,
    ) -> Result<EventsResponse, SdkError> {
        let page_size_str = page_size.map(|v| v.to_string());
        let mut params: Vec<(&str, &str)> = vec![("keyword", keyword)];
        if let Some(v) = cursor {
            params.push(("cursor", v));
        }
        if let Some(ref v) = page_size_str {
            params.push(("pageSize", v));
        }
        self.http_get_public(endpoints::SEARCH, &params).await
    }

    /// `GET /api/v5/predictions/events/{eventId}` — Retrieve a single event with its full market list.
    pub async fn get_event(&self, event_id: &str) -> Result<EventObject, SdkError> {
        let path = format!("{}/{}", endpoints::EVENTS, event_id);
        self.http_get_public(&path, &[]).await
    }

    /// `GET /api/v5/predictions/events/{eventId}/markets` — Retrieve all markets for an event (no pagination).
    pub async fn get_event_markets(&self, event_id: &str) -> Result<MarketsResponse, SdkError> {
        let path = format!("{}/{}/markets", endpoints::EVENTS, event_id);
        self.http_get_public(&path, &[]).await
    }

    /// `GET /api/v5/predictions/markets/{marketId}` — Retrieve a single market by its ID.
    pub async fn get_market(&self, market_id: &str) -> Result<MarketObject, SdkError> {
        let path = format!("{}/{}", endpoints::MARKETS, market_id);
        self.http_get_public(&path, &[]).await
    }
}
