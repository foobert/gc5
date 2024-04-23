# Web API

Next to the [new API]](new-api.md) there is also a web API available. This API is apparently used by the geocaching
website itself, for example the [map interface](https://www.geocaching.com/play/map).

A search for geocaches around a given coordinate looks like this:

```bash
curl \
  'https://www.geocaching.com/api/proxy/web/search/v2?skip=0&take=500&asc=true&sort=distance&properties=callernote&origin=51.508502%2C-0.120206&rad=16000' \
  -v \
  --compressed \
  -H 'User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:124.0) Gecko/20100101 Firefox/124.0' \
  -H 'Accept: application/json' \
  -H 'Accept-Language: en-US,en;q=0.5' \
  -H 'Accept-Encoding: gzip, deflate, br' \
  -H 'Referer: https://www.geocaching.com/play/map' \
  -H 'Content-Type: application/json' \
  -H 'Connection: keep-alive' \
  -H 'Cookie: gspkauth=...;' \
  -H 'Sec-Fetch-Dest: empty' \
  -H 'Sec-Fetch-Mode: cors' \
  -H 'Sec-Fetch-Site: same-origin'
```

Non-relevant cookies have been omitted.

## Authentication

The API requires the `gspkauth` cookie to be set. It's not clear to me what the value of this cookie is, but it seems to
be a session token. Interestingly, a request using the `gspkauth` cookie will return a `set-cookie` response with
a `jwt` cookie. This cookie can be used in subsequent requests instead of `gspkauth`.

And the best thing is: We know how to get the JWT token! It's the same as the one we get from the [new API](new-api.md).

Thus, the request looks like:

```bash
curl \
  'https://www.geocaching.com/api/proxy/web/search/v2?skip=0&take=500&asc=true&sort=distance&properties=callernote&origin=51.508502%2C-0.120206&rad=16000' \
  -v \
  --compressed \
  -H 'User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:124.0) Gecko/20100101 Firefox/124.0' \
  -H 'Accept: application/json' \
  -H 'Accept-Language: en-US,en;q=0.5' \
  -H 'Accept-Encoding: gzip, deflate, br' \
  -H 'Referer: https://www.geocaching.com/play/map' \
  -H 'Content-Type: application/json' \
  -H 'Connection: keep-alive' \
  -H 'Cookie: jwt=...;' \
  -H 'Sec-Fetch-Dest: empty' \
  -H 'Sec-Fetch-Mode: cors' \
  -H 'Sec-Fetch-Site: same-origin'
```

The response looks similar to this:

```json
{
  "results": [
    {
      "id": 4484883,
      "name": "Cleopatra's Needle - London",
      "code": "GC59ANJ",
      "premiumOnly": false,
      "favoritePoints": 322,
      "geocacheType": 137,
      "containerType": 6,
      "difficulty": 1.5,
      "terrain": 1,
      "cacheStatus": 0,
      "postedCoordinates": {
        "latitude": 51.508467,
        "longitude": -0.12035
      },
      "detailsUrl": "/geocache/GC59ANJ",
      "hasGeotour": false,
      "placedDate": "2014-07-21T00:00:00",
      "owner": {
        "code": "PR61VHZ",
        "username": "Heidi Seekers"
      },
      "lastFoundDate": "2024-04-10T12:00:00",
      "trackableCount": 0,
      "region": "London",
      "country": "United Kingdom",
      "attributes": [
        {
          "id": 24,
          "name": "Wheelchair accessible",
          "isApplicable": true
        },
        {
          "id": 8,
          "name": "Scenic view",
          "isApplicable": true
        },
        {
          "id": 13,
          "name": "Available 24/7",
          "isApplicable": true
        },
        {
          "id": 63,
          "name": "Recommended for tourists",
          "isApplicable": true
        },
        {
          "id": 28,
          "name": "Public restrooms nearby",
          "isApplicable": true
        },
        {
          "id": 59,
          "name": "Food nearby",
          "isApplicable": true
        },
        {
          "id": 29,
          "name": "Telephone nearby",
          "isApplicable": true
        }
      ],
      "distance": "35ft",
      "bearing": "W"
    }
  ],
  "total": 176
}
```
