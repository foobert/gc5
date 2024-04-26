use std::collections::HashMap;
use std::iter::Map;
use std::sync::{Arc, Mutex};

use log::debug;

use crate::{Cache, is_active, is_quick_stop};
use crate::gc::groundspeak::GcCode;
use crate::gcgeo::{Geocache, Tile};

pub struct JobQueue {
    jobs: Mutex<HashMap<String, Arc<Job>>>,
}

impl JobQueue {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
        }
    }

    pub fn add(&self, job: Arc<Job>) {
        self.jobs.lock().unwrap().insert(job.id.clone(), job);
    }

    pub fn get(&self, id: &str) -> Option<Arc<Job>> {
        self.jobs.lock().unwrap().get(id).cloned()
    }
}

pub struct Job {
    pub id: String,
    state: Mutex<JobState>,
}

struct JobState {
    message: String,
    progress: u8,
    total: u8,
    geocaches: Vec<Geocache>,
}

impl JobState {
    fn new() -> Self {
        Self {
            message: String::new(),
            progress: 0,
            total: 0,
            geocaches: Vec::new(),
        }
    }
}

impl Job {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            state: Mutex::new(JobState::new()),
        }
    }

    pub async fn process(&self, tiles: Vec<Tile>, cache: &Cache) {
        self.process_filtered(tiles, cache, |_| true, |_| true).await;
    }

    pub async fn process_filtered<PRE, POST>(&self, tiles: Vec<Tile>, cache: &Cache, pre_filter: PRE, post_filter: POST)
        where PRE: Fn(&GcCode) -> bool, POST: Fn(&Geocache) -> bool {
        info!("Processing job {}", self.id);
        let mut codes: Vec<String> = Vec::new();
        let tile_len = tiles.len();
        for (index, tile) in tiles.iter().enumerate() {
            self.set_message(&format!("Discover tile {}/{}: {}", index + 1, tile_len, tile));
            let mut tmp = cache.discover(&tile).await.unwrap();
            tmp.data.into_iter().filter(|code| pre_filter(code)).for_each(|code| codes.push(code.code));
        }

        self.set_message(&format!("Downloading {} geocaches", codes.len()));
        let all_geocaches: Vec<Geocache> = cache.get(codes.clone()).await.unwrap();
        let selected = all_geocaches.into_iter().filter(|gc| post_filter(&gc)).collect();

        let state = &mut self.state.lock().unwrap();
        state.geocaches = selected;
        self.set_message("Finished");
    }

    fn set_message(&self, message: &str) {
        let mut state = self.state.lock().unwrap();
        info!("Job {}: {}", self.id, message);
        state.message = message.to_string();
    }

    pub fn get_geocaches(&self) -> Option<Vec<Geocache>> {
        let state = &self.state.lock().unwrap();
        let geocaches = &state.geocaches;
        if geocaches.is_empty() {
            None
        } else {
            Some(geocaches.to_vec())
        }
    }
}
