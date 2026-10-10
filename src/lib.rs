use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{any, get, post, put},
};
use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use uuid::Uuid;

const CACHE_SECONDS: i64 = 3600;
mod climate;

#[derive(Clone)]
pub struct AppState {
    db: Arc<Mutex<Connection>>,
    mountains: Arc<Vec<Mountain>>,
    client: reqwest::Client,
    weather_locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Mountain {
    pub id: String,
    pub name: String,
    pub state: String,
    pub region: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
    pub image: String,
}

#[derive(Debug)]
struct ApiError(StatusCode, String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}
impl From<rusqlite::Error> for ApiError {
    fn from(error: rusqlite::Error) -> Self {
        tracing::error!(%error, "database operation failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not save your changes. Please try again.".into(),
        )
    }
}
impl From<serde_json::Error> for ApiError {
    fn from(error: serde_json::Error) -> Self {
        tracing::error!(%error, "stored data could not be read");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not read this planner.".into(),
        )
    }
}
type ApiResult<T> = Result<Json<T>, ApiError>;
fn invalid(message: &str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, message.into())
}
fn token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}
fn bearer(headers: &HeaderMap) -> &str {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or("")
}
pub fn today() -> NaiveDate {
    Utc::now()
        .with_timezone(&chrono_tz::America::Los_Angeles)
        .date_naive()
}

