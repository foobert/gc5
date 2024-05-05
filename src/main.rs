#[macro_use]
extern crate rocket;

use std::sync::Arc;

use rocket::{Data, data::ToByteUnit, State};
use thiserror::Error;

use gc::{Cache, Timestamped};
use gc::groundspeak::GcCode;
use gcgeo::{CacheType, Geocache};

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
    #[response(status = 200, content_type = "application/json")]
    GeoJson(Vec<u8>),
    #[response(status = 200, content_type = "application/gpx+xml")]
    Gpx(Vec<u8>),
    #[response(status = 200, content_type = "application/gpi")]
    Gpi(Vec<u8>),
}

// work in progress to replace /track and other methods
#[post("/enqueue", data = "<data>")]
async fn enqueue_task(data: Data<'_>, accept: &rocket::http::Accept, jobs: &State<JobQueue>) -> Result<JobResult, rocket::http::Status> {
    let data_stream = data.open(10.megabytes());
    let reader = data_stream.into_bytes().await.unwrap();
    let track = gcgeo::Track::from_gpx(reader.as_slice()).unwrap();
    // ugh, there must be a nicer way, right?
    let track2 = track.clone();
    let track3 = track.clone();
    let track4 = track.clone();
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
        info!("Job {} is already done", job_id);
        match accept.preferred().sub().as_str() {
            "gpx" => Ok(JobResult::Gpx(render_gpx(geocaches))),
            "gpi" => Ok(JobResult::Gpi(render_gpi(geocaches))),
            _ => Ok(JobResult::GeoJson(render_geojson(geocaches, Some(track4)))),
        }
    } else {
        info!("Job {} is still running", job_id);
        Ok(JobResult::Redirect(rocket::response::Redirect::to(format!("/job/{}", job_id))))
    }
}

#[get("/job/<job_id>")]
async fn query_task(job_id: &str, accept: &rocket::http::Accept, jobs: &State<JobQueue>) -> Vec<u8> {
    let job = jobs.get(job_id).unwrap();
    if let Some(geocaches) = job.get_geocaches() {
        render(geocaches, None, accept)
    } else {
        Vec::from(job.get_message().as_bytes())
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

fn render(geocaches: Vec<Geocache>, track: Option<Track>, accept: &rocket::http::Accept) -> Vec<u8> {
    match accept.preferred().sub().as_str() {
        "gpx" => render_gpx(geocaches),
        "gpi" => render_gpi(geocaches),
        _ => render_geojson(geocaches, track),
    }
}

fn render_gpx(geocaches: Vec<Geocache>) -> Vec<u8> {
    let mut output: Vec<u8> = Vec::new();
    gc::garmin::Garmin::gpx(geocaches, &CacheType::Traditional, &mut output)
        .expect("gpx writing failed");
    output
}

fn render_gpi(geocaches: Vec<Geocache>) -> Vec<u8> {
    let mut output: Vec<u8> = Vec::new();
    gc::garmin::Garmin::gpi(geocaches, &CacheType::Traditional, &mut output)
        .expect("gpi writing failed");
    output
}

fn render_geojson(geocaches: Vec<Geocache>, track: Option<Track>) -> Vec<u8> {
    let mut features: Vec<geojson::Feature> = geocaches.iter().map(|gc| {
        let mut properties = geojson::JsonObject::new();
        properties.insert("name".to_string(), geojson::JsonValue::from(gc.code.clone()));
        properties.insert("marker-color".to_string(), geojson::JsonValue::from("#000000"));
        geojson::Feature {
            properties: Some(properties),
            geometry: Some(geojson::Geometry::new(geojson::Value::Point(vec![gc.coord.lon, gc.coord.lat]))),
            bbox: None,
            id: None,
            foreign_members: None,
        }
    }).collect();
    if let Some(track) = track {
        let coordinates = track.waypoints.iter().map(|wp| vec![wp.lon, wp.lat]).collect();
        features.push(geojson::Feature {
            properties: None,
            geometry: Some(geojson::Geometry::new(geojson::Value::LineString(coordinates))),
            bbox: None,
            id: None,
            foreign_members: None,
        });
    }
    let geojson = geojson::GeoJson::FeatureCollection(geojson::FeatureCollection {
        features,
        bbox: None,
        foreign_members: None,
    });
    Vec::from(geojson.to_string().as_bytes())
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
        let tmp = cache.discover(tile).await.unwrap();
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
        "gpx" => render_gpx(geocaches),
        "gpi" => render_gpi(geocaches),
        _ => render_geojson(geocaches, Some(track)),
    }
}

fn is_active(gc: &Geocache) -> bool {
    !gc.is_premium && gc.available && !gc.archived
}

fn is_quick_stop(gc: &Geocache) -> bool {
    let quick_type = match gc.cache_type {
        // CacheType::Traditional | CacheType::Earth | CacheType::Webcam => true,
        CacheType::Traditional => true,
        _ => false,
    };
    let quick_diff_terrain = gc.difficulty <= 3.0 && gc.terrain <= 3.0;

    quick_type && quick_diff_terrain
}
