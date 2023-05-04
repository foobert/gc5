use crate::Coordinate;
use std::{collections::HashSet, f64::consts::PI, fmt};

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub z: u8,
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}/{}", self.z, self.x, self.y)
    }
}
impl Tile {
    const DEFAULT_ZOOM: u8 = 12;

    pub fn from_coordinates(lat: f64, lon: f64, z: u8) -> Self {
        let lat_rad = lat * PI / 180.0;
        let n = 2_i32.pow(z as u32) as f64;
        let x = ((lon + 180.0) / 360.0 * n) as u32;
        let y = ((1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI) / 2.0 * n) as u32;
        return Self { x, y, z };
    }

    pub fn quadkey(&self) -> u32 {
        let mut result = 0;
        for i in 0..self.z {
            result |= (self.x & 1 << i) << i | (self.y & 1 << i) << (i + 1);
        }
        return result;
    }

    pub fn near(coordinate: &Coordinate, radius: f64) -> Vec<Self> {
        // as a first approximation, use a square instead of a circle
        let top_left = coordinate.project(radius, 315.0);
        let bottom_right = coordinate.project(radius, 135.0);

        let top_left_tile = Self::from_coordinates(top_left.lat, top_left.lon, Self::DEFAULT_ZOOM);
        let bottom_right_tile =
            Self::from_coordinates(bottom_right.lat, bottom_right.lon, Self::DEFAULT_ZOOM);

        let mut result = HashSet::new();
        for x in top_left_tile.x..bottom_right_tile.x {
            for y in top_left_tile.y..bottom_right_tile.y {
                result.insert(Tile {
                    x,
                    y,
                    z: Self::DEFAULT_ZOOM,
                });
            }
        }
        result.into_iter().collect()
    }
}