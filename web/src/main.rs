#[macro_use]
extern crate rocket;

use std::{
    collections::HashMap,
    fmt::Write,
};

use rocket::{Data, data::ToByteUnit, State};
use thiserror::Error;

use gc::{Cache, Timestamped};
use gcgeo::{CacheType, Geocache};

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
    env_logger::init();

    let cache = Cache::new_lite().await?;
    let jobs = HashMap::<String, String>::new();

    info!("Service starting up...");
    let _rocket = rocket::build()
        .manage(cache)
        .manage(jobs)
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
    let t = gcgeo::Tile::from_coordinates(47.931330700422194, 8.452201111545495, 14);
    match cache.discover(&t).await {
        Ok(Timestamped { data, ts: _ts }) => {
            return format!("codes: {}", data.len());
        }
        Err(err) => {
            return err.to_string();
        }
    }
}

#[get("/get/<code>")]
async fn fetch(code: String, cache: &State<Cache>) -> String {
    let geocaches = cache.get(vec![code]).await.ok().unwrap();
    let geocache = geocaches.get(0).unwrap();
    format!("{}", geocache)
}


#[post("/track", data = "<data>")]
async fn track(data: Data<'_>, accept: &rocket::http::Accept, cache: &State<Cache>) -> Vec<u8> {
    info!("accept: {}", accept);
    let datastream = data.open(10.megabytes());
    let reader = datastream.into_bytes().await.unwrap();
    let track = gcgeo::Track::from_gpx(reader.as_slice()).unwrap();
    let tiles = cache.tracks(reader.as_slice()).await.unwrap();
    info!("Track resolved into {} tiles", &tiles.len());
    let mut gccodes: Vec<String> = Vec::new();
    for (i, tile) in tiles.iter().enumerate() {
        info!("Discover tile {}/{} {}", i + 1, &tiles.len(), tile);
        let mut tmp = cache.discover(tile).await.unwrap();
        gccodes.append(&mut tmp.data);
    }
    info!("Discovered {} geocaches", gccodes.len());
    let all_geocaches: Vec<Geocache> = cache.get(gccodes).await.unwrap();
    let geocaches: Vec<Geocache> = all_geocaches
        .into_iter()
        .filter(|gc| is_active(&gc))
        .filter(|gc| is_quick_stop(gc))
        .filter(|gc| track.near(&gc.coord) <= 100)
        .collect();

    info!("accept: {}", accept.preferred().sub());
    match accept.preferred().sub().as_str() {
        "gpx" => {
            let mut output: Vec<u8> = Vec::new();
            let garmin = gc::garmin::Garmin::new(geocaches);
            garmin
                .gpx(&CacheType::Traditional, &mut output)
                .expect("gpx writing failed");
            output
        }
        "gpi" => {
            let mut output: Vec<u8> = Vec::new();
            let garmin = gc::garmin::Garmin::new(geocaches);
            garmin
                .gpi(&CacheType::Traditional, &mut output)
                .expect("gpi writing failed");
            output
        }
        _ => {
            let mut geojson = String::new();
            write!(
                &mut geojson,
                "{{\"type\": \"FeatureCollection\", \"features\": ["
            )
                .ok();
            write!(
                &mut geojson,
                r#"{{
        "type": "Feature",
        "properties": {{}},
        "geometry": {{
          "coordinates": [
    "#
            )
                .ok();
            for (i, waypoint) in track.waypoints.iter().enumerate() {
                if i > 0 {
                    write!(&mut geojson, ", ").ok();
                }
                write!(&mut geojson, "[ {}, {} ]", waypoint.lon, waypoint.lat).ok();
            }
            write!(
                &mut geojson,
                r#"
          ],
          "type": "LineString"
        }}
      }},"#
            )
                .ok();
            for geocache in geocaches {
                write!(&mut geojson, ",").ok();
                write!(
                    &mut geojson,
                    r#"{{
            "type": "Feature",
            "properties": {{"name":"{}", "marker-color":"{}"}},
            "geometry": {{
                "coordinates": [ {}, {} ],
                "type": "Point"
            }}
        }}
        "#,
                    geocache.code,
                    match geocache.cache_type {
                        CacheType::Webcam => "#ff0000",
                        CacheType::Earth => "#00ff00",
                        _ => "#000000",
                    },
                    geocache.coord.lon,
                    geocache.coord.lat
                )
                    .ok();
            }
            write!(&mut geojson, "]}}").ok();
            Vec::from(geojson.as_bytes())
        }
    }
}

fn is_active(gc: &Geocache) -> bool {
    !gc.is_premium && gc.available && !gc.archived
}

fn is_quick_stop(gc: &Geocache) -> bool {
    match gc.cache_type {
        // CacheType::Traditional | CacheType::Earth | CacheType::Webcam => true,
        CacheType::Traditional => true,
        _ => false,
    }
}
