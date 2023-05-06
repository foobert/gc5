use std::{collections::HashSet, io::Error};

use crate::{Coordinate, Tile};

pub struct Track {
    pub tiles: Vec<Tile>,
}

impl Track {
    pub fn from_gpx<R: std::io::Read>(io: R) -> Result<Self, Error> {
        let gpx = gpx::read(io).unwrap();
        let tiles: Vec<Tile> = gpx
            .tracks
            .iter()
            .flat_map(|track| track.segments.iter())
            .flat_map(|segment| segment.points.clone())
            .map(|waypoint| waypoint.point())
            .map(|p| Coordinate {
                lat: p.y(),
                lon: p.x(),
            })
            // .flat_map(|coord| Tile::near(&coord, 2000.0))
            .map(|coord| Tile::from_coordinates(coord.lat, coord.lon, 14))
            .flat_map(|tile| tile.around())
            .collect::<HashSet<Tile>>()
            .into_iter()
            .collect();

        Ok(Track { tiles })
    }
}
