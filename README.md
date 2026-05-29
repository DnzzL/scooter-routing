# ScooterRoute 🛵

**Routeur pour scooter 50cc et voiture sans permis.** Évite automatiquement les autoroutes et voies rapides (type Voie Mathis à Nice).

Propulsé par **Rust** + **OpenStreetMap** + **A\***.

[→ scooter.legrand.sh](https://scooter.legrand.sh)

---

## Comment ça marche

1. **Données OSM** — extract de la région PACA (~4M nœuds, 7.7M arêtes)
2. **Graphe routier** — construit en Rust, sérialisé en Postcard (153MB, chargé en mémoire)
3. **A\*** — recherche de chemin avec heuristique adaptée aux véhicules lents
4. **API HTTP** — Axum, endpoint unique `/api/route`
5. **Frontend** — Leaflet + Photon autocomplete + overlay restrictions Overpass

### Blocage des voies rapides

En France, les voies rapides (Voie Pierre Mathis M6210, etc.) sont taguées `highway=trunk` avec `foot=no` et `bicycle=no`, mais rarement avec `motorroad=yes`. Le routeur détecte ces routes par heuristique :

- `highway=trunk` **+** `foot=no` **+** `bicycle=no` → bloqué ✅
- `highway=trunk` **+** pont/viaduc → bloqué ✅
- Via le tag `motorroad=yes` (quand présent) → bloqué ✅

⚠️ **Ne bloque pas les simple `highway=trunk` sans restriction piéton/vélo** (ex: certaines portions de la Promenade des Anglais sont taguées trunk mais sont autorisées aux 50cc)

## Profils

| Profil | Vitesse max | Autoroute | Voie rapide |
|--------|-------------|-----------|-------------|
| Scooter 50cc | 45 km/h | 🔴 Bloqué | 🔴 Bloqué |
| Voiture sans permis | 70 km/h | 🔴 Bloqué | 🔴 Bloqué |

Les deux profils ont les mêmes restrictions (pas d'autoroute, pas de voie rapide) — ils diffèrent uniquement par la vitesse max, ce qui impacte le temps de parcours.

## API

```bash
# Santé
curl https://scooter.legrand.sh/api/health

# Itinéraire scooter
curl "https://scooter.legrand.sh/api/route?from_lat=43.703&from_lon=7.266&to_lat=43.695&to_lon=7.273&profile=scooter"

# Itinéraire voiturette
curl "https://scooter.legrand.sh/api/route?from_lat=43.703&from_lon=7.266&to_lat=43.695&to_lon=7.273&profile=voiturette"

# Overlay restrictions (bbox)
curl "https://scooter.legrand.sh/api/restrictions?south=43.68&west=7.20&north=43.72&east=7.30"
```

### Réponse

```json
{
  "found": true,
  "distance_km": 2.0,
  "duration_min": 3,
  "max_speed_kmh": 45.0,
  "profile": "scooter",
  "path": [[7.2662, 43.7031], ...]
}
```

## Stack

| Couche | Technologie |
|--------|-------------|
| **Backend** | Rust, Axum, osmpbf, rstar, postcard |
| **Frontend** | Leaflet, Photon (autocomplete), Overpass API |
| **Routeur** | A\* custom (pas de wrapper) |
| **Données** | OpenStreetMap PACA |
| **Déploiement** | Docker multi-stage, Dokploy |
| **Design** | DM Sans · Palette Mapbox · Signal Blue #007afc |

## Développement

```bash
# Compiler
cargo build --release

# Importer les données OSM (nécessite un .osm.pbf)
cargo run --release import region.osm.pbf --output region.graph

# Lancer le serveur
cargo run --release serve --graph region.graph --bind 0.0.0.0:3000
```

### Données OSM

Télécharger un extract :
```bash
curl -L -o region.osm.pbf "https://download.geofabrik.de/europe/france/provence-alpes-cote-d-azur-latest.osm.pbf"
```

## Licence

MIT — fait avec 🦀 par [@DnzzL](https://github.com/DnzzL)
