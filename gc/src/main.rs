#[macro_use] extern crate rocket;
use cache::{Cache, Timestamped};
use thiserror::Error;
use rocket::State;

#[derive(Error, Debug)]
pub enum Error {
    #[error("db error")]
    Database(#[from] sqlx::Error),
    #[error("groundspeak")]
    GroundSpeak(#[from] groundspeak::Error),
    #[error("cache")]
    Cache(#[from] cache::Error),
    #[error("io")]
    Io(#[from] std::io::Error),
    #[error("rocket")]
    Rocket(#[from] rocket::Error),
    #[error("unknown data store error")]
    Unknown,
}

#[rocket::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let cache = Cache::new_lite().await?;

    let _rocket = rocket::build()
    .manage(cache)
        .mount("/", routes![index, codes])
        .launch()
        .await?;

    Ok(())
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/codes")]
async fn codes(cache: &State<Cache>) -> String {
    let t = geo::Tile::from_coordinates(51.34469577842422, 12.374765732990399, 12);
    match cache.discover(&t).await {
        Ok(Timestamped { data, ts: _ts }) => {
            return format!("codes: {}", data.len());
        }
        Err(_) => {
            return "fail".to_string();
        }
    }
}
