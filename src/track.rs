use std::sync::Arc;

use crate::gc::Cache;
use crate::gc::groundspeak::GcCode;
use crate::gcgeo::{CacheType, Geocache, Track};
use crate::job::{Job, JobQueue};

pub async fn compute_track(track: Track, jobs: &JobQueue) -> Arc<Job> {
    // ugh, there must be a nicer way, right?
    let track_pre_filter = track.clone();
    let track_post_filter = track.clone();
    let tiles = track.tiles;

    let pre_filter = {
        move |gc: &GcCode|
            match &gc.approx_coord {
                Some(coord) => track_pre_filter.near(&coord) <= 100,
                None => { true }
            }
    };
    let post_filter = move |gc: &Geocache| is_active(gc) && is_quick_stop(gc) && track_post_filter.near(&gc.coord) <= 100;
    let job = Arc::new(Job::new());
    let job_for_result = job.clone();
    jobs.add(job.clone());
    let handle = tokio::task::spawn(async move {
        let cache = Cache::new_lite().await.unwrap();
        job.process_filtered(tiles, &cache, pre_filter, post_filter).await;
    });

    // If everything is already cached, the job will finish very quickly, and we can immediately return the result
    let timeout = tokio::time::Duration::from_secs(2);
    let _ = tokio::time::timeout(timeout, handle).await;

    job_for_result
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
