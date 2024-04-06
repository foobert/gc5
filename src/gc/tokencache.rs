use log::{error, info};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use sqlx::Row;

use super::cache::Error;

pub struct AuthProvider {
    db: sqlx::PgPool,
}

impl AuthProvider {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            db: pool,
        }
    }

    pub async fn init(&self) -> Result<(), Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS settings (
            id TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )"
        )
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn token(&self) -> Result<String, Error> {
        // TODO we should probably introspect the JWT and refresh if necessary
        match self.load_access_token().await {
            Ok(token) => Ok(token),
            Err(_) => self.refresh().await
        }
    }

    pub async fn refresh(&self) -> Result<String, Error> {
        let refresh_token = self.load_refresh_token().await?;
        let (new_access_token, new_refresh_token) = self.call_groundspeak(refresh_token).await?;
        self.store_refresh_token(&new_refresh_token).await?;
        self.store_access_token(&new_access_token).await?;
        info!("Access token: {}", new_access_token);
        Ok(new_access_token)
    }

    async fn load_refresh_token(&self) -> Result<String, Error> {
        let result =
            sqlx::query("SELECT value FROM settings where id = 'refresh_token'")
                .fetch_one(&self.db).await?;
        Ok(result.get(0))
    }

    async fn load_access_token(&self) -> Result<String, Error> {
        let result =
            sqlx::query("SELECT value FROM settings where id = 'access_token'")
                .fetch_one(&self.db).await?;
        Ok(result.get(0))
    }

    async fn call_groundspeak(&self, refresh_token: String) -> Result<(String, String), Error> {
        // Create a HeaderMap and add the necessary headers
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/x-www-form-urlencoded; charset=UTF-8"));
        headers.insert(USER_AGENT, HeaderValue::from_static("looking4cache_pro/00336 CFNetwork/1492.0.1 Darwin/23.3.0"));
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-us"));

        // Create the body data
        let params = [
            ("redirect_uri", "https://app.looking4cache.com/l4cpro/auth/groundspeak"),
            ("refresh_token", &refresh_token),
            ("grant_type", "refresh_token"),
        ];

        // Send the POST request
        let client = reqwest::Client::new();
        let res = client.post("https://oauth.geocaching.com/token")
            .basic_auth("3E820485-D22A-48AE-8B78-75CA62A13190", Some("58987034-EB51-45F3-BB8B-764E263DD3BC"))
            .headers(headers)
            .form(&params)
            .send()
            .await?;

        info!("Token response: {:?}", res);

        // Check the status of the response
        if res.status().is_success() {
            let json: serde_json::Value = res.json().await?;
            let new_access_token = json["access_token"].as_str().unwrap().to_string();
            let new_refresh_token = json["refresh_token"].as_str().unwrap().to_string();

            info!("New access token: {}, new refresh token: {}", new_access_token, new_refresh_token);
            Ok((new_access_token, new_refresh_token))
        } else {
            error!("Unable to refresh token: {:?}", res);
            Err(Error::Geocaching)
        }
    }

    async fn store_access_token(&self, access_token: &str) -> Result<(), Error> {
        sqlx::query("INSERT INTO settings (id, value) VALUES ('access_token', $1) ON CONFLICT (id) DO UPDATE SET value = $1")
            .bind(&access_token)
            .execute(&self.db).await?;
        Ok(())
    }

    async fn store_refresh_token(&self, refresh_token: &str) -> Result<(), Error> {
        sqlx::query("INSERT INTO settings (id, value) VALUES ('refresh_token', $1) ON CONFLICT (id) DO UPDATE SET value = $1")
            .bind(&refresh_token)
            .execute(&self.db).await?;
        Ok(())
    }
}