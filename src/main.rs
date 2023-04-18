use sqlx::postgres::PgPoolOptions;
use thiserror::Error;
use log::{debug, info};
use serde::Deserialize;
use chrono::prelude::*;
use sqlx::Row;

#[derive(Error, Debug)]
pub enum DataStoreError {
    #[error("the data for key `{0}` is not available")]
    Redaction(String),
    #[error("invalid header (expected {expected:?}, found {found:?})")]
    InvalidHeader {
        expected: String,
        found: String,
    },
    #[error("db error")]
    Database(#[from] sqlx::Error),
    #[error("request error")]
    HttpRequest(#[from] reqwest::Error),
    #[error("json")]
    Json(#[from] serde_json::Error),
    #[error("unknown data store error")]
    Unknown,
}

#[tokio::main]
async fn main() -> Result<(), DataStoreError> {
    env_logger::init();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://localhost/gc").await?;

    let t = Tile::from_coordinates(51.34469577842422, 12.374765732990399, 12);
    info!("home: {:#?}", t);

    let tile_row = sqlx::query("SELECT gccodes, ts FROM tiles where id = $1")
        .bind(t.quadkey() as i32)
        .fetch_optional(&pool).await?;
    match tile_row {
        Some(row) => {
            let gccodes: Vec<String> = row.get(0);
            let ts: DateTime<Utc> = row.get(1);
            info!("already have a tile with {} gccodes from {}", gccodes.len(), ts);
        }
        None => {
            let gs = Groundspeak::new();
            let codes = gs.discover(&t).await?;

            sqlx::query("INSERT INTO tiles (id, gccodes, ts) VALUES ($1, $2, $3) ON CONFLICT (id) DO UPDATE SET gccodes = $2, ts = $3")
                .bind(t.quadkey() as i32)
                .bind(codes)
                .bind(Utc::now())
                .execute(&pool).await?;
        }
    }

    Ok(())
}

type GcCodes = Vec<String>;

struct Coordinate {
    lat: f64,
    lon: f64,
}

struct Timestamped<T> {
    ts: DateTime<Utc>,
    data: T,
}

#[derive(Debug)]
struct Tile {
    x: u32,
    y: u32,
    z: u32,
}

impl Tile {
    pub fn from_coordinates(lat: f64, lon: f64, z: u32) -> Self {
        const PI : f64 = std::f64::consts::PI;
        let lat_rad = lat * PI / 180.0;
        let n = 2_i32.pow(z) as f64;
        let x = ((lon + 180.0) / 360.0 * n) as u32;
        let y = ((1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI) / 2.0 * n) as u32;
        return Self { x: x, y: y, z: z };
    }

    pub fn quadkey(&self) -> u32 {
        let mut result = 0;
        for i in 0..self.z {
            result |= (self.x & 1 << i) << i | (self.y & 1 << i) << (i + 1);
        }
        return result;
    }
}

struct Groundspeak {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Deserialize, Debug)]
struct GroundspeakTileResponse {
    data: std::collections::HashMap<String, Vec<ResponseObject>>,
}

#[derive(Deserialize, Debug)]
struct ResponseObject {
    i: String,
}

struct Geocache {
}

impl Groundspeak {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://tiles01.geocaching.com".to_string(),
        }
    }

    pub async fn discover(&self, tile: &Tile) -> Result<GcCodes, DataStoreError> {
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

    pub async fn lookup(&self, code: &str) -> Result<Geocache, DataStoreError> {
        Ok(Geocache { })
    }
}
