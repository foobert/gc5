# Groundspeak API

Groundspeak recently turned off the trusty old V6 API. This document describes there current API.
The information was gathered by capturing traffic of one of the official apps using [mitmproxy](https://mitmproxy.org/).

## Authentication

### Refresh Token

Assuming you already have a working refresh token, you can get a new access token:

```shell
curl \
  -H 'Content-Type: application/x-www-form-urlencoded; charset=UTF-8' \
  -H "User-Agent: ${AUTH_USERAGENT}" \
  -H 'Connection: keep-alive' \
  -H 'Accept: */*' \
  -H 'Accept-Language: en-us' \
  --compressed \
  -u "${AUTH_USERNAME}:${AUTH_PASSWORD}" \
  -X POST https://oauth.geocaching.com/token \
  -d "redirect_uri=${AUTH_REDIRECT_URL}&refresh_token=${REFRESH_TOKEN}&grant_type=refresh_token"
```

Beware that you will also get a new refresh token and the old one will be invalidated. The response is typical token
payload, valid for one hour:

```json
{
  "access_token": "...",
  "expires_in": 3599,
  "refresh_token": "...",
  "token_type": "bearer"
}
```

### Initial Token Request

For reference, here's how the app gets the initial token:

```shell
curl \
  -H 'Content-Type: application/x-www-form-urlencoded; charset=UTF-8' \
  -H "User-Agent: ${AUTH_USERAGENT}" \
  -H 'Connection: keep-alive' \
  -H 'Accept: */*' \
  -H 'Accept-Language: en-us' \
  --compressed \
  -u "${AUTH_USERNAME}:${AUTH_PASSWORD}" \
  -X POST https://oauth.geocaching.com/token \
  -d 'code=${AUTH_CODE}&code_verifier=${AUTH_VERIFIER}&redirect_uri=${AUTH_REDIRECT_URL}&grant_type=authorization_code'
```

## Search

Then we can search for geocaches!

```shell
curl \
  -H 'Connection: keep-alive' \
  -H 'Accept: */*' \
  -w "User-Agent: ${USERAGENT}" \
  -H 'Accept-Language: en-US' \
  -H 'Authorization: bearer ${ACCESS_TOKEN}' \
  --compressed \
  'https://api.groundspeak.com/v1.0/geocaches?referenceCodes=GC...,GC...&lite=true&fields=referenceCode,ianaTimezoneId,name,postedCoordinates,geocacheType,geocacheSize,difficulty,terrain,userData,favoritePoints,placedDate,eventEndDate,ownerAlias,owner,isPremiumOnly,userData,lastVisitedDate,status,hasSolutionChecker'
```

Note that `lite=true` will limit the fields you can query, for example `hints` will not be available. But you can just
set `lite=false`.

The [full API description](groundspeak-openapi.json) is available as OpenAPI 2.0. There's also
an [online Swagger UI](https://api.groundspeak.com/api-docs/index) available.

Here's also an example response:

```json
[
  {
    "referenceCode": "GC8VPVW",
    "ianaTimezoneId": "Europe/Berlin",
    "name": "Kleine Pause",
    "postedCoordinates": {
      "latitude": 47.959967,
      "longitude": 8.493167
    },
    "geocacheType": {
      "id": 2,
      "name": "Traditional",
      "imageUrl": "https://www.geocaching.com/images/wpttypes/2.gif"
    },
    "geocacheSize": {
      "id": 2,
      "name": "Micro"
    },
    "difficulty": 1.5,
    "terrain": 1.5,
    "userData": {
      "isFavorited": false,
      "watched": false,
      "ignored": false
    },
    "favoritePoints": 0,
    "placedDate": "2020-06-27T00:00:00.000",
    "eventEndDate": null,
    "ownerAlias": "DJNL2020",
    "owner": {
      "membershipLevelId": 3,
      "joinedDateUtc": "2020-03-29T01:15:18.000",
      "findCount": 1049,
      "hideCount": 35,
      "favoritePoints": 0,
      "awardedFavoritePoints": 0,
      "profileText": "",
      "bannerUrl": "https://www.geocaching.com/account/app/ui-images/components/profile/p_bgimage-large.png",
      "url": "https://coord.info/PR1105PY",
      "isFriend": false,
      "optedInFriendSharing": false,
      "allowsFriendRequests": false,
      "souvenirCount": 0,
      "trackableFindCount": 0,
      "trackableOwnedCount": 0,
      "referenceCode": "PR1105PY",
      "username": "DJNL2020",
      "avatarUrl": "https://img.geocaching.com/large/394116d2-f15c-4faf-bb47-ce1e75736d18.jpeg"
    },
    "isPremiumOnly": false,
    "lastVisitedDate": "2024-03-22T12:00:00.000",
    "status": "Active",
    "hasSolutionChecker": false
  },
  {
    "referenceCode": "GC4BA44",
    "ianaTimezoneId": "Europe/Berlin",
    "name": "Nasse Socken am Mühlentor",
    "postedCoordinates": {
      "latitude": 47.930217,
      "longitude": 8.44925
    },
    "geocacheType": {
      "id": 3,
      "name": "Multi-Cache",
      "imageUrl": "https://www.geocaching.com/images/wpttypes/3.gif"
    },
    "geocacheSize": {
      "id": 2,
      "name": "Micro"
    },
    "difficulty": 2.0,
    "terrain": 1.5,
    "userData": {
      "isFavorited": false,
      "watched": false,
      "ignored": false
    },
    "favoritePoints": 5,
    "placedDate": "2013-05-04T00:00:00.000",
    "eventEndDate": null,
    "ownerAlias": "Edgar Briggs",
    "owner": {
      "membershipLevelId": 3,
      "joinedDateUtc": "2007-05-13T07:48:50.000",
      "findCount": 31207,
      "hideCount": 61,
      "favoritePoints": 0,
      "awardedFavoritePoints": 0,
      "bannerUrl": "https://img.geocaching.com/user/0dd5635f-d6fa-45df-a139-8767f3b5328a.jpg",
      "url": "https://coord.info/PR1KC4J",
      "isFriend": false,
      "optedInFriendSharing": false,
      "allowsFriendRequests": false,
      "souvenirCount": 0,
      "trackableFindCount": 0,
      "trackableOwnedCount": 0,
      "referenceCode": "PR1KC4J",
      "username": "Edgar Briggs",
      "avatarUrl": "https://img.geocaching.com/large/5e737651-e199-4f94-ba86-de929be01a92.jpg"
    },
    "isPremiumOnly": false,
    "lastVisitedDate": "2024-03-08T12:00:00.000",
    "status": "Active",
    "hasSolutionChecker": false
  }
]
```