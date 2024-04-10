use std::collections::HashMap;

use rocket::serde::Deserialize;

use crate::gc::groundspeak::{Error, GcCode, GcCodes};
use crate::gcgeo::Tile;

#[derive(Deserialize, Debug)]
pub struct UtfGrid {
    // only used with width() and height()
    grid: Vec<String>,
    data: HashMap<String, Vec<DataObject>>,
}

#[derive(Deserialize, Debug)]
struct DataObject {
    i: String, // the actual GC code
    // n would be the geocache name, but we don't care
}


impl UtfGrid {
    pub fn width(&self) -> usize {
        self.grid[0].len()
    }

    pub fn height(&self) -> usize {
        self.grid.len()
    }

    pub fn iter(&self) -> impl Iterator<Item=(&String, &String)> {
        self.data.iter().map(|(k, v)| (k, &v[0].i))
    }

    fn extract_x_y(key: &str) -> (u8, u8) {
        // or regex?
        let parts: Vec<&str> = key.strip_prefix('(').unwrap_or(key).strip_suffix(')').unwrap_or(key).split(',').collect();
        let x = parts[0].trim().parse::<u8>().unwrap();
        let y = parts[1].trim().parse::<u8>().unwrap();
        (x, y)
    }

    // maybe we should not pass in the tile reference and just return coordinate offsets instead?
    pub async fn parse(self, tile: &Tile) -> Result<GcCodes, Error> {
        let x_size = self.width() - 1;
        let y_size = self.height() - 1;

        // collect all gc codes and their x/y positions in the grid
        let mut gccodes_with_offset = HashMap::new();
        for ((x, y), gccode) in self.into_iter() {
            let entry = gccodes_with_offset.entry(gccode).or_insert(MinMax::new(x, y));
            entry.update(x, y);
        }

        // convert x/y positions into coordinates
        let gccodes = gccodes_with_offset.iter().map(|(code, value)| {
            let x = value.mid_x() / x_size as f64;
            let y = value.mid_y() / y_size as f64;
            let coord = tile.utf_grid_offset(x, y);
            GcCode {
                code: code.to_string(),
                approx_coord: Some(coord),
            }
        }).collect();

        Ok(gccodes)
    }
}

impl IntoIterator for UtfGrid {
    type Item = ((u8, u8), String);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
            .filter(|(_, v)| v.len() == 1)
            .map(|(k, v)| (Self::extract_x_y(&k), v[0].i.clone()))
            .collect::<Vec<Self::Item>>()
            .into_iter()
    }
}

struct MinMax {
    min_x: u8,
    max_x: u8,
    min_y: u8,
    max_y: u8,
}

impl MinMax {
    fn new(x: u8, y: u8) -> Self {
        Self {
            min_x: x,
            max_x: x,
            min_y: y,
            max_y: y,
        }
    }

    fn update(&mut self, x: u8, y: u8) {
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
    }

    fn mid_x(&self) -> f64 {
        (self.max_x + self.min_x) as f64 / 2.0
    }

    fn mid_y(&self) -> f64 {
        (self.max_y + self.min_y) as f64 / 2.0
    }
}

