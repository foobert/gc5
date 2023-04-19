use log::{debug, info};
use thiserror::Error;
use serde::Deserialize;
use geo::Tile;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

pub struct Groundspeak {
    client: reqwest::Client,
    base_url: String,
}

pub type GcCodes = Vec<String>;

pub struct Geocache {
}

#[derive(Deserialize, Debug)]
struct GroundspeakTileResponse {
    data: std::collections::HashMap<String, Vec<ResponseObject>>,
}

#[derive(Deserialize, Debug)]
struct ResponseObject {
    i: String,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("request error")]
    HttpRequest(#[from] reqwest::Error),
    #[error("json")]
    Json(#[from] serde_json::Error),
    #[error("unknown error")]
    Unknown,
}

impl Groundspeak {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://tiles01.geocaching.com".to_string(),
        }
    }

    pub async fn discover(&self, tile: &Tile) -> Result<GcCodes, Error> {
        info!("Discovering {:#?}", tile);

        let image_url = std::format!("{}/map.png?x={}&y={}&z={}", self.base_url, tile.x, tile.y, tile.z);
        let info_url = std::format!("{}/map.info?x={}&y={}&z={}", self.base_url, tile.x, tile.y, tile.z);

        self.client.get(image_url)
            .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/100.0.4896.127 Safari/537.36")
            .header("Accept", "*/*")
            .send()
            .await?;

        let response = self.client.get(info_url)
            .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/100.0.4896.127 Safari/537.36")
            .header("Accept", "application/json")
            .send().await?;

        debug!("tile response {:#?}", response);
        info!("tile status {}", response.status().as_str());

        let info = response.json::<GroundspeakTileResponse>().await?;

        // TODO strings are copied, can we do it without copying?
        let codes: GcCodes = info.data.values().flat_map(|v| v.iter().map(|o| String::from(&o.i) )).collect();

        debug!("codes: {:#?}", codes);
        info!("Found {} codes", codes.len());

        Ok(codes)
    }

    pub async fn lookup(&self, code: &str) -> Result<Geocache, Error> {
        info!("lookup {}", code);
        Ok(Geocache { })
    }
}
