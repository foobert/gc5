# UtfGrid

The [old geocaching map](https://www.geocaching.com/map/default.aspx) relies on a technology called UtfGrid to display
information about geocaches when you hover over them. UtfGrid is a way to encode data in a grid of UTF-8 characters.
Each character in the grid corresponds to an area on the map. When you hover over a tile, the character at that position
in the grid is used to look up information about the feature at that location.

It's rather old, but there's [a specification](https://github.com/mapbox/utfgrid-spec/tree/master) on Github.

Now, it would be weird if Groundspeak followed that spec, instead they have a slight variation: The grid are effectively
ignored and instead data is indexed using pairs of row and column numbers. This is nice, because we can just ignore the
grid altogether and just iterate over all keys.

We approximate the geocache then to be in the center of the grid (typically 6x6) and offset from the top-left
coordinate.