impl AppState {
    pub fn new(database: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let db = Connection::open(database)?;
        db.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS plans (
                id TEXT PRIMARY KEY, title TEXT NOT NULL, owner TEXT NOT NULL,
                owner_token TEXT NOT NULL, mountains TEXT NOT NULL, weekends TEXT NOT NULL,
                created_at TEXT NOT NULL, trip_options TEXT NOT NULL DEFAULT '[]');
            CREATE TABLE IF NOT EXISTS members (
                id TEXT PRIMARY KEY, plan_id TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
                username TEXT NOT NULL, username_key TEXT NOT NULL, token TEXT NOT NULL,
                trips TEXT NOT NULL, UNIQUE(plan_id, username_key));
            CREATE TABLE IF NOT EXISTS weather_cache (
                mountain_id TEXT PRIMARY KEY, fetched_at INTEGER NOT NULL, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS climate_cache (
                mountain_id TEXT PRIMARY KEY, fetched_at INTEGER NOT NULL, data TEXT NOT NULL);",
        )?;
        let has_trip_options = {
            let mut query = db.prepare("PRAGMA table_info(plans)")?;
            query
                .query_map([], |r| r.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .any(|c| c == "trip_options")
        };
        if !has_trip_options {
            db.execute(
                "ALTER TABLE plans ADD COLUMN trip_options TEXT NOT NULL DEFAULT '[]'",
                [],
            )?;
        }
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            mountains: Arc::new(serde_json::from_str(include_str!(
                "../static/catalog.json"
            ))?),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12))
                .user_agent("WeekendClub/0.1")
                .build()?,
            weather_locks: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

pub fn app(state: AppState, static_directory: &str) -> Router {
    Router::new()
        .route("/api/health", get(|| async { Json(json!({"status":"ok"})) }))
        .route("/api/mountains", get(mountains))
        .route("/api/plans", post(create_plan))
        .route("/api/plans/{id}", get(get_plan).patch(update_plan))
        .route("/api/plans/{id}/availability", put(save_availability))
        .route("/api/weather", post(weather))
        .route("/api/{*path}", any(|| async { ApiError(StatusCode::NOT_FOUND, "This endpoint does not exist.".into()) }))
        .fallback_service(ServeDir::new(static_directory).fallback(ServeFile::new(format!("{static_directory}/index.html"))))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        .layer(SetResponseHeaderLayer::if_not_present(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::if_not_present(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'; font-src 'self'; base-uri 'self'; frame-ancestors 'none'")))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn mountains(State(state): State<AppState>) -> Json<Value> {
    Json(json!({"mountains":state.mountains.as_ref(), "today":today(), "forecast_days":16}))
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PlanInput {
    pub username: String,
    pub title: String,
    pub mountains: Vec<String>,
    pub weekends: Vec<String>,
    #[serde(default)]
    pub trips: Option<Vec<String>>,
}
fn resolve_trips(
    mountains: &[String],
    weekends: &[String],
    trips: Option<&[String]>,
) -> Result<Vec<String>, ApiError> {
    let possible: HashSet<String> = mountains
        .iter()
        .flat_map(|m| weekends.iter().map(move |w| format!("{m}@{w}")))
        .collect();
    let mut chosen = trips
        .map(|t| t.to_vec())
        .unwrap_or_else(|| possible.iter().cloned().collect());
    if chosen.is_empty()
        || chosen.len() > possible.len()
        || chosen.iter().any(|t| !possible.contains(t))
        || chosen.iter().collect::<HashSet<_>>().len() != chosen.len()
    {
        return Err(invalid(
            "Choose valid, unique mountain options for your weekends.",
        ));
    }
    if weekends
        .iter()
        .any(|w| !chosen.iter().any(|t| t.ends_with(&format!("@{w}"))))
        || mountains
            .iter()
            .any(|m| !chosen.iter().any(|t| t.starts_with(&format!("{m}@"))))
    {
        return Err(invalid(
            "Choose at least one mountain for every selected weekend.",
        ));
    }
    chosen.sort();
    Ok(chosen)
}
fn clean_username(username: &str) -> Result<String, ApiError> {
    let username = username.trim();
    if !(2..=24).contains(&username.len())
        || !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_- ".contains(c))
    {
        return Err(invalid(
            "Choose a username of 2–24 letters, numbers, spaces, hyphens, or underscores.",
        ));
    }
    Ok(username.to_string())
}
fn validate_options(
    state: &AppState,
    mountain_ids: &[String],
    weekends: &[String],
    existing: &[String],
) -> Result<(), ApiError> {
    if mountain_ids.is_empty() || mountain_ids.len() > 12 {
        return Err(invalid("Choose between 1 and 12 mountains."));
    }
    if weekends.is_empty() || weekends.len() > 26 {
        return Err(invalid("Choose between 1 and 26 weekends."));
    }
    if mountain_ids.iter().collect::<HashSet<_>>().len() != mountain_ids.len()
        || weekends.iter().collect::<HashSet<_>>().len() != weekends.len()
    {
        return Err(invalid("Mountains and weekends must be unique."));
    }
    if mountain_ids
        .iter()
        .any(|id| !state.mountains.iter().any(|m| &m.id == id))
    {
        return Err(invalid("Choose mountains from the Ikon destination list."));
    }
    for weekend in weekends {
        let date = NaiveDate::parse_from_str(weekend, "%Y-%m-%d")
            .map_err(|_| invalid("Choose a valid weekend date."))?;
        if date.to_string() != *weekend || date.weekday() != Weekday::Sat {
            return Err(invalid("Weekends must start on a Saturday."));
        }
        if (!existing.contains(weekend) && date + Duration::days(1) < today())
            || date > today() + Duration::weeks(52)
        {
            return Err(invalid("Choose a weekend within the next year."));
        }
    }
    Ok(())
}
fn clean_title(title: &str) -> Result<String, ApiError> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 70 || title.chars().any(|c| c.is_control()) {
        return Err(invalid("Give your planner a name of 1–70 characters."));
    }
    Ok(title.into())
}

#[derive(Serialize)]
pub struct Plan {
    pub id: String,
    pub title: String,
    pub owner: String,
    pub mountains: Vec<String>,
    pub weekends: Vec<String>,
    pub trips: Vec<String>,
    pub created_at: String,
    pub members: Vec<Member>,
}
#[derive(Serialize)]
pub struct Member {
    pub id: String,
    pub username: String,
    pub trips: Vec<String>,
}

fn read_plan(state: &AppState, id: &str) -> Result<Plan, ApiError> {
    let db = state.db.lock().unwrap();
    let row = db
        .query_row(
            "SELECT title,owner,mountains,weekends,created_at,trip_options FROM plans WHERE id=?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                "We couldn’t find this planner. Check the link and try again.".into(),
            )
        })?;
    let mut statement =
        db.prepare("SELECT id,username,trips FROM members WHERE plan_id=?1 ORDER BY rowid")?;
    let rows = statement.query_map([id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut members = Vec::new();
    for row in rows {
        let (id, username, trips) = row?;
        members.push(Member {
            id,
            username,
            trips: serde_json::from_str(&trips)?,
        });
    }
    let mountains = serde_json::from_str::<Vec<String>>(&row.2)?;
    let weekends = serde_json::from_str::<Vec<String>>(&row.3)?;
    let stored_trips = serde_json::from_str::<Vec<String>>(&row.5)?;
    let trips = resolve_trips(
        &mountains,
        &weekends,
        if stored_trips.is_empty() {
            None
        } else {
            Some(&stored_trips)
        },
    )?;
    Ok(Plan {
        id: id.into(),
        title: row.0,
        owner: row.1,
        mountains,
        weekends,
        trips,
        created_at: row.4,
        members,
    })
}
async fn create_plan(
    State(state): State<AppState>,
    Json(input): Json<PlanInput>,
) -> ApiResult<Value> {
    let username = clean_username(&input.username)?;
    let title = clean_title(&input.title)?;
    validate_options(&state, &input.mountains, &input.weekends, &[])?;
    let trips = resolve_trips(&input.mountains, &input.weekends, input.trips.as_deref())?;
    let id = Uuid::new_v4().simple().to_string();
    let owner_token = token();
    let mut weekends = input.weekends.clone();
    weekends.sort();
    state.db.lock().unwrap().execute(
        "INSERT INTO plans (id,title,owner,owner_token,mountains,weekends,created_at,trip_options) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            id,
            title,
            username,
            owner_token,
            serde_json::to_string(&input.mountains)?,
            serde_json::to_string(&weekends)?,
            Utc::now().to_rfc3339(),
            serde_json::to_string(&trips)?
        ],
    )?;
    Ok(Json(
        json!({"plan":read_plan(&state,&id)?,"owner_token":owner_token,"share_path":format!("/p/{id}")}),
    ))
}
async fn get_plan(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Plan> {
    Ok(Json(read_plan(&state, &id)?))
}

async fn update_plan(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<PlanInput>,
) -> ApiResult<Plan> {
    let current = read_plan(&state, &id)?;
    let username = clean_username(&input.username)?;
    let title = clean_title(&input.title)?;
    validate_options(&state, &input.mountains, &input.weekends, &current.weekends)?;
    let trips = resolve_trips(&input.mountains, &input.weekends, input.trips.as_deref())?;
    let mut db = state.db.lock().unwrap();
    let stored: String = db.query_row("SELECT owner_token FROM plans WHERE id=?1", [&id], |r| {
        r.get(0)
    })?;
    if stored != bearer(&headers) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Only this planner’s owner can change its mountains or weekends.".into(),
        ));
    }
    if username != current.owner {
        return Err(invalid("The planner owner cannot be changed."));
    }
    let mut weekends = input.weekends.clone();
    weekends.sort();
    let transaction = db.transaction()?;
    transaction.execute(
        "UPDATE plans SET title=?1,mountains=?2,weekends=?3,trip_options=?5 WHERE id=?4",
        params![
            title,
            serde_json::to_string(&input.mountains)?,
            serde_json::to_string(&weekends)?,
            id,
            serde_json::to_string(&trips)?
        ],
    )?;
    let allowed: HashSet<String> = trips.into_iter().collect();
    let members = {
        let mut statement = transaction.prepare("SELECT id,trips FROM members WHERE plan_id=?1")?;
        statement
            .query_map([&id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (member_id, encoded_trips) in members {
        let trips: Vec<String> = serde_json::from_str(&encoded_trips)?;
        let trips: Vec<_> = trips.iter().filter(|t| allowed.contains(*t)).collect();
        transaction.execute(
            "UPDATE members SET trips=?1 WHERE id=?2",
            params![serde_json::to_string(&trips)?, member_id],
        )?;
    }
    transaction.commit()?;
    drop(db);
    Ok(Json(read_plan(&state, &id)?))
}

#[derive(Deserialize)]
struct AvailabilityInput {
    username: String,
    trips: Vec<String>,
}
async fn save_availability(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<AvailabilityInput>,
) -> ApiResult<Value> {
    let plan = read_plan(&state, &id)?;
    let username = clean_username(&input.username)?;
    if username.eq_ignore_ascii_case(&plan.owner) {
        return Err(invalid(
            "That username belongs to the planner’s owner. Choose your own username.",
        ));
    }
    let allowed: HashSet<String> = plan.trips.iter().cloned().collect();
    if input.trips.len() > allowed.len()
        || input.trips.iter().any(|t| !allowed.contains(t))
        || input.trips.iter().collect::<HashSet<_>>().len() != input.trips.len()
    {
        return Err(invalid(
            "You can only join mountains and weekends selected by the owner.",
        ));
    }
    let db = state.db.lock().unwrap();
    // Recheck under the write lock, so a simultaneous owner edit cannot admit a removed option.
    let (latest_mountains, latest_weekends, latest_trips): (String, String, String) = db
        .query_row(
            "SELECT mountains,weekends,trip_options FROM plans WHERE id=?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
    let latest_mountains: Vec<String> = serde_json::from_str(&latest_mountains)?;
    let latest_weekends: Vec<String> = serde_json::from_str(&latest_weekends)?;
    let latest_trips: Vec<String> = serde_json::from_str(&latest_trips)?;
    let latest_allowed = resolve_trips(
        &latest_mountains,
        &latest_weekends,
        if latest_trips.is_empty() {
            None
        } else {
            Some(&latest_trips)
        },
    )?;
    if input
        .trips
        .iter()
        .any(|trip| !latest_allowed.contains(trip))
    {
        return Err(invalid(
            "The owner updated this planner. Refresh and choose from the current options.",
        ));
    }
    let existing: Option<(String, String)> = db
        .query_row(
            "SELECT id,token FROM members WHERE plan_id=?1 AND username_key=?2",
            params![id, username.to_ascii_lowercase()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (member_id, member_token) = if let Some((member_id, member_token)) = existing {
        if member_token != bearer(&headers) {
            return Err(ApiError(StatusCode::CONFLICT,"That username is already in the crew. Use the browser where you joined, or choose another username.".into()));
        }
        db.execute(
            "UPDATE members SET trips=?1 WHERE id=?2",
            params![serde_json::to_string(&input.trips)?, member_id],
        )?;
        (member_id, member_token)
    } else {
        if input.trips.is_empty() {
            return Err(invalid("Pick at least one trip to join the crew."));
        }
        let member_count: usize = db.query_row(
            "SELECT COUNT(*) FROM members WHERE plan_id=?1",
            [&id],
            |r| r.get(0),
        )?;
        if member_count >= 60 {
            return Err(invalid(
                "This planner has reached its 60-person crew limit.",
            ));
        }
        let member_id = Uuid::new_v4().simple().to_string();
        let member_token = token();
        db.execute(
            "INSERT INTO members VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                member_id,
                id,
                username,
                username.to_ascii_lowercase(),
                member_token,
                serde_json::to_string(&input.trips)?
            ],
        )?;
        (member_id, member_token)
    };
    drop(db);
    Ok(Json(
        json!({"member_id":member_id,"member_token":member_token,"plan":read_plan(&state,&id)?}),
    ))
}

#[derive(Deserialize)]
struct WeatherInput {
    mountains: Vec<String>,
    weekends: Vec<String>,
    #[serde(default)]
    trips: Option<Vec<String>>,
}
#[derive(Clone, Deserialize, Serialize)]
struct ForecastData {
    daily: DailyData,
    daily_units: Value,
}
#[derive(Clone, Deserialize, Serialize)]
struct DailyData {
    time: Vec<String>,
    temperature_2m_max: Vec<Option<f64>>,
    temperature_2m_min: Vec<Option<f64>>,
    snowfall_sum: Vec<Option<f64>>,
    wind_speed_10m_max: Vec<Option<f64>>,
    #[serde(default)]
    weather_code: Vec<Option<u32>>,
}
#[derive(Serialize)]
struct WeatherDay {
    date: String,
    high_f: f64,
    low_f: f64,
    snowfall_inches: f64,
    wind_mph: f64,
    code: Option<u32>,
}
#[derive(Serialize)]
struct WeekendForecast {
    mountain_id: String,
    weekend: String,
    status: &'static str,
    days: Vec<WeatherDay>,
    updated_at: Option<String>,
    stale: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    estimate_basis: Option<climate::EstimateBasis>,
}
fn forecast_for(
    mountain: &Mountain,
    weekend: &str,
    cached: Option<&(i64, ForecastData)>,
) -> WeekendForecast {
    let mut forecast = WeekendForecast {
        mountain_id: mountain.id.clone(),
        weekend: weekend.into(),
        status: "unavailable",
        days: vec![],
        updated_at: None,
        stale: false,
        estimate_basis: None,
    };
    let date = NaiveDate::parse_from_str(weekend, "%Y-%m-%d").unwrap();
    let local_today = Utc::now()
        .with_timezone(&mountain.timezone.parse::<chrono_tz::Tz>().unwrap())
        .date_naive();
    if date + Duration::days(1) < local_today {
        forecast.status = "past";
        return forecast;
    }
    if date + Duration::days(1) > local_today + Duration::days(15) {
        forecast.status = "pending";
        return forecast;
    }
    if let Some((fetched_at, data)) = cached {
        let daily = &data.daily;
        for day in [date, date + Duration::days(1)] {
            if let Some(index) = daily.time.iter().position(|t| t == &day.to_string()) {
                if let (Some(high), Some(low), Some(snow), Some(wind), Some(code)) = (
                    daily.temperature_2m_max.get(index).copied().flatten(),
                    daily.temperature_2m_min.get(index).copied().flatten(),
                    daily.snowfall_sum.get(index).copied().flatten(),
                    daily.wind_speed_10m_max.get(index).copied().flatten(),
                    daily.weather_code.get(index).copied().flatten(),
                ) {
                    let snow = if data.daily_units["snowfall_sum"] == "cm" {
                        snow / 2.54
                    } else {
                        snow
                    };
                    forecast.days.push(WeatherDay {
                        date: day.to_string(),
                        high_f: high,
                        low_f: low,
                        snowfall_inches: snow,
                        wind_mph: wind,
                        code: Some(code),
                    });
                }
            }
        }
        if !forecast.days.is_empty() {
            forecast.status = if forecast.days.len() == 2 {
                "available"
            } else {
                "partial"
            };
            forecast.updated_at =
                chrono::DateTime::from_timestamp(*fetched_at, 0).map(|t| t.to_rfc3339());
            forecast.stale = Utc::now().timestamp() - fetched_at >= CACHE_SECONDS;
        }
    }
    forecast
}
async fn fetch_forecast(state: &AppState, mountain: &Mountain) -> Option<(i64, ForecastData)> {
    let lock = state
        .weather_locks
        .lock()
        .unwrap()
        .entry(mountain.id.clone())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let _guard = lock.lock().await;
    let cached = {
        let db = state.db.lock().unwrap();
        db.query_row(
            "SELECT fetched_at,data FROM weather_cache WHERE mountain_id=?1",
            [&mountain.id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()
        .ok()
        .flatten()
        .and_then(|(time, data)| serde_json::from_str(&data).ok().map(|data| (time, data)))
    };
    if cached
        .as_ref()
        .is_some_and(|(time, _)| Utc::now().timestamp() - time < CACHE_SECONDS)
    {
        return cached;
    }
    let result = state.client.get("https://api.open-meteo.com/v1/forecast").query(&[
        ("latitude",mountain.latitude.to_string()),("longitude",mountain.longitude.to_string()),("timezone",mountain.timezone.clone()),
        ("forecast_days","16".into()),("temperature_unit","fahrenheit".into()),("wind_speed_unit","mph".into()),
        ("precipitation_unit","inch".into()),("daily","temperature_2m_max,temperature_2m_min,snowfall_sum,wind_speed_10m_max,weather_code".into()),
    ]).send().await;
    match result {
        Ok(response) => match response.error_for_status() {
            Ok(response) => {
                if let Ok(data) = response.json::<ForecastData>().await {
                    let time = Utc::now().timestamp();
                    if let Ok(encoded) = serde_json::to_string(&data) {
                        if let Err(error) = state.db.lock().unwrap().execute("INSERT INTO weather_cache VALUES (?1,?2,?3) ON CONFLICT(mountain_id) DO UPDATE SET fetched_at=excluded.fetched_at,data=excluded.data",params![mountain.id,time,encoded]) { tracing::warn!(%error,"weather cache write failed"); }
                    }
                    return Some((time, data));
                }
            }
            Err(error) => {
                tracing::warn!(%error,mountain=%mountain.id,"forecast service unavailable")
            }
        },
        Err(error) => tracing::warn!(%error,mountain=%mountain.id,"forecast request failed"),
    }
    cached
}
async fn weather(
    State(state): State<AppState>,
    Json(input): Json<WeatherInput>,
) -> ApiResult<Value> {
    validate_options(&state, &input.mountains, &input.weekends, &input.weekends)?;
    let trips = resolve_trips(&input.mountains, &input.weekends, input.trips.as_deref())?;
    let mut requests = tokio::task::JoinSet::new();
    for id in &input.mountains {
        let mountain = state
            .mountains
            .iter()
            .find(|m| &m.id == id)
            .unwrap()
            .clone();
        let state = state.clone();
        let weekends: Vec<_> = input
            .weekends
            .iter()
            .filter(|w| trips.contains(&format!("{id}@{w}")))
            .cloned()
            .collect();
        requests.spawn(async move {
            let local_today = Utc::now()
                .with_timezone(&mountain.timezone.parse::<chrono_tz::Tz>().unwrap())
                .date_naive();
            let needs_forecast = weekends.iter().any(|w| {
                let date = NaiveDate::parse_from_str(w, "%Y-%m-%d").unwrap();
                date + Duration::days(1) >= local_today
                    && date + Duration::days(1) <= local_today + Duration::days(15)
            });
            let cached = if needs_forecast {
                fetch_forecast(&state, &mountain).await
            } else {
                None
            };
            let mut forecasts: Vec<_> = weekends
                .iter()
                .map(|w| forecast_for(&mountain, w, cached.as_ref()))
                .collect();
            if forecasts
                .iter()
                .any(|f| f.status != "available" && f.status != "past")
            {
                let historical = climate::fetch(&state, &mountain).await;
                for forecast in &mut forecasts {
                    if forecast.status == "available" || forecast.status == "past" {
                        continue;
                    }
                    if let Some(mut estimate) =
                        climate::estimate_for(&mountain, &forecast.weekend, historical.as_ref())
                    {
                        if forecast.status == "partial" {
                            for day in &mut estimate.days {
                                if let Some(index) =
                                    forecast.days.iter().position(|d| d.date == day.date)
                                {
                                    *day = forecast.days.remove(index);
                                }
                            }
                            estimate.status = "mixed";
                        }
                        *forecast = estimate;
                    } else {
                        forecast.status = "unavailable";
                    }
                }
            }
            forecasts
        });
    }
    let mut forecasts = Vec::new();
    while let Some(result) = requests.join_next().await {
        forecasts.extend(result.map_err(|error| {
            tracing::error!(%error,"weather task failed");
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Weather estimates could not refresh. Try again shortly.".into(),
            )
        })?);
    }
    Ok(Json(
        json!({"forecasts":forecasts,"source":"Open-Meteo","forecast_days":16,"estimate_method":"Five complete years of ERA5 data, averaged within seven calendar days of each trip day"}),
    ))
}

pub async fn poll_weather(state: AppState) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(CACHE_SECONDS as u64));
    loop {
        interval.tick().await;
        let planned = {
            let db = state.db.lock().unwrap();
            let mut query = db
                .prepare("SELECT mountains,weekends,trip_options FROM plans")
                .unwrap();
            query
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })
                .unwrap()
                .filter_map(Result::ok)
                .collect::<Vec<_>>()
        };
        let mut forecast_ids = HashSet::new();
        let mut climate_ids = HashSet::new();
        for (mountains, weekends, trips) in planned {
            let mountains: Vec<String> = serde_json::from_str(&mountains).unwrap_or_default();
            let weekends: Vec<String> = serde_json::from_str(&weekends).unwrap_or_default();
            let stored: Vec<String> = serde_json::from_str(&trips).unwrap_or_default();
            let Ok(trips) = resolve_trips(
                &mountains,
                &weekends,
                if stored.is_empty() {
                    None
                } else {
                    Some(&stored)
                },
            ) else {
                continue;
            };
            for trip in trips {
                let Some((id, weekend)) = trip.split_once('@') else {
                    continue;
                };
                let Some(mountain) = state.mountains.iter().find(|m| m.id == id) else {
                    continue;
                };
                let local_today = Utc::now()
                    .with_timezone(&mountain.timezone.parse::<chrono_tz::Tz>().unwrap())
                    .date_naive();
                if let Ok(date) = NaiveDate::parse_from_str(weekend, "%Y-%m-%d") {
                    if date + Duration::days(1) >= local_today {
                        climate_ids.insert(id.to_string());
                        if date + Duration::days(1) <= local_today + Duration::days(15) {
                            forecast_ids.insert(id.to_string());
                        }
                    }
                }
            }
        }
        for id in climate_ids {
            if let Some(mountain) = state.mountains.iter().find(|m| m.id == id) {
                if forecast_ids.contains(&id) {
                    fetch_forecast(&state, mountain).await;
                }
                climate::fetch(&state, mountain).await;
            }
        }
    }
}

#[cfg(test)]
mod tests;
