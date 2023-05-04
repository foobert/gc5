use geo::{CacheType, ContainerSize, Tile};
use log::{debug, info};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use thiserror::Error;

pub const BATCH_SIZE: usize = 50;

const FETCH_URL: &'static str =
    "https://api.groundspeak.com/LiveV6/Geocaching.svc/internal/SearchForGeocaches?format=json";

pub struct Groundspeak {
    client: reqwest::Client,
    base_url: String,
}

pub type GcCodes = Vec<String>;

#[derive(Deserialize, Debug)]
struct GroundspeakTileResponse {
    data: HashMap<String, Vec<ResponseObject>>,
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
    #[error("json_raw")]
    JsonRaw,
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

        let image_url = std::format!(
            "{}/map.png?x={}&y={}&z={}",
            self.base_url,
            tile.x,
            tile.y,
            tile.z
        );
        let info_url = std::format!(
            "{}/map.info?x={}&y={}&z={}",
            self.base_url,
            tile.x,
            tile.y,
            tile.z
        );

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
        let codes_set: std::collections::BTreeSet<String> = info
            .data
            .values()
            .flat_map(|v| v.iter().map(|o| String::from(&o.i)))
            .collect();
        let codes = Vec::from_iter(codes_set.into_iter());

        debug!("codes: {:#?}", codes);
        info!("Found {} codes", codes.len());

        Ok(codes)
    }

    pub async fn fetch(&self, codes: Vec<&String>) -> Result<Vec<serde_json::Value>, Error> {
        if codes.len() > BATCH_SIZE {
            return Err(Error::Unknown);
        }
        info!("fetch chunk {}", codes.len());
        let response = self
            .client
            .post(FETCH_URL)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "AccessToken": self.access_token(),
                    "GeocacheLogCount": 5,
                    "IsLite": false,
                    "MaxPerPage": BATCH_SIZE,
                    "TrackableLogCount": 0,
                    "CacheCode": {
                        "CacheCodes": codes,
                    }
                })
                .to_string(),
            )
            .send()
            .await?;
        debug!("fetch status {}", response.status().as_str());
        let json: serde_json::Value = serde_json::from_slice(&response.bytes().await?)?;
        match json["Geocaches"].as_array() {
            Some(geocaches) => Ok(geocaches.clone()),
            None => Err(Error::Unknown),
        }
    }

    fn access_token(&self) -> String {
        "2c144c16-b33d-48bc-845b-cbd969681c4c".to_string()
    }
}

pub fn parse(v: &serde_json::Value) -> Result<geo::Geocache, Error> {
    // this is pretty ugly, but more advanced serde scared me more
    let code = String::from(v["Code"].as_str().ok_or(Error::JsonRaw)?);
    let name = String::from(v["Name"].as_str().ok_or(Error::JsonRaw)?);
    let terrain = v["Terrain"].as_f64().ok_or(Error::JsonRaw)? as f32;
    let difficulty = v["Difficulty"].as_f64().ok_or(Error::JsonRaw)? as f32;
    let lat = v["Latitude"].as_f64().ok_or(Error::JsonRaw)?;
    let lon = v["Longitude"].as_f64().ok_or(Error::JsonRaw)?;
    let short_description = String::from(v["ShortDescription"].as_str().ok_or(Error::JsonRaw)?);
    let long_description = String::from(v["LongDescription"].as_str().ok_or(Error::JsonRaw)?);
    let encoded_hints = String::from(v["EncodedHints"].as_str().ok_or(Error::JsonRaw)?);
    let size = ContainerSize::from(
        v["ContainerType"]["ContainerTypeId"]
            .as_u64()
            .ok_or(Error::JsonRaw)?,
    );
    let cache_type = CacheType::from(
        v["CacheType"]["GeocacheTypeId"]
            .as_u64()
            .ok_or(Error::JsonRaw)?,
    );
    Ok(geo::Geocache {
        code,
        name,
        terrain,
        difficulty,
        coord: geo::Coordinate { lat, lon },
        short_description,
        long_description,
        encoded_hints,
        size,
        cache_type,
    })
}
