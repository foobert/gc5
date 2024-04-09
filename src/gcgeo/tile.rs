use std::{collections::HashSet, f64::consts::PI, fmt};

use super::Coordinate;

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub z: u8,
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}/{} #{}", self.z, self.x, self.y, self.quadkey())
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

    pub fn to_coord(&self) -> Coordinate {
        let lon = self.x as f64 / (self.z as f64).exp2() * 360.0 - 180.0;
        let n = PI - 2.0 * PI * self.y as f64 / (self.z as f64).exp2();
        let lat = 180.0 / PI * (0.5 * (n.exp() - (-n).exp())).atan();
        Coordinate { lat, lon }
    }

    pub fn top_left(&self) -> Coordinate {
        self.to_coord()
    }

    pub fn bottom_right(&self) -> Coordinate {
        Self {
            x: self.x + 1,
            y: self.y + 1,
            z: self.z,
        }
            .to_coord()
    }

    pub fn quadkey(&self) -> u32 {
        let mut result = 0;
        for i in 0..self.z {
            result |= (self.x & 1 << i) << i | (self.y & 1 << i) << (i + 1);
        }
        return result;
    }

    pub fn around(&self) -> Vec<Self> {
        let mut result = Vec::new();
        for x in self.x - 1..=self.x + 1 {
            for y in self.y - 1..=self.y + 1 {
                result.push(Self { x, y, z: self.z });
            }
        }
        result
    }

    pub fn near(coordinate: &Coordinate, radius: f64) -> Vec<Self> {
        // as a first approximation, use a square instead of a circle
        let top_left = coordinate.project(radius, 315.0);
        let bottom_right = coordinate.project(radius, 135.0);

        let top_left_tile = Self::from_coordinates(top_left.lat, top_left.lon, Self::DEFAULT_ZOOM);
        let bottom_right_tile =
            Self::from_coordinates(bottom_right.lat, bottom_right.lon, Self::DEFAULT_ZOOM);

        let mut result = HashSet::new();
        for x in top_left_tile.x..=bottom_right_tile.x {
            for y in top_left_tile.y..=bottom_right_tile.y {
                result.insert(Tile {
                    x,
                    y,
                    z: Self::DEFAULT_ZOOM,
                });
            }
        }
        result.into_iter().collect()
    }

    pub fn utf_grid_offset(&self, x: f64, y: f64) -> Coordinate {
        let lon = (self.x as f64 + x) / (self.z as f64).exp2() * 360.0 - 180.0;
        let n = PI - 2.0 * PI * (self.y as f64 + y) / (self.z as f64).exp2();
        let lat = 180.0 / PI * (0.5 * (n.exp() - (-n).exp())).atan();
        Coordinate { lat, lon }
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;

    #[test]
    fn test_corner_coordinates() {
        let uut = Tile {
            x: 8579,
            y: 5698,
            z: 14,
        };

        let top_left = uut.top_left();
        assert_approx_eq!(top_left.lat, 47.96050238891509);
        assert_approx_eq!(top_left.lon, 8.50341796875);

        let bottom_right = uut.bottom_right();
        assert_approx_eq!(bottom_right.lat, 47.945786463687185);
        assert_approx_eq!(bottom_right.lon, 8.525390625);
    }

    #[test]
    fn test_from_coordinate() {
        let uut = Tile::from_coordinates(47.947971, 8.508224, 14);
        assert_eq!(uut.x, 8579);
        assert_eq!(uut.y, 5698);
        assert_eq!(uut.z, 14);

        let uut2 = Tile::from_coordinates(47.931330700422194, 8.452201111545495, 14);
        assert_eq!(uut2.x, 8576);
        assert_eq!(uut2.y, 5699);
        assert_eq!(uut2.z, 14);
    }
}
