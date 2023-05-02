use std::fmt;
use serde::{Serialize, Deserialize};

#[derive(Debug)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

#[derive(Serialize,Deserialize,Debug)]
pub struct Coordinate {
    pub lat: f64,
    pub lon: f64,
}

impl fmt::Display for Coordinate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.lat, self.lon)
    }
}

pub type GcCodes = Vec<String>;

#[derive(Serialize,Deserialize,Debug)]
#[serde(rename_all="PascalCase")]
pub struct Geocache {
    pub code: String,
    pub name: String,
    pub terrain: f32,
    pub difficulty: f32,
    pub coord: Coordinate,
    pub short_description: String,
    pub long_description: String,
    pub encoded_hints: String,
    pub size: ContainerSize,
    pub cache_type: CacheType,
}

#[derive(Serialize,Deserialize,Debug)]
pub enum ContainerSize {
    Large,
    Medium,
    Small,
    Nano,
    Unknown,
}

#[derive(Serialize,Deserialize,Debug)]
pub enum CacheType {
    Traditional,
    Multi,
    Earth,
    Webcam,
    Mystery,
    Wherigo,
}

impl Tile {
    pub fn from_coordinates(lat: f64, lon: f64, z: u32) -> Self {
        const PI : f64 = std::f64::consts::PI;
        let lat_rad = lat * PI / 180.0;
        let n = 2_i32.pow(z) as f64;
        let x = ((lon + 180.0) / 360.0 * n) as u32;
        let y = ((1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI) / 2.0 * n) as u32;
        return Self { x: x, y: y, z: z };
    }

    pub fn quadkey(&self) -> u32 {
        let mut result = 0;
        for i in 0..self.z {
            result |= (self.x & 1 << i) << i | (self.y & 1 << i) << (i + 1);
        }
        return result;
    }
}

