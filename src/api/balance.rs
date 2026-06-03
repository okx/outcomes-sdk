//! Balance API method.

use crate::{
    client::OutcomesSdkClient, endpoints, error::SdkError, models::balance::BalanceResponse,
};

impl OutcomesSdkClient {
    /// `GET /api/v5/predictions/balance` — Return the authenticated account's balance, one
    /// [`BalanceEntry`](crate::models::balance::BalanceEntry) per odds type
    /// (`spots` = real market, `points` = points market).
    pub async fn get_balance(&self) -> Result<BalanceResponse, SdkError> {
        self.http_get(endpoints::BALANCE, &[]).await
    }
}
