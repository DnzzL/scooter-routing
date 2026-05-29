use crate::graph::{IdxPoint, RoadGraph};
use crate::profile::{Profile, VehicleType};
use crate::routing;
use axum::{
    extract::{Query, State},
    http::HeaderValue,
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub struct AppState {
    pub graph: RoadGraph,
    pub spatial: rstar::RTree<IdxPoint>,
}

#[derive(Deserialize)]
pub struct RouteQuery {
    from_lat: f64,
    from_lon: f64,
    to_lat: f64,
    to_lon: f64,
    #[serde(default = "default_profile")]
    profile: String,
}

fn default_profile() -> String {
    "scooter".into()
}

async fn handle_route(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RouteQuery>,
) -> Json<serde_json::Value> {
    let profile = match query.profile.as_str() {
        "voiturette" => Profile::for_vehicle(VehicleType::Voiturette),
        _ => Profile::for_vehicle(VehicleType::Scooter50),
    };

    let from_idx = match state
        .graph
        .nearest_node(&state.spatial, query.from_lat, query.from_lon, 500.0)
    {
        Some(idx) => idx,
        None => {
            return Json(serde_json::json!({
                "found": false,
                "error": "No road near departure"
            }));
        }
    };

    let to_idx = match state
        .graph
        .nearest_node(&state.spatial, query.to_lat, query.to_lon, 500.0)
    {
        Some(idx) => idx,
        None => {
            return Json(serde_json::json!({
                "found": false,
                "error": "No road near arrival"
            }));
        }
    };

    match routing::route(&state.graph, from_idx, to_idx, &profile, &state.spatial) {
        Some(result) => Json(serde_json::json!({
            "found": true,
            "distance_km": result.distance_km,
            "duration_min": result.duration_min,
            "max_speed_kmh": profile.max_speed_kmh,
            "profile": profile.name,
            "path": result.path,
        })),
        None => Json(serde_json::json!({
            "found": false,
            "error": "No route found (roads may be blocked)"
        })),
    }
}

async fn handle_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn handle_index() -> impl IntoResponse {
    Html(include_str!("../static/index.html"))
}

pub async fn serve(graph: RoadGraph, bind: &str) {
    let spatial = graph.build_spatial_index();
    let state = Arc::new(AppState { graph, spatial });

    let app = Router::new()
        .route("/", get(handle_index))
        .route("/api/route", get(handle_route))
        .route("/api/health", get(handle_health))
        .layer(
            CorsLayer::new()
                .allow_origin("*".parse::<HeaderValue>().unwrap())
                .allow_methods([axum::http::Method::GET]),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .expect("bind failed");

    tracing::info!("Listening on http://{bind}");
    tracing::info!("→ Frontend: http://{bind}/");
    tracing::info!("→ API: http://{bind}/api/route?from_lat=43.71&from_lon=7.26&to_lat=43.70&to_lon=7.29&profile=scooter");
    tracing::info!("→ Health: http://{bind}/api/health");

    axum::serve(listener, app).await.expect("serve failed");
}
