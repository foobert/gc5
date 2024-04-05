use std::collections::HashMap;
use std::time::Duration;

use log::{debug, info};
use rand::Rng;
use serde::Deserialize;
use thiserror::Error;
use tokio::time::sleep;

use gcgeo::{CacheType, ContainerSize, Tile};

pub const BATCH_SIZE: usize = 50;

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
    const FETCH_URL: &'static str = "https://api.groundspeak.com/v1.0/geocaches";

    const USER_AGENT: &'static str = "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:109.0) Gecko/20100101 Firefox/112.0";

    const USER_AGENT_FETCH: &'static str = "L4C Pro/4.3.2 (iPhone; iOS 17.3.1; Scale/3.00)";

    //const FETCH_FIELDS: &'static str = "referenceCode,ianaTimezoneId,name,postedCoordinates,geocacheType,geocacheSize,difficulty,terrain,userData,favoritePoints,placedDate,eventEndDate,ownerAlias,owner,isPremiumOnly,userData,lastVisitedDate,status,hasSolutionChecker";
    const FETCH_FIELDS: &'static str = "referenceCode,name,postedCoordinates,geocacheType,geocacheSize,difficulty,terrain,favoritePoints,placedDate,isPremiumOnly,lastVisitedDate,status,shortDescription,longDescription,hints,additionalWaypoints,geocacheLogs";

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

    pub async fn fetch(&self, token: &str, codes: Vec<&String>) -> Result<Vec<serde_json::Value>, Error> {
        if codes.len() > BATCH_SIZE {
            return Err(Error::Unknown);
        }
        info!("fetch chunk {}", codes.len());
        let codes_str: Vec<&str> = codes.iter().map(|x| x.as_str()).collect();
        let comma_separated_codes = codes_str.join(",");
        let response = self
            .client
            .get(Groundspeak::FETCH_URL)
            .header(reqwest::header::ACCEPT, "*/*")
            .header(reqwest::header::ACCEPT_LANGUAGE, "en-US;q=1")
            .header(reqwest::header::USER_AGENT, Groundspeak::USER_AGENT_FETCH)
            .bearer_auth(token)
            .query(&[("referenceCodes", comma_separated_codes), ("lite", "false".to_string()), ("fields", Groundspeak::FETCH_FIELDS.to_string()), ("expand", "geocacheLogs:5".to_string())])
            .send()
            .await?;
        info!("fetch status {}", response.status().as_str());
        let json: serde_json::Value = serde_json::from_slice(&response.bytes().await?)?;
        info!("fetch json {:#?}", json);

        sleep(Duration::from_secs(1)).await;

        let geocaches = json.as_array().ok_or(Error::JsonRaw)?.clone();
        debug!("fetch geocaches {}", geocaches.len());

        if geocaches.len() != codes.len() {
            return Err(Error::JsonRaw);
        }

        Ok(geocaches)
    }
}

pub fn parse(v: &serde_json::Value) -> Result<gcgeo::Geocache, Error> {
    // this is pretty ugly, but more advanced serde scared me more
    let code = String::from(v["referenceCode"].as_str().ok_or(Error::JsonRaw)?);
    let is_premium = v["isPremiumOnly"].as_bool().unwrap_or(false);

    if is_premium {
        return Ok(gcgeo::Geocache::premium(code));
    }

    let name = String::from(v["name"].as_str().ok_or(Error::JsonRaw)?);
    let terrain = v["terrain"].as_f64().ok_or(Error::JsonRaw)? as f32;
    let difficulty = v["difficulty"].as_f64().ok_or(Error::JsonRaw)? as f32;
    let lat = v["postedCoordinates"]["latitude"].as_f64().ok_or(Error::JsonRaw)?;
    let lon = v["postedCoordinates"]["longitude"].as_f64().ok_or(Error::JsonRaw)?;
    let short_description = String::from(v["shortDescription"].as_str().ok_or(Error::JsonRaw)?);
    let long_description = String::from(v["longDescription"].as_str().ok_or(Error::JsonRaw)?);
    let encoded_hints = String::from(v["hints"].as_str().ok_or(Error::JsonRaw)?);
    let size = ContainerSize::from(
        v["geocacheSize"]["id"]
            .as_u64()
            .ok_or(Error::JsonRaw)?,
    );
    let cache_type = CacheType::from(
        v["geocacheType"]["id"]
            .as_u64()
            .ok_or(Error::JsonRaw)?,
    );
    let available = v["status"].as_str().ok_or(Error::JsonRaw)? == "Active";
    // TODO archived?
    let archived = false; //v["Archived"].as_bool().ok_or(Error::JsonRaw)?;
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
        available,
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
