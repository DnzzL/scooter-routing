use crate::graph::{IdxPoint, RoadGraph};
use crate::profile::{Profile, VehicleType};
use crate::routing;
use axum::{
    extract::{Query, State},
    http::{HeaderValue, header},
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

#[derive(Deserialize)]
pub struct RestrictionsQuery {
    south: f64,
    west: f64,
    north: f64,
    east: f64,
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

async fn handle_restrictions(
    Query(query): Query<RestrictionsQuery>,
) -> Json<serde_json::Value> {
    let bbox = format!("{},{},{},{}", query.south, query.west, query.north, query.east);
    let query_str = format!(
        "[out:json][timeout:15];(\
        way[\"highway\"=\"motorway\"]({bbox});\
        way[\"highway\"=\"motorway_link\"]({bbox});\
        way[\"motorroad\"=\"yes\"]({bbox});\
        way[\"highway\"=\"trunk\"]({bbox});\
        way[\"highway\"=\"trunk_link\"]({bbox});\
        way[\"highway\"~\"primary|secondary|tertiary|residential|service|unclassified|living_street\"]({bbox});\
        );out body;>;out skel qt;"
    );

    // Use curl subprocess to avoid adding HTTP deps
    let body = format!("data={}", url_encode(&query_str));
    let output = tokio::process::Command::new("curl")
        .args(["-s", "--max-time", "20", "-X", "POST",
               "https://overpass-api.de/api/interpreter",
               "-H", "Content-Type: application/x-www-form-urlencoded",
               "--data-binary", &body])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            match serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                Ok(json) => Json(json),
                Err(e) => Json(serde_json::json!({"error": format!("parse: {e}")})),
            }
        }
        Ok(out) => Json(serde_json::json!({
            "error": format!("Overpass curl failed: {}", String::from_utf8_lossy(&out.stderr))
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("Overpass proxy error: {}", e)
        })),
    }
}

fn url_encode(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '.' | '-' | '~' => c.to_string(),
        ' ' => "+".to_string(),
        ':' | ';' | ',' | '(' | ')' | '!' | '?' | '=' | '>' | '<' | '|' | '/' | '\\' | '[' | ']' | '{' | '}' => {
            format!("%{:02X}", c as u8)
        }
        c => format!("%{:02X}", c as u8),
    }).collect()
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

async fn handle_static(path: axum::extract::Path<String>) -> impl IntoResponse {
    let mime = |ext: &str| -> &str {
        match ext {
            "svg" => "image/svg+xml",
            "json" => "application/manifest+json",
            "js" => "application/javascript",
            "css" => "text/css",
            "png" => "image/png",
            _ => "application/octet-stream",
        }
    };
    let path = path.0.trim_start_matches('/');
    let ext = path.rsplit('.').next().unwrap_or("");
    let static_dir = std::path::Path::new("static");
    // Also check /data/static for Docker deployments
    let data_dir = std::path::Path::new("/data/static");
    let content = std::fs::read(static_dir.join(&path))
        .or_else(|_| std::fs::read(data_dir.join(&path)));
    match content {
        Ok(bytes) => ([(header::CONTENT_TYPE, mime(ext))], bytes).into_response(),
        Err(_) => (axum::http::StatusCode::NOT_FOUND, "404").into_response(),
    }
}

pub async fn serve(graph: RoadGraph, bind: &str) {
    let spatial = graph.build_spatial_index();
    let state = Arc::new(AppState { graph, spatial });

    let app = Router::new()
        .route("/", get(handle_index))
        .route("/{*path}", get(handle_static))
        .route("/api/route", get(handle_route))
        .route("/api/restrictions", get(handle_restrictions))
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
