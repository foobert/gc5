use geo::{GcCodes, Tile, Geocache, Coordinate};
use crate::groundspeak::Groundspeak;

use log::info;
use chrono::prelude::*;
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;
use futures::{stream,StreamExt};

pub mod groundspeak;
pub mod st;

pub struct Cache {
    db: sqlx::PgPool,
    groundspeak: Groundspeak,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("db error")]
    Database(#[from] sqlx::Error),
    #[error("groundspeak")]
    GroundSpeak(#[from] groundspeak::Error),
    #[error("json")]
    Json(#[from] serde_json::Error),
    #[error("unknown data store error")]
    Unknown,
}

impl Cache {
    pub fn new(pool: sqlx::PgPool) -> Self {
        let gs = Groundspeak::new();
        return Self { db: pool, groundspeak: gs };
    }

    pub async fn new_lite() -> Result<Self, Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect("postgres://localhost/gc").await?;
        let gs = Groundspeak::new();
        Ok(Self { db: pool, groundspeak: gs })
    }

    pub async fn find_tile(&self, tile: &Tile) -> Result<Timestamped<Vec<Geocache>>, Error> {
        let result : Vec<Geocache> = vec![];
        let codes = self.discover(tile).await?;
        self.get(codes.data).await?;
        Ok(Timestamped::now(result))
    }

    pub async fn find_near(&self, center: &Coordinate, radius: usize) -> Result<Vec<Geocache>, Error> {
        info!("find_near {}, {}", center, radius);
        // needed? based on the assumption that we can do an efficient api call with search radius
        // and that might be a nice use case, but haven't used it that much yet?
        Err(Error::Unknown)
    }

    pub async fn find(&self, top_left: &Coordinate, bottom_right: &Coordinate, sloppy: bool) -> Result<Vec<Geocache>, Error> {
        info!("find {} {} {}", top_left, bottom_right, sloppy);
        // translate into tiles, then discover tiles and fetch them
        // optionally: filter afterwards to make sure all gcs are within bounds
        Err(Error::Unknown)
    }

    pub async fn get(&self, codes: Vec<String>) -> Result<Vec<Geocache>, Error> {
        let mut cache_hit: Vec<Geocache> = vec![];
        let mut cache_miss: Vec<String> = vec![];
        let cutoff = Utc::now() - chrono::Duration::hours(48);
        let codes_len = codes.len();
        for code in codes {
            match self.load_geocache(&code, &cutoff).await {
                Some(geocache) => cache_hit.push(geocache),
                None => cache_miss.push(code),
            }
        }
        info!("Fetching {} geocaches, {} from DB and {} from Groundspeak",
              codes_len,
              cache_hit.len(),
              cache_miss.len());

        let fetched: Vec<serde_json::Value> = stream::iter(cache_miss)
            .chunks(groundspeak::BATCH_SIZE)
            .then(|x| self.groundspeak.fetch(x))
            .map(|x| x.unwrap())
            .flat_map(stream::iter)
            .then(|x| self.save_geocache(x))
            .collect()
            .await;

        fetched.into_iter().map(|json| serde_json::from_value::<Geocache>(json)).filter_map(|x| x.ok()).for_each(|gc| cache_hit.push(gc));
        return Ok(cache_hit);
    }

    async fn save_geocache(&self, geocache: serde_json::Value) -> serde_json::Value {
        // TODO this ignores the error!
        let id = geocache["Code"].as_str().unwrap();
        let insert_result = sqlx::query("INSERT INTO geocaches (id, raw, ts) VALUES ($1, $2, $3) ON CONFLICT (id) DO UPDATE SET raw = $2::JSON, ts = $3")
            .bind(&id)
            .bind(&geocache)
            .bind(Utc::now())
            .execute(&self.db).await;
        match &insert_result {
            Ok(res) => println!("insert: {} {}", id, res.rows_affected()),
            Err(e) => println!("insert err: {}", e),
        }
        geocache
    }

    async fn load_geocache(&self, code: &String, cutoff: &DateTime<Utc>) -> Option<Geocache> {
        info!("load_geocache {}", code);
        let json_result: Result<Option<sqlx::postgres::PgRow>, _> = sqlx::query("SELECT raw::VARCHAR FROM geocaches where id = $1 and ts >= $2")
            .bind(code)
            .bind(cutoff)
            .fetch_optional(&self.db).await;
        match json_result {
            Ok(Some(row)) => {
                let gc: Result<serde_json::Value, serde_json::Error> = serde_json::from_str(row.get(0));
                match gc {
                    Ok(v) => {
                        Some(Geocache {
                            code: String::from(v["Code"].as_str().unwrap()),
                            name: String::from(""),
                            terrain: 0.0,
                            difficulty: 0.0,
                            coord: Coordinate { lat: 0.0, lon: 0.0 },
                            short_description: String::from(""),
                            long_description: String::from(""),
                            encoded_hints: String::from(""),
                            size: geo::ContainerSize::Large,
                            cache_type: geo::CacheType::Earth,
                        })
                    }
                    Err(e) => {
                        info!("json failed {}", e);
                        return None;
                    }
                }
                // return serde_json::from_str(row.get(0)).ok();
            },
            Ok(None) => None,
            Err(e) => {
                info!("Failed to load geocache {}", e);
                return None;
            }
        }
        // let json_option: Option<sqlx::postgres::PgRow> = json_result.ok().flatten();
        // return json_option.map(|row| serde_json::from_str(row.get(0)).ok()).flatten()
    }

    pub async fn discover(&self, tile: &Tile) -> Result<Timestamped<GcCodes>, Error> {
        // TODO think about switching from single row per tile to single row per gc code
        // update could be done in a transaction and we could natively work with sqlite
        // which would make operations easier
        info!("Discover {:#?}", tile.quadkey());
        let tile_row = sqlx::query("SELECT gccodes, ts FROM tiles where id = $1")
            .bind(tile.quadkey() as i32)
            .fetch_optional(&self.db).await?;
        match tile_row {
            Some(row) => {
                let gccodes: Vec<String> = row.get(0);
                let ts: DateTime<Utc> = row.get(1);
                info!("already have a tile with {} gccodes from {}", gccodes.len(), ts);
                return Ok(Timestamped { ts: ts, data: gccodes });
            }
            None => {
                let codes = self.groundspeak.discover(&tile).await?;

                sqlx::query("INSERT INTO tiles (id, gccodes, ts) VALUES ($1, $2, $3) ON CONFLICT (id) DO UPDATE SET gccodes = $2, ts = $3")
                    .bind(tile.quadkey() as i32)
                    .bind(&codes)
                    .bind(Utc::now())
                    .execute(&self.db).await?;
                return Ok(Timestamped::now(codes));
            }
        }
    }
}

pub struct Timestamped<T> {
    pub ts: DateTime<Utc>,
    pub data: T,
}

impl<T> Timestamped<T> {
    fn now(data: T) -> Self {
        Self { ts: Utc::now(), data: data }
    }
}
