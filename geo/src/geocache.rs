use std::fmt;

use serde::Serialize;

use crate::Coordinate;

pub type GcCodes = Vec<String>;

#[derive(Debug, Serialize)]
pub struct Geocache {
    pub code: String,
    pub name: String,
    pub is_premium: bool,
    pub terrain: f32,
    pub difficulty: f32,
    pub coord: Coordinate,
    pub short_description: String,
    pub long_description: String,
    pub encoded_hints: String,
    pub size: ContainerSize,
    pub cache_type: CacheType,
    pub archived: bool,
    pub available: bool,
    pub logs: Vec<GeocacheLog>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum ContainerSize {
    Nano,
    Micro,
    Small,
    Regular,
    Large,
    Unknown,
}

impl fmt::Display for ContainerSize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl fmt::Display for CacheType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}


impl fmt::Display for Geocache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code)
    }
}

impl Geocache {
    pub fn premium(code: String) -> Geocache {
        Self {
            code,
            name: String::new(),
            is_premium: true,
            available: false,
            archived: false,
            terrain: 0.0,
            difficulty: 0.0,
            coord: Coordinate { lat: 0.0, lon: 0.0 },
            short_description: String::new(),
            long_description: String::new(),
            encoded_hints: String::new(),
            size: ContainerSize::Unknown,
            cache_type: CacheType::Unknown,
            logs: vec![],
        }
    }
}

impl ContainerSize {
    pub fn from(size: u64) -> Self {
        match size {
            2 => Self::Micro,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Serialize)]
pub struct GeocacheLog {
    pub text: String,
    pub timestamp: String,
    pub log_type: LogType,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum LogType {
    Found,
    DidNotFind,
    WriteNote,
    Unknown,
}

impl LogType {
    pub fn from(cache_type: u64) -> Self {
        match cache_type {
            2 => Self::Found,
            3 => Self::DidNotFind,
            _ => Self::Unknown,
        }
    }
}