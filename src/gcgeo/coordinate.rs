use std::{f64::consts::PI, fmt};

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Coordinate {
    pub lat: f64,
    pub lon: f64,
}

impl fmt::Display for Coordinate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.lat, self.lon)
    }
}

impl Coordinate {
    const EARTH_RADIUS: u32 = 6_371_000;
    // radius of earth in meters
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