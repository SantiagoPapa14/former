mod model;
use axum::{
    Json, Router,
    extract::{ConnectInfo, State},
    http::{HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use model::Submission;
use sqlx::{Pool, Postgres, postgres::PgPool};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Instant};
use tokio::{net::TcpListener, sync::Mutex};
use tower_http::cors::{Any, CorsLayer};

type RateMap = Arc<Mutex<HashMap<String, (u32, Instant)>>>;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to DB");

    init_tables(&pool).await;

    let rate_map: RateMap = Arc::new(Mutex::new(HashMap::new()));

    let cors = CorsLayer::new()
        .allow_origin("https://semantic.com.ar".parse::<HeaderValue>().unwrap())
        .allow_origin("https://ombufinanzas.com".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/semantic", post(create_message))
        .route("/ombu", post(create_message))
        .route("/submissions", get(get_submissions))
        .layer(cors)
        .with_state((pool, rate_map))
        .into_make_service_with_connect_info::<SocketAddr>();

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn init_tables(pool: &Pool<Postgres>) {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS submission (
            id SERIAL PRIMARY KEY,
            date TIMESTAMP,
            page TEXT,
            name TEXT,
            email TEXT,
            message TEXT
        )",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn create_message(
    State((pool, rate_map)): State<(Pool<Postgres>, RateMap)>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<Submission>,
) -> impl IntoResponse {
    let mut map = rate_map.lock().await;
    let entry = map
        .entry(addr.ip().to_string())
        .or_insert((0, Instant::now()));

    if entry.1.elapsed().as_secs() > 60 {
        *entry = (0, Instant::now());
    }
    entry.0 += 1;
    if entry.0 > 10 {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    drop(map);

    // PostgreSQL uses $1, $2, ... placeholders instead of ?1, ?2, ...
    sqlx::query(
        "INSERT INTO submission (date, page, name, email, message) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind("semantic")
    .bind(payload.name)
    .bind(payload.email)
    .bind(payload.message)
    .execute(&pool)
    .await
    .unwrap();

    StatusCode::CREATED.into_response()
}

async fn get_submissions(
    State((pool, _)): State<(Pool<Postgres>, RateMap)>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    if !addr.ip().is_loopback() && !matches!(addr.ip(), std::net::IpAddr::V4(ip) if ip.is_private())
    {
        return StatusCode::FORBIDDEN.into_response();
    }

    let rows = sqlx::query_as::<_, Submission>("SELECT * FROM submission")
        .fetch_all(&pool)
        .await
        .unwrap();

    let html = rows.iter().fold(
        String::from(
            "<html><body><table><tr><th>Date</th><th>Name</th><th>Email</th><th>Message</th></tr>",
        ),
        |mut acc, r| {
            acc.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                r.date.as_deref().unwrap_or(""),
                r.name,
                r.email,
                r.message
            ));
            acc
        },
    );

    axum::response::Html(html + "</table></body></html>").into_response()
}
