use std::{io::Write, path::Path, process::Command};

use geo::Point;
use gpx::{GpxVersion, Waypoint};
use log::{error, info};
use regex::Regex;
use tempfile::NamedTempFile;

use gcgeo::Geocache;

use crate::Error;

pub struct Garmin {
    geocaches: Vec<Geocache>,
}

impl Garmin {
    pub fn new(geocaches: Vec<Geocache>) -> Self {
        Self { geocaches }
    }
    pub fn gpx<W: Write>(&self, cache_type: &gcgeo::CacheType, writer: &mut W) -> Result<(), Error> {
        info!("Writing gpx");
        let mut gpx = gpx::Gpx::default();
        gpx.creator = Some(String::from("cachecache"));
        gpx.version = GpxVersion::Gpx11;
        gpx.waypoints.extend(
            self.geocaches
                .iter()
                .filter(|gc| gc.cache_type == *cache_type)
                .map(|gc| {
                    let mut waypoint = Waypoint::new(Point::new(gc.coord.lon, gc.coord.lat));
                    waypoint.name = Some(Self::title(&gc));
                    waypoint.description = Some(Self::description(&gc));
                    waypoint._type = Some(String::from("geocache"));
                    waypoint
                }),
        );
        gpx::write(&gpx, writer)?;
        Ok(())
    }

    pub fn gpi<W: ?Sized>(&self, cache_type: &gcgeo::CacheType, writer: &mut W) -> Result<(), Error>
        where
            W: Write,
    {
        let mut gpx_file = NamedTempFile::new()?;
        let mut gpi_file = NamedTempFile::new()?;
        let image_file = NamedTempFile::new()?;
        self.gpx(cache_type, &mut gpx_file)?;
        info!(
            "Wrote {:?} to {}",
            cache_type,
            gpx_file.path().to_string_lossy()
        );
        std::fs::copy(Path::new("image.bmp"), image_file.path())?;
        info!("Copied image to {}", image_file.path().to_string_lossy());
        let gpsbabel_output = Command::new("gpsbabel")
            .args([
                "-i",
                "gpx",
                "-f",
                &gpx_file.path().to_string_lossy(),
                "-o",
                &format!(
                    "garmin_gpi,bitmap={},sleep=1",
                    image_file.path().to_string_lossy()
                ),
                "-F",
                &gpi_file.path().to_string_lossy(),
            ])
            .output()?;
        if !gpsbabel_output.status.success() {
            error!(
                "gpsbabel returned {}: {}",
                gpsbabel_output.status,
                std::str::from_utf8(&gpsbabel_output.stderr)?
            );
            return Err(Error::Unknown);
        }
        std::io::copy(&mut gpi_file, writer)?;
        info!("Copied {} to output", gpi_file.path().to_string_lossy());
        Ok(())
    }

    pub fn gpi_zip<W: Write>(&self, _: W) -> Result<(), Error> {
        Err(Error::Unknown)
    }

    fn title(gc: &Geocache) -> String {
        format!(
            "{} {}{} {}",
            Self::code(gc),
            Self::size(gc),
            Self::gctype(gc),
            Self::skill(gc)
        )
    }

    fn code(gc: &Geocache) -> String {
        String::from(&gc.code[2..])
    }

    fn size(gc: &Geocache) -> String {
        Self::first_char(&gc.size)
    }

    fn gctype(gc: &Geocache) -> String {
        Self::first_char(&gc.cache_type)
    }

    fn first_char<D: std::fmt::Display>(x: &D) -> String {
        String::from(&x.to_string()[..1]).to_ascii_uppercase()
    }

    fn skill(gc: &Geocache) -> String {
        format!("{:.1}/{:.1}", gc.difficulty, gc.terrain)
    }

    fn description(gc: &Geocache) -> String {
        let hint = Self::hint(gc);
        let newline = if hint.len() > 0 { "\n" } else { "" };
        let description = format!("{}{}{}", Self::name(gc), newline, hint);
        description.chars().into_iter().take(100).collect()
    }

    fn hint(gc: &Geocache) -> String {
        Self::clean(&gc.encoded_hints)
    }

    fn name(gc: &Geocache) -> String {
        Self::clean(&gc.name)
    }

    fn clean(str: &String) -> String {
        lazy_static::lazy_static! {
            static ref PATTERN_WHITESPACE: Regex = Regex::new(r"\s{2,}").unwrap();
            static ref PATTERN_ALLOWED: Regex = Regex::new(r"[^\w;:?!,.\-=_/@$%*+() |\n]").unwrap();
        }

        let clean1 = str
            .replace("ä", "ae")
            .replace("ö", "oe")
            .replace("ü", "ue")
            .replace("Ä", "AE")
            .replace("Ö", "OE")
            .replace("Ü", "UE")
            .replace("ß", "ss");
        let clean2 = PATTERN_ALLOWED.replace_all(&clean1, "");
        let clean3 = PATTERN_WHITESPACE.replace_all(&clean2, " ");

        return String::from(clean3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_removes_unicode() {
        let cleaned = Garmin::clean(&String::from("smile 🙂 for me"));
        assert_eq!(cleaned, String::from("smile for me"));
    }
}
