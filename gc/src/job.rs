use std::{rc::Rc, sync::{Arc, Mutex}};

use gcgeo::{Tile, Geocache};
use log::debug;

use crate::Cache;

pub struct Job {
    tiles: Vec<Tile>,
    pub id: String,
    pub geocaches: Vec<Geocache>,
}

impl Job {
    pub fn new(tiles: Vec<Tile>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tiles,
            geocaches: vec![],
        }
    }
    pub async fn process(mut job: &mut Job, cache: &Cache) {
        let mut codes: Vec<String> = Vec::new();
        for tile in job.tiles.iter() {
            debug!("Discover tile {}", tile);
            // TODO deal with unreap here
            let mut tmp = cache.discover(tile).await.unwrap();
            codes.append(&mut tmp.data);
        }
        debug!("Discovered {} geocaches", codes.len());
        job.geocaches = cache.get(codes).await.unwrap();
        job.tiles.clear();
    }

    pub fn is_done(&self) -> bool {
        self.tiles.is_empty()
    }
}
