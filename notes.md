# re-design of download queue

## Use Case 1: Find quick geocaches near track

In REST handler:

1. upload GPX track
2. store GPX track
3. resolve track into tiles
4. store everything in job table
5. wakeup background thread

In Background thread:

1. check jobs table, find stale job
   this should be either new or processing - processing means the job was interrupted and we need to redo it
2. if gccodes is null, then resolve tiles into gc codes and apply inclusion area filter
3. fetch all gc codes
4. done

After a while user comes back into REST handler:

1. List all jobs or query individual by id
2. If done, can compute gpx

jobs table

| id  | state      | message               | tiles         | inclusion area                | gccodes    |
| --- | ---------- | --------------------- | ------------- | ----------------------------- | ---------- |
| 1   | new        | no started yet...     | xzy, xzy, ... | geojson of track + 100 m hull | null       |
| 2   | processing | discovering tile 5/10 | ...           | geojson                       | ...        |
| 3   | done       | took 5s               | ...           | geojson                       | GC123, ... |

## Use Case 2: Find all geocaches in an area

In REST Handler:

1. upload boundary
2. resolve boundary into tiles
3. inclusion area is either null or just the same boundary
4. rest as above

## Open question

How to filter coordinate to see if it's included in geojson -> should be a library method

How to form a hull around a track?!
