use gcgeo::{CacheType, ContainerSize, Tile};
use log::{debug, info};
use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;

pub const BATCH_SIZE: usize = 50;

const FETCH_URL: &'static str =
    "https://api.groundspeak.com/LiveV6/Geocaching.svc/internal/SearchForGeocaches?format=json";

pub struct Groundspeak {
    client: reqwest::Client,
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
    const USER_AGENT: &'static str = "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:109.0) Gecko/20100101 Firefox/112.0";

    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn discover(&self, tile: &Tile) -> Result<GcCodes, Error> {
        debug!("Discovering {}", tile);

        let base_url = format!(
            "https://tiles0{}.geocaching.com",
            rand::thread_rng().gen_range(1..5)
        );
        let image_url = std::format!(
            "{}/map.png?x={}&y={}&z={}",
            base_url,
            tile.x,
            tile.y,
            tile.z,
        );
        let info_url = std::format!(
            "{}/map.info?x={}&y={}&z={}",
            base_url,
            tile.x,
            tile.y,
            tile.z,
        );

        self.client
            .get(image_url)
            .header(reqwest::header::USER_AGENT, Self::USER_AGENT)
            .header(reqwest::header::ACCEPT, "*/*")
            .send()
            .await?;

        let response = self
            .client
            .get(info_url)
            .header(reqwest::header::USER_AGENT, Self::USER_AGENT)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        sleep(Duration::from_secs(1)).await;

        debug!("tile response {:#?}", response);
        if response.status() == 204 {
            info!("Discover {} -> 0", tile);
            return Ok(vec![]);
        }

        let info = response.json::<GroundspeakTileResponse>().await?;

        // TODO strings are copied, can we do it without copying?
        let codes_set: std::collections::BTreeSet<String> = info
            .data
            .values()
            .flat_map(|v| v.iter().map(|o| String::from(&o.i)))
            .collect();
        let codes = Vec::from_iter(codes_set.into_iter());

        info!("Discover {} -> {}", tile, codes.len());

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
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
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

        sleep(Duration::from_secs(1)).await;

        let mut geocaches = json["Geocaches"].as_array().ok_or(Error::JsonRaw)?.clone();

        if geocaches.len() == 0 {
            // all premium
            return Ok(codes.iter().map(|code| Self::hacky_premium_geocache(code)).collect());
        }

        for (i, code) in codes.iter().enumerate() {
            if let Some(geocache) = geocaches.get(i) {
                if let Some(gc) = geocache["Code"].as_str() {
                    if code != &gc {
                        debug!("{} is premium", code);
                        geocaches.insert(i, Self::hacky_premium_geocache(code));
                    }
                }
            }
        }

        Ok(geocaches)
    }

    fn hacky_premium_geocache(code: &str) -> serde_json::Value {
        json!({
            "Code": code,
            "IsPremium": true,
        })
    }

    fn access_token(&self) -> String {
        "2c144c16-b33d-48bc-845b-cbd969681c4c".to_string()
    }
}

pub fn parse(v: &serde_json::Value) -> Result<gcgeo::Geocache, Error> {
    // this is pretty ugly, but more advanced serde scared me more
    let code = String::from(v["Code"].as_str().ok_or(Error::JsonRaw)?);
    let is_premium = v["IsPremium"].as_bool().unwrap_or(false);

    if is_premium {
        return Ok(gcgeo::Geocache::premium(code));
    }

    let name = String::from(v["Name"].as_str().ok_or(Error::JsonRaw)?);
    let terrain = v["Terrain"].as_f64().ok_or(Error::JsonRaw)? as f32;
    let difficulty = v["Difficulty"].as_f64().ok_or(Error::JsonRaw)? as f32;
    let lat = v["Latitude"].as_f64().ok_or(Error::JsonRaw)?;
    let lon = v["Longitude"].as_f64().ok_or(Error::JsonRaw)?;
    let short_description = String::from(v["ShortDescription"].as_str().unwrap_or_default());
    // let short_description = String::from(v["ShortDescription"].as_str().ok_or(Error::JsonRaw)?);
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
    let available = v["Available"].as_bool().ok_or(Error::JsonRaw)?;
    let archived = v["Archived"].as_bool().ok_or(Error::JsonRaw)?;
    Ok(gcgeo::Geocache {
        code,
        name,
        is_premium,
        terrain,
        difficulty,
        coord: gcgeo::Coordinate { lat, lon },
        short_description,
        long_description,
        encoded_hints,
        size,
        cache_type,
        archived,
        available
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_foo() {
        let uut = Groundspeak::new();
        let tile = gcgeo::Tile::from_coordinates(51.34469577842422, 12.374765732990399, 12);
        uut.discover(&tile).await.unwrap();
    }
}
