#!/bin/bash
set -e
GRAPH="${GRAPH_FILE:-/data/region.graph}"
PBF="${OSM_PBF:-/data/region.osm.pbf}"

if [ ! -f "$GRAPH" ]; then
    if [ ! -f "$PBF" ]; then
        echo "Downloading OSM PACA..."
        curl -sL -o "$PBF" "https://download.geofabrik.de/europe/france/provence-alpes-cote-d-azur-latest.osm.pbf"
    fi
    echo "Building road graph..."
    scooter-routing import --output "$GRAPH" "$PBF"
fi

if [ "$1" = "serve" ]; then
    exec scooter-routing serve --graph "$GRAPH" --bind "0.0.0.0:3000"
else
    exec scooter-routing "$@"
fi
