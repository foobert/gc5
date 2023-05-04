use crate::groundspeak::Groundspeak;
use geo::{Coordinate, GcCodes, Geocache, Tile};

use chrono::prelude::*;
use futures::{future::ready, stream, StreamExt};
use log::{debug, info, error};
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use thiserror::Error;

pub mod groundspeak;

pub struct Cache {
    db: sqlx::PgPool,
    groundspeak: Groundspeak,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("gc")]
    Geocaching,
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
        return Self {
            db: pool,
            groundspeak: gs,
        };
    }

    pub async fn new_lite() -> Result<Self, Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect("postgres://localhost/gc")
            .await?;
        let gs = Groundspeak::new();
        Ok(Self {
            db: pool,
            groundspeak: gs,
        })
    }

    pub async fn find_tile(&self, tile: &Tile) -> Result<Timestamped<Vec<Geocache>>, Error> {
        let result: Vec<Geocache> = vec![];
        let codes = self.discover(tile).await?;
        self.get(codes.data).await?;
        Ok(Timestamped::now(result))
    }

    pub async fn find(
        &self,
        top_left: &Coordinate,
        bottom_right: &Coordinate,
        sloppy: bool,
    ) -> Result<Vec<Geocache>, Error> {
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
        info!(
            "Fetching {} geocaches, {} from DB and {} from Groundspeak",
            codes_len,
            cache_hit.len(),
            cache_miss.len()
        );

        let mut fetched: Vec<Geocache> = stream::iter(&cache_miss)
            .chunks(groundspeak::BATCH_SIZE)
            .then(|x| self.groundspeak.fetch(x))
            .filter_map(|x| ready(x.ok()))
            .flat_map(stream::iter)
            .then(|x| self.save_geocache(x))
            .filter_map(|x| ready(x.ok()))
            .collect()
            .await;

        if fetched.len() < cache_miss.len() {
            return Err(Error::Geocaching);
        }

        cache_hit.append(&mut fetched);

        Ok(cache_hit)
    }

    async fn save_geocache(&self, geocache: serde_json::Value) -> Result<Geocache, Error> {
        let code = geocache["Code"].as_str().ok_or(Error::Geocaching)?;
        debug!("Save {}", code);
        sqlx::query("INSERT INTO geocaches (id, raw, ts) VALUES ($1, $2, $3) ON CONFLICT (id) DO UPDATE SET raw = $2::JSON, ts = $3")
            .bind(&code)
            .bind(&geocache)
            .bind(Utc::now())
            .execute(&self.db).await?;
        Ok(serde_json::from_value::<Geocache>(geocache)?)
    }

    async fn load_geocache(&self, code: &String, cutoff: &DateTime<Utc>) -> Option<Geocache> {
        debug!("Load {}", code);
        let json_result: Result<Option<sqlx::postgres::PgRow>, _> =
            sqlx::query("SELECT raw::VARCHAR FROM geocaches where id = $1 and ts >= $2")
                .bind(code)
                .bind(cutoff)
                .fetch_optional(&self.db)
                .await;
        match json_result {
            Ok(Some(row)) => {
                let gc: Result<serde_json::Value, serde_json::Error> =
                    serde_json::from_str(row.get(0));
                match gc {
                    Ok(v) => groundspeak::parse(&v).ok(),
                    Err(e) => {
                        error!("json failed {}", e);
                        return None;
                    }
                }
            }
            Ok(None) => None,
            Err(e) => {
                error!("Failed to load geocache {}", e);
                return None;
            }
        }
    }

    pub async fn discover(&self, tile: &Tile) -> Result<Timestamped<GcCodes>, Error> {
        // TODO think about switching from single row per tile to single row per gc code
        // update could be done in a transaction and we could natively work with sqlite
        // which would make operations easier
        info!("Discover {:#?}", tile.quadkey());
        let tile_row = sqlx::query("SELECT gccodes, ts FROM tiles where id = $1")
            .bind(tile.quadkey() as i32)
            .fetch_optional(&self.db)
            .await?;
        match tile_row {
            Some(row) => {
                let gccodes: Vec<String> = row.get(0);
                let ts: DateTime<Utc> = row.get(1);
                info!(
                    "already have a tile with {} gccodes from {}",
                    gccodes.len(),
                    ts
                );
                return Ok(Timestamped {
                    ts: ts,
                    data: gccodes,
                });
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
        Self {
            ts: Utc::now(),
            data: data,
        }
    }
}
