use geo::{GcCodes, Tile};
use groundspeak::Groundspeak;

use log::info;
use chrono::prelude::*;
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;

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
            .connect("sqlite:gc.db").await?;
        let gs = Groundspeak::new();
        Ok(Self { db: pool, groundspeak: gs })
    }

    pub async fn discover(&self, tile: &Tile) -> Result<Timestamped<GcCodes>, Error> {
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
                return Ok(Timestamped { ts: Utc::now(), data: codes });
            }
        }
    }
}

pub struct Timestamped<T> {
    pub ts: DateTime<Utc>,
    pub data: T,
}
