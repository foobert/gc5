use std::time::Duration;

use chrono::{NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use log::{debug, info};
use rand::Rng;
use thiserror::Error;
use tokio::time::sleep;

use crate::gc::utfgrid::UtfGrid;
use crate::gcgeo::{CacheType, ContainerSize, Coordinate, Geocache, GeocacheLog, LogType, Tile};

pub const BATCH_SIZE: usize = 50;

pub struct Groundspeak {
    client: reqwest::Client,
}

pub type GcCodes = Vec<GcCode>;

#[derive(Debug, Clone)]
pub struct GcCode {
    pub code: String,
    pub approx_coord: Option<Coordinate>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("request error")]
    HttpRequest(#[from] reqwest::Error),
    #[error("json")]
    Json(#[from] serde_json::Error),
    #[error("json_raw")]
    JsonRaw,
    #[error("chrono")]
    Chrono(#[from] chrono::ParseError),
    #[error("chrono-tz")]
    ChronoTz(#[from] chrono_tz::ParseError),
    #[error("unknown error")]
    Unknown,
}

impl Groundspeak {
    const FETCH_URL: &'static str = "https://api.groundspeak.com/v1.0/geocaches";

    const USER_AGENT: &'static str = "User-Agent: Mozilla/6.0 (Macintosh; Intel Mac OS X 10.15; rv:109.0) Gecko/20100101 Firefox/112.0";

    const USER_AGENT_FETCH: &'static str = env!("USERAGENT");

    //const FETCH_FIELDS: &'static str = "referenceCode,ianaTimezoneId,name,postedCoordinates,geocacheType,geocacheSize,difficulty,terrain,userData,favoritePoints,placedDate,eventEndDate,ownerAlias,owner,isPremiumOnly,userData,lastVisitedDate,status,hasSolutionChecker";
    const EXPAND_FIELDS: &'static str = "geocachelogs:5";
    const FETCH_FIELDS: &'static str = "referenceCode,name,postedCoordinates,geocacheType,geocacheSize,difficulty,terrain,favoritePoints,placedDate,isPremiumOnly,lastVisitedDate,status,shortDescription,longDescription,hints,additionalWaypoints,geocachelogs[loggedDate,ianaTimezoneId,text,geocacheLogType[id]]";

    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn discover(&self, tile: &Tile) -> Result<GcCodes, Error> {
        debug!("Discovering {}", tile);

        let base_url = format!(
            "https://tiles0{}.geocaching.com",
            rand::thread_rng().gen_range(1..5)
        );
        let image_url = std::format!(
            "{}/map.png?x={}&y={}&z={}",
            base_url,
            tile.x,
            tile.y,
            tile.z,
        );
        let info_url = std::format!(
            "{}/map.info?x={}&y={}&z={}",
            base_url,
            tile.x,
            tile.y,
            tile.z,
        );

        self.client
            .get(image_url)
            .header(reqwest::header::USER_AGENT, Self::USER_AGENT)
            .header(reqwest::header::ACCEPT, "*/*")
            .send()
            .await?;

        let response = self
            .client
            .get(info_url)
            .header(reqwest::header::USER_AGENT, Self::USER_AGENT)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        sleep(Duration::from_secs(1)).await;

        debug!("tile response {:#?}", response);
        if response.status() == 204 {
            info!("Discover {} -> 0", tile);
            return Ok(vec![]);
        }
        let grid = response.json::<UtfGrid>().await?;
        let codes = grid.parse(&tile).await?;

        Ok(codes)
    }

    pub async fn fetch(&self, token: &str, codes: Vec<&String>) -> Result<Vec<serde_json::Value>, Error> {
        if codes.len() > BATCH_SIZE {
            return Err(Error::Unknown);
        }
        debug!("fetch chunk {}", codes.len());
        let codes_str: Vec<&str> = codes.iter().map(|x| x.as_str()).collect();
        let comma_separated_codes = codes_str.join(",");
        let response = self
            .client
            .get(Groundspeak::FETCH_URL)
            .header(reqwest::header::ACCEPT, "*/*")
            .header(reqwest::header::ACCEPT_LANGUAGE, "en-US;q=1")
            .header(reqwest::header::USER_AGENT, Groundspeak::USER_AGENT_FETCH)
            .bearer_auth(token)
            .query(&[("referenceCodes", comma_separated_codes), ("lite", "true".to_string()), ("fields", Self::FETCH_FIELDS.to_string()), ("expand", Self::EXPAND_FIELDS.to_string())])
            .send()
            .await?;
        debug!("fetch status {}", response.status().as_str());
        let json: serde_json::Value = serde_json::from_slice(&response.bytes().await?)?;
        debug!("fetch json {:#?}", json);

        sleep(Duration::from_secs(1)).await;

        let geocaches = json.as_array().ok_or(Error::JsonRaw)?.clone();
        debug!("fetch geocaches {}", geocaches.len());

        Ok(geocaches)
    }
}

pub fn parse(v: &serde_json::Value) -> Result<Geocache, Error> {
    debug!("parsing geocache");
    // this is pretty ugly, but more advanced serde scared me more
    let code = String::from(v["referenceCode"].as_str().ok_or(Error::JsonRaw)?);
    debug!("Parse geocache {}", code);
    let is_premium = v["isPremiumOnly"].as_bool().unwrap_or(false);

    if is_premium {
        return Ok(Geocache::premium(code));
    }

    let name = String::from(v["name"].as_str().ok_or(Error::JsonRaw)?);
    let terrain = v["terrain"].as_f64().ok_or(Error::JsonRaw)? as f32;
    let difficulty = v["difficulty"].as_f64().ok_or(Error::JsonRaw)? as f32;
    let lat = v["postedCoordinates"]["latitude"].as_f64().ok_or(Error::JsonRaw)?;
    let lon = v["postedCoordinates"]["longitude"].as_f64().ok_or(Error::JsonRaw)?;
    /* not availble for lite=true
    let short_description = String::from(v["shortDescription"].as_str().ok_or(Error::JsonRaw)?);
    let long_description = String::from(v["longDescription"].as_str().ok_or(Error::JsonRaw)?);
    let encoded_hints = String::from(v["hints"].as_str().ok_or(Error::JsonRaw)?);
     */
    let short_description = String::new();
    let long_description = String::new();
    let encoded_hints = String::new();

    let size = ContainerSize::from(
        v["geocacheSize"]["id"]
            .as_u64()
            .ok_or(Error::JsonRaw)?,
    );
    let cache_type = CacheType::from(
        v["geocacheType"]["id"]
            .as_u64()
            .ok_or(Error::JsonRaw)?,
    );
    let available = v["status"].as_str().ok_or(Error::JsonRaw)? == "Active";
    // TODO archived?
    let archived = false; //v["Archived"].as_bool().ok_or(Error::JsonRaw)?;
    // not available for lite=true
    // let logs = v["geocacheLogs"].as_array().ok_or(Error::JsonRaw)?.iter().map(parse_geocache_log).collect::<Result<Vec<GeocacheLog>, Error>>()?;
    let logs = vec![];

    Ok(Geocache {
        code,
        name,
        is_premium,
        terrain,
        difficulty,
        coord: Coordinate { lat, lon },
        short_description,
        long_description,
        encoded_hints,
        size,
        cache_type,
        archived,
        available,
        logs,
    })
}

fn parse_geocache_log(v: &serde_json::Value) -> Result<GeocacheLog, Error> {
    let date = v["loggedDate"].as_str().ok_or(Error::JsonRaw)?;
    let tz = v["ianaTimezoneId"].as_str().ok_or(Error::JsonRaw)?;
    let text = v["text"].as_str().ok_or(Error::JsonRaw)?;
    let log_type = v["geocacheLogType"]["id"].as_u64().ok_or(Error::JsonRaw)?;

    let naive_date = NaiveDateTime::parse_from_str(date, "%Y-%m-%dT%H:%M:%S%.f")?;
    let tz: Tz = tz.parse()?;
    let date = tz.from_utc_datetime(&naive_date);

    Ok(GeocacheLog {
        text: text.to_string(),
        log_type: LogType::from(log_type),
        timestamp: date.to_rfc3339(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_foo() {
        let uut = Groundspeak::new();
        let tile = Tile::from_coordinates(51.34469577842422, 12.374765732990399, 12);
        uut.discover(&tile).await.unwrap();
    }

    #[tokio::test]
    async fn test_parse() {
        let text: &'static str = "{\"name\": \"Berg auf Berg ab (oder Jula's Geburtstagscache)\", \"hints\": \"Magnetisch, der Herr wird den Weg schon weisen.\", \"status\": \"Active\", \"terrain\": 2.5, \"difficulty\": 2.0, \"placedDate\": \"2012-10-02T00:00:00.000\", \"geocacheLogs\": [{\"text\": \"Ist dieser Cache überhaupt noch da? Seit 2021 nicht mehr gefunden.\", \"loggedDate\": \"2023-10-05T12:00:00.000\", \"ianaTimezoneId\": \"Europe/Berlin\", \"geocacheLogType\": {\"id\": 3}}, {\"text\": \"Na mehrfachen suchen und erfolglosem Kontakt zum Owner geb ich auch und logge einen DNF\", \"loggedDate\": \"2021-05-29T16:27:27.000\", \"ianaTimezoneId\": \"Europe/Berlin\", \"geocacheLogType\": {\"id\": 3}}, {\"text\": \"Die Daten waren schnell eingesammelt und so ging es zügig zum Final.Danke sagen Sonny&Harry\", \"loggedDate\": \"2021-05-16T12:00:00.000\", \"ianaTimezoneId\": \"Europe/Berlin\", \"geocacheLogType\": {\"id\": 2}}, {\"text\": \"Alle Stationen konnten gut gefunden werden.Irgendwo haben wir uns dann noch ins Logbuch reingequetscht.DFDC sagtTeam Rudi\", \"loggedDate\": \"2021-01-28T12:00:00.000\", \"ianaTimezoneId\": \"Europe/Berlin\", \"geocacheLogType\": {\"id\": 2}}, {\"text\": \"Für heute hatte ich mir ein paar Caches in VS und im Brigachtal rausgesucht.Nachdem ich am Magdalenenberg unterwegs war, ging es nach Grüningen.Diesen Cache konnte ich finden und mich noch irgendwo ins volle Logbuch reinzwängen.Danke fürs Legen und Herführen. TFTC\", \"loggedDate\": \"2020-05-23T12:00:00.000\", \"ianaTimezoneId\": \"Europe/Berlin\", \"geocacheLogType\": {\"id\": 2}}], \"geocacheSize\": {\"id\": 2, \"name\": \"Micro\"}, \"geocacheType\": {\"id\": 3, \"name\": \"Multi-Cache\", \"imageUrl\": \"https://www.geocaching.com/images/wpttypes/3.gif\"}, \"isPremiumOnly\": false, \"referenceCode\": \"GC3Y133\", \"favoritePoints\": 0, \"lastVisitedDate\": \"2021-05-16T12:00:00.000\", \"longDescription\": \"An diesem Berg bin ich aufgewachsen und musste ihn Tag ein und aus hoch und runter laufen, wobei hoch laufen deutlich anstrengender war und auch heute noch ist.Am Ausgangspunkt (nicht der empfohlene Parkplatz) angekommen musst Du auf ca. ABC Grad peilen und dann geht's auch schon los. Der Weg ist nicht weit und Du musst keinesfalls die grosse Strasse überschreiten um den Nano zu finden.A= Hausnummer (Eckhaus mit 3 Stromverteiler davor) -1B= Hausnummer (Eckhaus mit 3 Stromverteiler davor) *2C= Hausnummer (Eckhaus mit 3 Stromverteiler davor) +1\", \"shortDescription\": \"Ein kurzes Rätsel zu Jula's Geburtstag ;-)\", \"postedCoordinates\": {\"latitude\": 47.9842, \"longitude\": 8.4743}, \"additionalWaypoints\": [{\"url\": \"https://geocaching.com/seek/wpt.aspx?WID=de51dd1b-394b-42ee-b15d-0e3735ea6280\", \"name\": \"Empfohlener Parkplatz\", \"prefix\": \"00\", \"typeId\": 217, \"typeName\": \"Parking Area\", \"coordinates\": {\"latitude\": 47.9841, \"longitude\": 8.473}, \"description\": \"Bitte hier parken um die Aufmerksamkeit der Anwohner zu reduzieren.\", \"referenceCode\": \"WP003Y133\", \"visibilityTypeId\": 0}, {\"url\": \"https://geocaching.com/seek/wpt.aspx?WID=75db04aa-65e7-4194-854e-05c92a5f358a\", \"name\": \"Stage 1\", \"prefix\": \"01\", \"typeId\": 452, \"typeName\": \"Reference Point\", \"coordinates\": {\"latitude\": 47.9842, \"longitude\": 8.4743}, \"description\": \"Startpunkt von wo aus die Peilung vorgenommen werden muss. Der Startpunkt ist die Kreuzung.\", \"referenceCode\": \"WP013Y133\", \"visibilityTypeId\": 0}]}";
        println!("{}", text);
        let json: serde_json::Value = serde_json::from_str(text).unwrap();
        let geocache = parse(&json).unwrap();
        assert_eq!(geocache.code, "GC3Y133");
    }
}
