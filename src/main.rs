#[macro_use]
extern crate rocket;

use std::{
    collections::HashMap,
    fmt::Write,
};
use std::sync::{Arc, Condvar, Mutex};

use futures::poll;
use rocket::{Data, data::ToByteUnit, State};
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;

use gc::{Cache, Timestamped};
use gc::groundspeak::GcCode;
use gcgeo::{CacheType, Geocache};

use crate::gc::groundspeak::Groundspeak;
use crate::gcgeo::Track;
use crate::job::{Job, JobQueue};

mod gcgeo;
mod gc;
mod job;

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

    let jobs = JobQueue::new();
    let cache = Cache::new_lite().await?;

    info!("Service starting up...");

    let _rocket = rocket::build()
        .manage(jobs)
        .manage(cache)
        .mount("/", routes![index, codes, fetch, track, enqueue_task, query_task])
        .launch()
        .await?;

    Ok(())
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[derive(Responder)]
enum JobResult {
    #[response(status = 303, content_type = "text/plain")]
    Redirect(rocket::response::Redirect),
    #[response(status = 200, content_type = "text/plain")]
    Done(String),
}

// work in progress to replace /track and other methods
#[post("/enqueue", data = "<data>")]
async fn enqueue_task(data: Data<'_>, jobs: &State<JobQueue>) -> Result<JobResult, rocket::http::Status> {
    let data_stream = data.open(10.megabytes());
    let reader = data_stream.into_bytes().await.unwrap();
    let track = gcgeo::Track::from_gpx(reader.as_slice()).unwrap();
    // ugh, there must be a nicer way, right?
    let track2 = track.clone();
    let track3 = track.clone();
    let tiles = track.tiles;


    let pre_filter = {
        move |gc: &GcCode|
            match &gc.approx_coord {
                Some(coord) => track2.near(&coord) <= 100,
                None => { true }
            }
    };
    let post_filter = move |gc: &Geocache| is_active(gc) && is_quick_stop(gc) && track3.near(&gc.coord) <= 100;
    let job = Arc::new(Job::new());
    let job_for_result = job.clone();
    let job_id = job.id.clone();
    jobs.add(job.clone());
    let handle = tokio::task::spawn(async move {
        let cache = Cache::new_lite().await.unwrap();
        job.process_filtered(tiles, &cache, pre_filter, post_filter).await;
    });

    // If everything is already cached, the job will finish very quickly, and we can immediately return the result
    let timeout = tokio::time::Duration::from_secs(2);
    let _ = tokio::time::timeout(timeout, handle).await;

    if let Some(geocaches) = job_for_result.get_geocaches() {
        // TODO: render the result based on accept header
        Ok(JobResult::Done(format!("Discovered {} geocaches", geocaches.len())))
    } else {
        Ok(JobResult::Redirect(rocket::response::Redirect::to(format!("/job/{}", job_id))))
    }
}

#[get("/job/<job_id>")]
async fn query_task(job_id: &str, jobs: &State<JobQueue>) -> String {
    let job = jobs.get(job_id).unwrap();
    if let Some(geocaches) = job.get_geocaches() {
        format!("Discovered {} geocaches", geocaches.len())
    } else {
        "Task not done yet".to_string()
    }
}

#[get("/codes?<lat>&<lon>&<zoom>")]
async fn codes(lat: f64, lon: f64, zoom: Option<u8>) -> String {
    let t = gcgeo::Tile::from_coordinates(lat, lon, zoom.unwrap_or(14));
    let cache = Cache::new_lite().await.unwrap();
    match cache.discover(&t).await {
        Ok(Timestamped { data, ts: _ts }) => {
            format!("codes: {}", data.len())
        }
        Err(err) => {
            error!("Error: {:?}", err);
            err.to_string()
        }
    }
}

#[get("/get/<code>")]
async fn fetch(code: String) -> String {
    let cache = Cache::new_lite().await.unwrap();
    let geocaches = cache.get(vec![code]).await.ok().unwrap();
    let geocache = geocaches.get(0).unwrap();
    info!("Geocache: {:?}", geocache);
    // format!("{}", geocache)
    serde_json::to_string(geocache).unwrap()
}


#[post("/track", data = "<data>")]
async fn track(data: Data<'_>, accept: &rocket::http::Accept) -> Vec<u8> {
    info!("accept: {}", accept);
    let cache = Cache::new_lite().await.unwrap();
    let data_stream = data.open(10.megabytes());
    let reader = data_stream.into_bytes().await.unwrap();
    let track = gcgeo::Track::from_gpx(reader.as_slice()).unwrap();
    let tiles = cache.tracks(reader.as_slice()).await.unwrap();
    info!("Track resolved into {} tiles", &tiles.len());
    let mut gccodes: Vec<GcCode> = Vec::new();
    for (i, tile) in tiles.iter().enumerate() {
        info!("Discover tile {}/{} {}", i + 1, &tiles.len(), tile);
        let mut tmp = cache.discover(tile).await.unwrap();
        gccodes.append(&mut (tmp.data as Vec<GcCode>));
    }
    info!("Discovered {} geocaches", gccodes.len());
    let near_codes: Vec<String> = gccodes.into_iter().filter(|gc| {
        match &gc.approx_coord {
            Some(coord) => track.near(coord) <= 100,
            None => { true }
        }
    }).map(|gc| gc.code).collect();

    info!("Prefiltered {} geocaches", near_codes.len());
    let all_geocaches: Vec<Geocache> = cache.get(near_codes).await.unwrap();
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
