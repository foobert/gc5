use geo::{GcCodes, Tile, Geocache, Coordinate};
use crate::groundspeak::Groundspeak;

use log::info;
use chrono::prelude::*;
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;

pub mod groundspeak;

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
        self.groundspeak.fetch(codes.data).await?;
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

    pub async fn get(&self, code: &str) -> Result<Geocache, Error> {
        // needed?
        info!("get {}", code);
        Err(Error::Unknown)
    }

    pub async fn discover(&self, tile: &Tile) -> Result<Timestamped<GcCodes>, Error> {
        // TODO think about switching from single row per tile to single row per gc code
        // update could be done in a transaction and we could natively work with sqlite
        // which would make operations easier
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
