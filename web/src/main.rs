#[macro_use]
extern crate rocket;
use gc::{Cache, Timestamped};
use rocket::{data::ToByteUnit, Data, State};
use thiserror::Error;
use tokio::io::BufReader;

#[derive(Error, Debug)]
pub enum Error {
    #[error("db error")]
    Database(#[from] sqlx::Error),
    #[error("cache")]
    Gc(#[from] gc::Error),
    #[error("io")]
    Io(#[from] std::io::Error),
    #[error("rocket")]
    Rocket(#[from] rocket::Error),
    #[error("unknown data store error")]
    Unknown,
}

#[rocket::main]
async fn main() -> Result<(), Error> {
    println!("FOO");
    env_logger::init();

    let cache = Cache::new_lite().await?;

    let _rocket = rocket::build()
        .manage(cache)
        .mount("/", routes![index, codes, fetch, track])
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
    // let t = geo::Tile::from_coordinates(51.34469577842422, 12.374765732990399, 12);
    let t = geo::Tile::from_coordinates(47.931330700422194, 8.452201111545495, 14);
    match cache.discover(&t).await {
        Ok(Timestamped { data, ts: _ts }) => {
            return format!("codes: {}", data.len());
        }
        Err(err) => {
            return err.to_string();
        }
    }
}

#[get("/fetch")]
async fn fetch(cache: &State<Cache>) -> String {
    let t = geo::Tile::from_coordinates(51.34469577842422, 12.374765732990399, 14);
    match cache.find_tile(&t).await {
        Ok(Timestamped {
            data: _data,
            ts: _ts,
        }) => {
            return "ok".to_string();
        }
        Err(err) => {
            info!("err: {:#?}", err);
            return err.to_string();
        }
    }
}

use std::fmt::Write;

#[post("/track", data = "<data>")]
async fn track(data: Data<'_>, cache: &State<Cache>) -> String {
    let datastream = data.open(10.megabytes());
    let reader = datastream.into_bytes().await.unwrap();
    let tiles = cache.tracks(reader.as_slice()).await.unwrap();
    info!("Track resolved into {} tiles", &tiles.len());
    for (i, tile) in tiles.iter().enumerate() {
        info!("Discover tile {}/{} {}", i + 1, &tiles.len(), tile);
        cache.discover(tile).await.unwrap();
    }
    /*
    for (i, tile) in tiles.iter().enumerate() {
        info!("Fetch tile {}/{}", i + 1, &tiles.len());
        cache.find_tile(tile).await.unwrap();
    }
    */
    let mut geojson = String::new();
    write!(
        &mut geojson,
        "{{\"type\": \"FeatureCollection\", \"features\": ["
    );
    for (i, tile) in tiles.iter().enumerate() {
        let tl = tile.top_left();
        let br = tile.bottom_right();
        if i > 0 {
            write!(&mut geojson, ",");
        }
        write!(
            &mut geojson,
            r#"{{
            "type": "Feature",
            "properties": {{}},
            "geometry": {{
              "coordinates": [
                [
                  [ {}, {} ],
                  [ {}, {} ],
                  [ {}, {} ],
                  [ {}, {} ],
                  [ {}, {} ]
                ]
              ],
              "type": "Polygon"
            }}
        }}"#,
            tl.lon, tl.lat, br.lon, tl.lat, br.lon, br.lat, tl.lon, br.lat, tl.lon, tl.lat,
        );
    }
    write!(&mut geojson, "]}}");
    geojson
}
