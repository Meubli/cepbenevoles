//! Minimal Google Sheets API client.
//!
//! Only proves the HTTP + parsing path end to end against a mock; the real
//! sync feature (service-account OAuth, mapping, dedup) is built later.

use serde::Deserialize;

pub struct SheetsClient {
    http: reqwest::Client,
    /// Overridable so tests point this at a wiremock server instead of
    /// `https://sheets.googleapis.com`.
    base_url: String,
    access_token: String,
}

#[derive(Deserialize)]
struct ValuesResponse {
    #[serde(default)]
    values: Vec<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
pub enum SheetsError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
}

impl SheetsClient {
    pub fn new(base_url: impl Into<String>, access_token: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
            access_token: access_token.into(),
        }
    }

    /// Fetches a value range, e.g. `"Réponses!A2:Z"`, and returns rows of
    /// cell strings as the Sheets API reports them.
    pub async fn fetch_values(
        &self,
        spreadsheet_id: &str,
        range: &str,
    ) -> Result<Vec<Vec<String>>, SheetsError> {
        let url = format!(
            "{}/v4/spreadsheets/{}/values/{}",
            self.base_url, spreadsheet_id, range
        );
        let resp = self
            .http
            .get(url)
            .bearer_auth(&self.access_token)
            .send()
            .await?
            .error_for_status()?
            .json::<ValuesResponse>()
            .await?;
        Ok(resp.values)
    }
}
