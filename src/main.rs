#[macro_use]
extern crate rocket;

use std::str::FromStr;

use geojson::GeoJson;
use rocket::{Data, data::ToByteUnit, State};
use rocket::form::Form;
use rocket::http::Accept;
use rocket::response::Responder;
use rocket_dyn_templates::{context, Template};
use thiserror::Error;

use gc::Cache;
use gcgeo::{CacheType, Geocache};

use crate::job::JobQueue;
use crate::track::compute_track;

mod gcgeo;
mod gc;
mod job;
mod track;
mod area;

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
        .mount("/", routes![index, list_jobs, upload, fetch, enqueue_task, query_task, query_task_gpi, enqueue_area])
        .attach(Template::fairing())
        .launch()
        .await?;

    Ok(())
}

#[get("/")]
async fn index(jobs: &State<JobQueue>) -> Template {
    list_jobs(jobs).await
    // Template::render("index", context! { field: "value" })
}

enum JobResult {
    Complete(Vec<Geocache>, Option<Accept>),
    Incomplete(String),
}

impl<'a> Responder<'a, 'static> for JobResult {
    fn respond_to(self, req: &'a rocket::Request<'_>) -> rocket::response::Result<'static> {
        match self {
            JobResult::Complete(data, forced_accept) => {
                let json = rocket::http::Accept::JSON;
                let accept = forced_accept.as_ref().or(req.accept()).unwrap_or(&json);
                match accept.preferred().sub().as_str() {
                    "gpx" => {
                        let mut output: Vec<u8> = Vec::new();
                        gc::garmin::Garmin::gpx(data, &CacheType::Traditional, &mut output)
                            .expect("gpx writing failed");
                        rocket::response::Response::build()
                            .header(rocket::http::ContentType::XML)
                            .sized_body(output.len(), std::io::Cursor::new(output))
                            .ok()
                    }
                    "gpi" => {
                        let mut output: Vec<u8> = Vec::new();
                        gc::garmin::Garmin::gpi(data, &CacheType::Traditional, &mut output)
                            .expect("gpi writing failed");
                        rocket::response::Response::build()
                            .header(rocket::http::ContentType::parse_flexible("application/gpi").unwrap())
                            .sized_body(output.len(), std::io::Cursor::new(output))
                            .ok()
                    }
                    _ => {
                        let json = bundle_geojson(data).to_string();
                        rocket::response::Response::build()
                            .header(rocket::http::ContentType::Plain)
                            .sized_body(json.len(), std::io::Cursor::new(json))
                            .ok()
                    }
                }
            }
            JobResult::Incomplete(message) => {
                rocket::response::Response::build()
                    .header(rocket::http::ContentType::Plain)
                    .sized_body(message.len(), std::io::Cursor::new(message))
                    .ok()
            }
        }
    }
}

fn bundle_geojson(data: Vec<Geocache>) -> GeoJson {
    let features: Vec<geojson::Feature> = data.iter().map(|gc| {
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
    GeoJson::FeatureCollection(geojson::FeatureCollection {
        features,
        bbox: None,
        foreign_members: None,
    })
}

#[post("/track", data = "<data>")]
async fn enqueue_task(data: Data<'_>, jobs: &State<JobQueue>) -> Result<JobResult, rocket::http::Status> {
    let data_stream = data.open(10.megabytes());
    let reader = data_stream.into_bytes().await.unwrap();
    let track = gcgeo::Track::from_gpx(reader.as_slice()).unwrap();
    let job = compute_track(track, jobs.inner()).await;

    if let Some(geocaches) = job.get_geocaches() {
        info!("Job {} is already done", job.id);
        Ok(JobResult::Complete(geocaches, None))
    } else {
        info!("Job {} is still running", job.id);
        Ok(JobResult::Incomplete(job.get_message()))
    }
}

#[get("/area/<lat>/<lon>/<radius>")]
async fn enqueue_area(lat: &str, lon: &str, radius: &str, jobs: &State<JobQueue>) -> Result<JobResult, rocket::http::Status> {
    let lat = lat.parse::<f64>().unwrap();
    let lon = lon.parse::<f64>().unwrap();
    let radius = radius.parse::<f64>().unwrap();
    let job = compute_area(&Coordinate { lat, lon }, radius, jobs.inner()).await;
    if let Some(geocaches) = job.get_geocaches() {
        info!("Job {} is already done", job.id);
        Ok(JobResult::Complete(geocaches, None))
    } else {
        info!("Job {} is still running", job.id);
        Ok(JobResult::Incomplete(job.get_message()))
    }
}

#[derive(FromForm)]
struct UploadForm<'r> {
    file: &'r [u8],
}

#[get("/jobs")]
async fn list_jobs(jobs: &State<JobQueue>) -> Template {
    let mut jobs_for_context = Vec::new();
    for job in jobs.list().iter() {
        jobs_for_context.push((job.id.clone(), job.get_message()));
    }
    Template::render("jobs", context! { jobs: jobs_for_context })
}

#[post("/jobs", data = "<data>")]
async fn upload(data: Form<UploadForm<'_>>, jobs: &State<JobQueue>) -> Template {
    let track = gcgeo::Track::from_gpx(data.file).unwrap();
    compute_track(track, jobs.inner()).await;
    list_jobs(jobs).await
}

#[get("/jobs/<job_id>")]
async fn query_task(job_id: &str, jobs: &State<JobQueue>) -> JobResult {
    let job = jobs.get(job_id).unwrap();
    if let Some(geocaches) = job.get_geocaches() {
        JobResult::Complete(geocaches, None)
    } else {
        JobResult::Incomplete(job.get_message())
    }
}

#[get("/jobs/<job_id>/gpi")]
async fn query_task_gpi(job_id: &str, jobs: &State<JobQueue>) -> JobResult {
    let job = jobs.get(job_id).unwrap();
    if let Some(geocaches) = job.get_geocaches() {
        JobResult::Complete(geocaches, Some(Accept::from_str("application/gpi").unwrap()))
    } else {
        JobResult::Incomplete(job.get_message())
    }
}

// for debugging, needed?
#[get("/geocache/<code>")]
async fn fetch(code: String) -> String {
    let cache = Cache::new_lite().await.unwrap();
    let geocaches = cache.get(vec![code]).await.ok().unwrap();
    let geocache = geocaches.get(0).unwrap();
    info!("Geocache: {:?}", geocache);
    serde_json::to_string(geocache).unwrap()
}