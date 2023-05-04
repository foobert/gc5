use serde::{Deserialize, Serialize};
use std::{collections::HashSet, f64::consts::PI, fmt};

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub z: u8,
}

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
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

#[derive(Serialize, Deserialize, Debug)]
pub enum ContainerSize {
    Nano,
    Micro,
    Small,
    Regular,
    Large,
    Unknown,
}

impl ContainerSize {
    pub fn from(size: u64) -> Self {
        match size {
            2 => Self::Micro,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum CacheType {
    Traditional,
    Multi,
    Earth,
    Webcam,
    Mystery,
    Wherigo,
    Event,
    Virtual,
    Letterbox,
    Cito,
    Ape,
    MegaEvent,
    GigaEvent,
    GpsAdventures,
    Headquarter,
    Waypoint,
    Unknown,
}

impl CacheType {
    pub fn from(cache_type: u64) -> Self {
        match cache_type {
            2 => Self::Traditional,
            1858 => Self::Wherigo,
            6 => Self::Event,
            8 => Self::Mystery,
            3 => Self::Multi,
            137 => Self::Earth,
            4 => Self::Virtual,
            5 => Self::Letterbox,
            13 => Self::Cito,
            9 => Self::Ape,
            11 => Self::Webcam,
            453 => Self::MegaEvent,
            1304 => Self::GpsAdventures,
            3773 => Self::Headquarter,
            7005 => Self::GigaEvent,
            0 => Self::Waypoint,
            _ => Self::Unknown,
        }
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

impl Coordinate {
    const EARTH_RADIUS: u32 = 6_371_000; // radius of earth in meters
    pub fn project(&self, distance: f64, bearing: f64) -> Self {
        // see http://www.movable-type.co.uk/scripts/latlong.html
        // (all angles in radians)
        let lat_rad = self.lat * PI / 180.0;
        let lon_rad = self.lon * PI / 180.0;
        let bearing_rad = bearing * PI / 180.0;

        let lat_rad2 = (lat_rad.sin() * (distance / Self::EARTH_RADIUS as f64).cos()
            + lat_rad.cos() * (distance / Self::EARTH_RADIUS as f64).sin() * bearing_rad.cos())
        .asin();
        let mut lon_rad2 = lon_rad
            + (bearing_rad.sin() * (distance / Self::EARTH_RADIUS as f64).sin() * lat_rad.cos())
                .atan2(
                    (distance / Self::EARTH_RADIUS as f64).cos() - lat_rad.sin() * lat_rad2.sin(),
                );

        // The longitude can be normalised to −180…+180 using (lon+540)%360-180
        lon_rad2 = (lon_rad2 + 540.0) % 360.0 - 180.0;

        let lat2 = lat_rad2 * 180.0 / PI;
        let lon2 = lon_rad2 * 180.0 / PI;
        Coordinate {
            lat: lat2,
            lon: lon2,
        }
    }

    pub fn distance(&self, other: &Coordinate) -> f64 {
        let lat_rad1 = self.lat * PI / 180.0;
        let lat_rad2 = other.lat * PI / 180.0;
        let delta_lat = (other.lat - self.lat) * PI / 180.0;
        let delta_lon = (other.lon - self.lon) * PI / 180.0;

        let a = (delta_lat / 2.0).sin() * (delta_lat / 2.0).sin()
            + lat_rad1.cos() * lat_rad2.cos() * (delta_lon / 2.0).sin() * (delta_lon / 2.0).sin();
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        Self::EARTH_RADIUS as f64 * c // in metres
    }
}
