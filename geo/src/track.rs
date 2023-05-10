use std::{collections::HashSet, io::Error};

use geo::{LineString, ClosestPoint, GeodesicDistance};

use crate::{Coordinate, Tile};

pub struct Track {
    pub tiles: Vec<Tile>,
    pub waypoints: Vec<Coordinate>,
    line_string: LineString,
}

impl Track {
    pub fn from_gpx<R: std::io::Read>(io: R) -> Result<Self, Error> {
        let gpx = gpx::read(io).unwrap();
        let waypoints: Vec<Coordinate> = gpx
            .tracks
            .iter()
            .flat_map(|track| track.segments.iter())
            .flat_map(|segment| segment.points.clone())
            .map(|waypoint| waypoint.point())
            .map(|p| Coordinate {
                lat: p.y(),
                lon: p.x(),
            })
            .collect();

        let tiles = waypoints.iter()
            .map(|coord| Tile::from_coordinates(coord.lat, coord.lon, 14))
            .flat_map(|tile| tile.around())
            .collect::<HashSet<Tile>>()
            .into_iter()
            .collect();

        let line_string = LineString::from_iter(waypoints.iter()
        .map(|coord| geo::coord! {x: coord.lon, y: coord.lat}));

        Ok(Track { tiles, waypoints, line_string})
    }

    pub fn near(&self, coord: &Coordinate) -> u16 {
        let other = geo::point! { x: coord.lon, y: coord.lat };
        let closest = self.line_string.closest_point(&other);
        let distance = match closest {
            geo::Closest::SinglePoint(p) => p.geodesic_distance(&other),
            geo::Closest::Intersection(p) => p.geodesic_distance(&other),
            _ => f64::MAX,
        };

        distance as u16
    }
}
