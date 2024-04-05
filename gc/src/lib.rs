use std::collections::HashMap;

use chrono::prelude::*;
use log::{debug, error, info};
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use thiserror::Error;

use gcgeo::{Coordinate, GcCodes, Geocache, Tile, Track};

use crate::groundspeak::{Groundspeak, parse};
use crate::tokencache::AuthProvider;

pub mod groundspeak;
pub mod job;
pub mod garmin;
mod tokencache;

pub struct Cache {
    db: sqlx::PgPool,
    groundspeak: Groundspeak,
    token_cache: AuthProvider,
    jobs: HashMap<String, job::Job>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("gc")]
    Geocaching,
    #[error("db error")]
    Database(#[from] sqlx::Error),
    #[error("groundspeak")]
    GroundSpeak(#[from] groundspeak::Error),
    #[error("reqwest")]
    Reqwest(#[from] reqwest::Error),
    #[error("json")]
    Json(#[from] serde_json::Error),
    #[error("io")]
    IO(#[from] std::io::Error),
    #[error("gpx")]
    Gpx(#[from] gpx::errors::GpxError),
    #[error("utf8")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("unknown data store error")]
    Unknown,
}

impl Cache {
    pub fn new(pool: sqlx::PgPool) -> Self {
        let groundspeak = Groundspeak::new();
        let token_cache = AuthProvider::new(pool.clone());
        return Self {
            db: pool,
            groundspeak,
            token_cache,
            jobs: HashMap::new(),
        };
    }

    pub async fn new_lite() -> Result<Self, Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect("postgres://localhost/gc")
            .await?;
        let s = Self::new(pool);
        s.token_cache.init().await?;
        Ok(s)
    }

    /*
    pub fn compute(foo: Arc<Mutex<Self>>, tiles: Vec<Tile>) -> Result<(), Error> {
        let job = job::Job::new(foo.clone(), tiles);
        foo.lock().unwrap().jobs.insert(job.id.clone(), job);
        Ok(())
    }
    */

    pub async fn find_tile(&mut self, tile: &Tile) -> Result<Timestamped<Vec<Geocache>>, Error> {
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
        let cutoff = Utc::now() - chrono::Duration::days(7);
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
        info!("missing: {:?}", cache_miss);

        if !cache_miss.is_empty() {
            info!("Fetching {} geocaches from Groundspeak", cache_miss.len());
            let chunk_size = 50;
            let mut fetched = Vec::new();
            for chunk in cache_miss.chunks(chunk_size) {
                let chunk: Vec<&String> = chunk.into_iter().collect();
                fetched.extend(self.fetch_chunk(chunk).await?);
            }

            /*
            let mut fetched: Vec<Geocache> = stream::iter(&cache_miss)
                .chunks(groundspeak::BATCH_SIZE)
                .then(|x| self.groundspeak.fetch(token, x))
                .filter_map(|x| ready(x.ok()))
                .flat_map(stream::iter)
                .then(|x| self.save_geocache(x))
                .filter_map(|x| ready(x.ok()))
                .collect()
                .await;

             */

            if fetched.len() < cache_miss.len() {
                error!("Got back less than the expected number of geocaches {} < {}", fetched.len(), cache_miss.len());
                return Err(Error::Geocaching);
            }
            cache_hit.append(&mut fetched);
        }

        Ok(cache_hit)
    }

    async fn fetch_chunk(&self, codes: Vec<&String>) -> Result<Vec<Geocache>, Error> {
        info!("Fetching {} geocaches from Groundspeak", codes.len());
        let mut attempts = 0;
        while attempts < 2 {
            let token = self.token_cache.token().await?;
            let fetched = self.groundspeak.fetch(&token, codes.clone()).await;
            match fetched {
                Ok(fetched) => {
                    info!("Fetched {} geocaches from Groundspeak", fetched.len());
                    let mut result = Vec::new();
                    for geocache in fetched {
                        result.push(self.save_geocache(geocache).await?);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    error!("Unable to fetch geocaches from Groundspeak, refreshing token {:?}", e);
                    self.token_cache.refresh().await?;
                    attempts += 1;
                }
            }
        }
        Err(Error::Geocaching)
    }


    async fn save_geocache(&self, geocache: serde_json::Value) -> Result<Geocache, Error> {
        let code = geocache["referenceCode"].as_str().ok_or(Error::Geocaching)?;
        debug!("Save {}", code);
        sqlx::query("INSERT INTO geocaches (id, raw, ts) VALUES ($1, $2, $3) ON CONFLICT (id) DO UPDATE SET raw = $2::JSON, ts = $3")
            .bind(&code)
            .bind(&geocache)
            .bind(Utc::now())
            .execute(&self.db).await?;
        Ok(parse(&geocache)?)
    }

    async fn load_geocache(&self, code: &String, cutoff: &DateTime<Utc>) -> Option<Geocache> {
        debug!("Load {}", code);
        match self.load_geocache_err(code, cutoff).await {
            Ok(v) => v,
            Err(e) => {
                error!("Unable to load geocache {}: {}", code, e);
                None
            }
        }
    }
    async fn load_geocache_err(&self, code: &String, cutoff: &DateTime<Utc>) -> Result<Option<Geocache>, Error> {
        let json_result: Option<sqlx::postgres::PgRow> =
            sqlx::query("SELECT raw::VARCHAR FROM geocaches where id = $1 and ts >= $2")
                .bind(code)
                .bind(cutoff)
                .fetch_optional(&self.db)
                .await?;
        match json_result {
            Some(row) => {
                let gc: serde_json::Value = serde_json::from_str(row.get(0))?;
                return Ok(Some(groundspeak::parse(&gc)?));
            }
            None => {
                return Ok(None);
            }
        }
    }

    pub async fn discover(&self, tile: &Tile) -> Result<Timestamped<GcCodes>, Error> {
        // TODO think about switching from single row per tile to single row per gc code
        // update could be done in a transaction and we could natively work with sqlite
        // which would make operations easier
        debug!("Discover {}", tile);
        let cutoff = Utc::now() - chrono::Duration::days(7);
        let tile_row = sqlx::query("SELECT gccodes, ts FROM tiles where id = $1 and ts >= $2")
            .bind(tile.quadkey() as i32)
            .bind(cutoff)
            .fetch_optional(&self.db)
            .await?;
        match tile_row {
            Some(row) => {
                let gccodes: Vec<String> = row.get(0);
                let ts: DateTime<Utc> = row.get(1);
                debug!(
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

    pub async fn tracks<R: std::io::Read>(&self, io: R) -> Result<Vec<Tile>, Error> {
        let track = Track::from_gpx(io)?;
        Ok(track.tiles)
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
            data,
        }
    }
}
