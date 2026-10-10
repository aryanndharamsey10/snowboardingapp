use super::*;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn next_saturday() -> String {
    let mut day = today() + Duration::days(1);
    while day.weekday() != Weekday::Sat {
        day += Duration::days(1);
    }
    day.to_string()
}
async fn request(
    router: &Router,
    method: &str,
    path: &str,
    body: Value,
    secret: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("Content-Type", "application/json");
    if let Some(secret) = secret {
        request = request.header("Authorization", format!("Bearer {secret}"));
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap())
}
async fn create(router: &Router) -> Value {
    let (status,plan)=request(router,"POST","/api/plans",json!({"username":"aryan","title":"Winter with friends","mountains":["mammoth","palisades"],"weekends":[next_saturday()]}),None).await;
    assert_eq!(status, StatusCode::OK);
    plan
}

#[tokio::test]
async fn public_link_exposes_options_without_ownership_credentials() {
    let router = app(AppState::new(":memory:").unwrap(), "static");
    let created = create(&router).await;
    let path = format!("/api/plans/{}", created["plan"]["id"].as_str().unwrap());
    let (status, public) = request(&router, "GET", &path, Value::Null, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(public["mountains"], json!(["mammoth", "palisades"]));
    assert_eq!(public["owner"], "aryan");
    assert!(public.get("owner_token").is_none());
    assert!(
        !public
            .to_string()
            .contains(created["owner_token"].as_str().unwrap())
    );
}

#[tokio::test]
async fn friends_cannot_edit_the_planner_or_join_unselected_options() {
    let router = app(AppState::new(":memory:").unwrap(), "static");
    let created = create(&router).await;
    let id = created["plan"]["id"].as_str().unwrap();
    let input = json!({"username":"aryan","title":"Changed","mountains":["alta"],"weekends":[next_saturday()]});
    assert_eq!(
        request(&router, "PATCH", &format!("/api/plans/{id}"), input, None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let wrong_mountain = json!({"username":"friend","trips":[format!("alta@{}",next_saturday())]});
    assert_eq!(
        request(
            &router,
            "PUT",
            &format!("/api/plans/{id}/availability"),
            wrong_mountain,
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let wrong_date = json!({"username":"friend","trips":["mammoth@2099-01-03"]});
    assert_eq!(
        request(
            &router,
            "PUT",
            &format!("/api/plans/{id}/availability"),
            wrong_date,
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let owner_name = json!({"username":"ARYAN","trips":[format!("mammoth@{}",next_saturday())]});
    assert_eq!(
        request(
            &router,
            "PUT",
            &format!("/api/plans/{id}/availability"),
            owner_name,
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn availability_updates_require_the_original_members_credential() {
    let router = app(AppState::new(":memory:").unwrap(), "static");
    let created = create(&router).await;
    let path = format!(
        "/api/plans/{}/availability",
        created["plan"]["id"].as_str().unwrap()
    );
    let input = json!({"username":"Alex","trips":[format!("mammoth@{}",next_saturday())]});
    let (status, joined) = request(&router, "PUT", &path, input.clone(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(joined["plan"]["members"].as_array().unwrap().len(), 1);
    assert_eq!(
        request(&router, "PUT", &path, input, None).await.0,
        StatusCode::CONFLICT
    );
    let empty = json!({"username":"alex","trips":[]});
    let (status, updated) = request(
        &router,
        "PUT",
        &path,
        empty,
        joined["member_token"].as_str(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["plan"]["members"][0]["trips"], json!([]));
    assert!(updated["plan"]["members"][0].get("token").is_none());
}

#[tokio::test]
async fn owner_edits_remove_availability_for_removed_trips_and_preserve_the_owner() {
    let router = app(AppState::new(":memory:").unwrap(), "static");
    let created = create(&router).await;
    let id = created["plan"]["id"].as_str().unwrap();
    let trips = json!({"username":"Alex","trips":[format!("mammoth@{}",next_saturday()),format!("palisades@{}",next_saturday())]});
    request(
        &router,
        "PUT",
        &format!("/api/plans/{id}/availability"),
        trips,
        None,
    )
    .await;
    let edit = json!({"username":"aryan","title":"Just Mammoth","mountains":["mammoth"],"weekends":[next_saturday()]});
    let (status, updated) = request(
        &router,
        "PATCH",
        &format!("/api/plans/{id}"),
        edit,
        created["owner_token"].as_str(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        updated["members"][0]["trips"],
        json!([format!("mammoth@{}", next_saturday())])
    );
    let edit = json!({"username":"another-owner","title":"Invalid","mountains":["mammoth"],"weekends":[next_saturday()]});
    assert_eq!(
        request(
            &router,
            "PATCH",
            &format!("/api/plans/{id}"),
            edit,
            created["owner_token"].as_str()
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn dates_and_catalog_options_are_validated() {
    let router = app(AppState::new(":memory:").unwrap(), "static");
    for (mountains, weekends) in [
        (json!(["invented-mountain"]), json!([next_saturday()])),
        (json!(["mammoth"]), json!(["2027-01-04"])),
        (json!(["mammoth", "mammoth"]), json!([next_saturday()])),
        (json!(["mammoth"]), json!(["2099-01-03"])),
        (json!(["mammoth"]), json!([])),
    ] {
        let input =
            json!({"username":"aryan","title":"Invalid","mountains":mountains,"weekends":weekends});
        assert_eq!(
            request(&router, "POST", "/api/plans", input, None).await.0,
            StatusCode::BAD_REQUEST
        );
    }
}

#[tokio::test]
async fn far_future_weather_uses_labeled_historical_estimates() {
    let state = AppState::new(":memory:").unwrap();
    let historical = historical_fixture();
    state
        .db
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO climate_cache VALUES (?1,?2,?3)",
            params![
                "mammoth",
                Utc::now().timestamp(),
                serde_json::to_string(&historical).unwrap()
            ],
        )
        .unwrap();
    let router = app(state, "static");
    let mut future = today() + Duration::days(60);
    while future.weekday() != Weekday::Sat {
        future += Duration::days(1);
    }
    let (status, weather) = request(
        &router,
        "POST",
        "/api/weather",
        json!({"mountains":["mammoth"],"weekends":[future.to_string()]}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(weather["forecasts"][0]["status"], "estimate");
    assert_eq!(weather["forecasts"][0]["days"].as_array().unwrap().len(), 2);
    assert_eq!(weather["forecasts"][0]["days"][0]["high_f"], 40.0);
    assert_eq!(weather["forecasts"][0]["days"][0]["code"], Value::Null);
    assert!(weather["forecasts"][0].get("estimate_basis").is_some());
}

#[test]
fn weather_converts_snowfall_units_and_handles_missing_model_data() {
    let state = AppState::new(":memory:").unwrap();
    let mountain = &state.mountains[0];
    let weekend = next_saturday();
    let mut data = ForecastData {
        daily: DailyData {
            time: vec![
                weekend.clone(),
                (NaiveDate::parse_from_str(&weekend, "%Y-%m-%d").unwrap() + Duration::days(1))
                    .to_string(),
            ],
            temperature_2m_max: vec![Some(32.0); 2],
            temperature_2m_min: vec![Some(20.0); 2],
            snowfall_sum: vec![Some(2.54); 2],
            wind_speed_10m_max: vec![Some(10.0); 2],
            weather_code: vec![Some(71); 2],
        },
        daily_units: json!({"snowfall_sum":"cm"}),
    };
    let forecast = forecast_for(
        mountain,
        &weekend,
        Some(&(Utc::now().timestamp(), data.clone())),
    );
    assert_eq!(forecast.status, "available");
    assert!((forecast.days[0].snowfall_inches - 1.0).abs() < 0.001);
    data.daily.snowfall_sum[0] = None;
    let forecast = forecast_for(
        mountain,
        &weekend,
        Some(&(Utc::now().timestamp() - 7200, data)),
    );
    assert_eq!(forecast.status, "partial");
    assert_eq!(forecast.days.len(), 1);
    assert!(forecast.stale);
}

#[tokio::test]
async fn sqlite_records_survive_reopening_the_database() {
    let database =
        std::env::temp_dir().join(format!("weekend-club-test-{}.sqlite", Uuid::new_v4()));
    let database = database.to_str().unwrap();
    let id = {
        let router = app(AppState::new(database).unwrap(), "static");
        create(&router).await["plan"]["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let reopened = AppState::new(database).unwrap();
    assert_eq!(
        read_plan(&reopened, &id).unwrap().title,
        "Winter with friends"
    );
    drop(reopened);
    std::fs::remove_file(database).unwrap();
}

#[test]
fn catalog_has_unique_ids_and_valid_us_timezones() {
    let state = AppState::new(":memory:").unwrap();
    assert_eq!(state.mountains.len(), 41);
    assert_eq!(
        state
            .mountains
            .iter()
            .map(|m| &m.id)
            .collect::<HashSet<_>>()
            .len(),
        41
    );
    assert!(
        state
            .mountains
            .iter()
            .all(|m| m.timezone.parse::<chrono_tz::Tz>().is_ok()
                && m.longitude < 0.0
                && m.latitude > 0.0)
    );
}

#[tokio::test]
async fn shared_links_serve_the_frontend_with_a_successful_status() {
    let router = app(AppState::new(":memory:").unwrap(), "static");
    let response = router
        .oneshot(
            Request::builder()
                .uri("/p/0123456789abcdef0123456789abcdef")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&body).contains("/app.js"));
}

fn historical_fixture() -> ForecastData {
    let mut date = NaiveDate::from_ymd_opt(today().year() - 5, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(today().year() - 1, 12, 31).unwrap();
    let mut time = Vec::new();
    while date <= end {
        time.push(date.to_string());
        date += Duration::days(1);
    }
    let length = time.len();
    ForecastData {
        daily: DailyData {
            time,
            temperature_2m_max: vec![Some(40.0); length],
            temperature_2m_min: vec![Some(20.0); length],
            snowfall_sum: vec![Some(2.54); length],
            wind_speed_10m_max: vec![Some(10.0); length],
            weather_code: vec![],
        },
        daily_units: json!({"snowfall_sum":"cm"}),
    }
}

#[test]
fn historical_estimates_handle_new_year_and_leap_day_without_inventing_weather_codes() {
    let state = AppState::new(":memory:").unwrap();
    let historical = (Utc::now().timestamp(), historical_fixture());
    for target in ["2028-01-01", "2028-02-26", "2028-02-28", "2028-12-31"] {
        let estimate =
            climate::estimate_for(&state.mountains[0], target, Some(&historical)).unwrap();
        assert_eq!(estimate.days.len(), 2);
        assert!((estimate.days[0].snowfall_inches - 1.0).abs() < 0.001);
        assert_eq!(estimate.days[0].high_f, 40.0);
        assert_eq!(estimate.days[0].code, None);
    }
}

#[tokio::test]
async fn mountain_options_are_specific_to_each_weekend() {
    let router = app(AppState::new(":memory:").unwrap(), "static");
    let first = next_saturday();
    let second =
        (NaiveDate::parse_from_str(&first, "%Y-%m-%d").unwrap() + Duration::weeks(1)).to_string();
    let trips = vec![format!("mammoth@{first}"), format!("palisades@{second}")];
    let input = json!({"username":"owner","title":"Different destinations","mountains":["mammoth","palisades"],"weekends":[first,second],"trips":trips});
    let (status, created) = request(&router, "POST", "/api/plans", input, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["plan"]["trips"].as_array().unwrap().len(), 2);
    let id = created["plan"]["id"].as_str().unwrap();
    let invalid = json!({"username":"friend","trips":[format!("mammoth@{second}")]});
    assert_eq!(
        request(
            &router,
            "PUT",
            &format!("/api/plans/{id}/availability"),
            invalid,
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let valid = json!({"username":"friend","trips":trips});
    assert_eq!(
        request(
            &router,
            "PUT",
            &format!("/api/plans/{id}/availability"),
            valid,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn every_weekend_requires_at_least_one_mountain() {
    let router = app(AppState::new(":memory:").unwrap(), "static");
    let first = next_saturday();
    let second =
        (NaiveDate::parse_from_str(&first, "%Y-%m-%d").unwrap() + Duration::weeks(1)).to_string();
    let input = json!({"username":"owner","title":"Incomplete","mountains":["mammoth"],"weekends":[first,second],"trips":[format!("mammoth@{first}")]});
    assert_eq!(
        request(&router, "POST", "/api/plans", input, None).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[test]
fn existing_sqlite_planners_are_migrated_without_losing_their_options() {
    let database =
        std::env::temp_dir().join(format!("weekend-migration-{}.sqlite", Uuid::new_v4()));
    {
        let db = Connection::open(&database).unwrap();
        db.execute_batch("CREATE TABLE plans (id TEXT PRIMARY KEY,title TEXT NOT NULL,owner TEXT NOT NULL,owner_token TEXT NOT NULL,mountains TEXT NOT NULL,weekends TEXT NOT NULL,created_at TEXT NOT NULL)").unwrap();
        db.execute(
            "INSERT INTO plans VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                "legacy",
                "Legacy",
                "owner",
                "secret",
                json!(["mammoth", "palisades"]).to_string(),
                json!([next_saturday()]).to_string(),
                Utc::now().to_rfc3339()
            ],
        )
        .unwrap();
    }
    let state = AppState::new(database.to_str().unwrap()).unwrap();
    let plan = read_plan(&state, "legacy").unwrap();
    assert_eq!(plan.trips.len(), 2);
    assert_eq!(plan.owner, "owner");
    drop(state);
    std::fs::remove_file(database).unwrap();
}
