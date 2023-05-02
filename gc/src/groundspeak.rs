use log::{debug, info};
use thiserror::Error;
use serde::{Serialize, Deserialize};
use geo::Tile;
use std::collections::HashMap;

pub const BATCH_SIZE: usize = 50;

pub struct Groundspeak {
    client: reqwest::Client,
    base_url: String,
}

pub type GcCodes = Vec<String>;

#[derive(Deserialize, Debug)]
pub struct Geocache {
}

#[derive(Deserialize, Debug)]
struct GroundspeakTileResponse {
    data: HashMap<String, Vec<ResponseObject>>,
}

#[derive(Deserialize, Debug)]
struct ResponseObject {
    i: String,
}

#[derive(Deserialize, Debug)]
struct GroundspeakFetchResponse {
    #[serde(rename = "Status")]
    status: StatusObject,
    #[serde(rename = "Geocaches")]
    geocaches: Vec<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct StatusObject {
    #[serde(rename = "StatusCode")]
    status_code: u32,
}

impl GroundspeakFetchResponse {
    fn get_raw(&self) -> Vec<serde_json::Value> {
        self.geocaches.clone()
    }
}

#[derive(Serialize, Debug)]
struct RequestBody {
    #[serde(rename = "AccessToken")]
    access_token: String,
    #[serde(rename = "CacheCode")]
    cache_code: CacheCode,
    #[serde(rename = "GeocacheLogCount")]
    geocache_log_count: u32,
    #[serde(rename = "IsLite")]
    is_lite: bool,
    #[serde(rename = "MaxPerPage")]
    max_per_page: usize,
    #[serde(rename = "TrackableLogCount")]
    trackable_log_count: u32,
}

#[derive(Serialize, Debug)]
struct CacheCode {
    #[serde(rename = "CacheCodes")]
    cache_codes: Vec<String>,
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
        let codes_set: std::collections::BTreeSet<String> = info.data.values().flat_map(|v| v.iter().map(|o| String::from(&o.i) )).collect();
        let codes = Vec::from_iter(codes_set.into_iter());


        debug!("codes: {:#?}", codes);
        info!("Found {} codes", codes.len());

        Ok(codes)
    }

    pub async fn fetch(&self, codes: Vec<String>) -> Result<Vec<serde_json::Value>, Error> {
        let mut result = Vec::new();
        for chunk in codes.chunks(BATCH_SIZE) {
            // TODO whats the difference between Vec and slice aka &[String] ?
            let mut chunk_of_caches = self.fetch_chunk(chunk.to_vec()).await?;
            result.append(&mut chunk_of_caches);
        }
        Ok(result)
    }

    async fn fetch_chunk(&self, chunk: Vec<String>) -> Result<Vec<serde_json::Value>, Error> {
        // let test_chunk = vec!["GC95978","GC3MP1K"].iter().map(|s| s.to_string()).collect();
        info!("fetch chunk {}", chunk.len());
        let request = RequestBody {
            access_token: self.access_token(),
            geocache_log_count: 5,
            is_lite: false,
            max_per_page: BATCH_SIZE,
            trackable_log_count: 0,
            cache_code: CacheCode {
                cache_codes: chunk
            }
        };
        let url = "https://api.groundspeak.com/LiveV6/Geocaching.svc/internal/SearchForGeocaches?format=json";
        let json = serde_json::to_string(&request)?;
        info!("fetching...");
        let response = self.client.post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await?;
        info!("fetch status {}", response.status().as_str());
        let json = response.json::<GroundspeakFetchResponse>().await?;
        info!("fetch status {}", json.status.status_code);
        let raw = json.get_raw();
        //info!("raw: {:#?}", raw);
        Ok(raw)
    }

    pub fn parse(&self, value: serde_json::Value) -> Result<Geocache, Error> {
        match serde_json::from_value(value) {
            Ok(value) => Ok(value),
            Err(_) => 
                // wtf
                Err(Error::Unknown)
        }
    }

    pub fn access_token(&self) -> String {
        "2c144c16-b33d-48bc-845b-cbd969681c4c".to_string()
    }
}
