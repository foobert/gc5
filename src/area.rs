use crate::gc::Cache;
use crate::gcgeo::{Coordinate, Tile};
use crate::job::{Job, JobQueue};
use std::sync::Arc;

pub async fn compute_area(coordinate: &Coordinate, radius: f64, jobs: &JobQueue) -> Arc<Job> {
    let job = Arc::new(Job::new());
    let job_for_result = job.clone();
    jobs.add(job.clone());

    let tiles = Tile::near(coordinate, radius);
    let handle = tokio::task::spawn(async move {
        let cache = Cache::new_lite().await.unwrap();
        job.process(tiles, &cache).await;
    });

    // If everything is already cached, the job will finish very quickly, and we can immediately return the result
    let timeout = tokio::time::Duration::from_secs(2);
    let _ = tokio::time::timeout(timeout, handle).await;

    job_for_result
}