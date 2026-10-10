use super::*;

const CLIMATE_CACHE_SECONDS: i64 = 30 * 24 * 3600;

#[derive(Serialize)]
pub(super) struct EstimateBasis {
    start_year: i32,
    end_year: i32,
    sample_days_per_day: usize,
    calendar_window_days: usize,
}

pub(super) fn estimate_for(
    mountain: &Mountain,
    weekend: &str,
    cached: Option<&(i64, ForecastData)>,
) -> Option<WeekendForecast> {
    let (fetched_at, data) = cached?;
    let weekend_date = NaiveDate::parse_from_str(weekend, "%Y-%m-%d").ok()?;
    let daily = &data.daily;
    let parsed: Vec<_> = daily
        .time
        .iter()
        .enumerate()
        .filter_map(|(i, d)| {
            NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .ok()
                .map(|d| (i, d))
        })
        .collect();
    let start_year = parsed.iter().map(|(_, d)| d.year()).min()?;
    let end_year = parsed.iter().map(|(_, d)| d.year()).max()?;
    let mut days = Vec::new();
    let mut sample_count = usize::MAX;
    for target in [weekend_date, weekend_date + Duration::days(1)] {
        // Use a leap-year calendar for circular month/day distances, including New Year and Feb 29.
        let target_ordinal =
            NaiveDate::from_ymd_opt(2000, target.month(), target.day())?.ordinal() as i32;
        let mut count = 0usize;
        let mut totals = [0.0; 4];
        for (index, day) in &parsed {
            let ordinal = NaiveDate::from_ymd_opt(2000, day.month(), day.day())?.ordinal() as i32;
            let difference = (ordinal - target_ordinal).abs();
            if difference.min(366 - difference) > 7 {
                continue;
            }
            if let (Some(high), Some(low), Some(snow), Some(wind)) = (
                daily.temperature_2m_max.get(*index).copied().flatten(),
                daily.temperature_2m_min.get(*index).copied().flatten(),
                daily.snowfall_sum.get(*index).copied().flatten(),
                daily.wind_speed_10m_max.get(*index).copied().flatten(),
            ) {
                if [high, low, snow, wind].iter().any(|v| !v.is_finite()) {
                    continue;
                }
                let snow = if data.daily_units["snowfall_sum"] == "cm" {
                    snow / 2.54
                } else {
                    snow
                };
                totals[0] += high;
                totals[1] += low;
                totals[2] += snow;
                totals[3] += wind;
                count += 1;
            }
        }
        if count < 10 {
            return None;
        }
        sample_count = sample_count.min(count);
        days.push(WeatherDay {
            date: target.to_string(),
            high_f: totals[0] / count as f64,
            low_f: totals[1] / count as f64,
            snowfall_inches: totals[2] / count as f64,
            wind_mph: totals[3] / count as f64,
            code: None,
        });
    }
    Some(WeekendForecast {
        mountain_id: mountain.id.clone(),
        weekend: weekend.into(),
        status: "estimate",
        days,
        updated_at: chrono::DateTime::from_timestamp(*fetched_at, 0).map(|t| t.to_rfc3339()),
        stale: Utc::now().timestamp() - fetched_at >= CLIMATE_CACHE_SECONDS,
        estimate_basis: Some(EstimateBasis {
            start_year,
            end_year,
            sample_days_per_day: sample_count,
            calendar_window_days: 15,
        }),
    })
}

pub(super) async fn fetch(state: &AppState, mountain: &Mountain) -> Option<(i64, ForecastData)> {
    let lock = state
        .weather_locks
        .lock()
        .unwrap()
        .entry(format!("climate:{}", mountain.id))
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let _guard = lock.lock().await;
    let cached = {
        let db = state.db.lock().unwrap();
        db.query_row(
            "SELECT fetched_at,data FROM climate_cache WHERE mountain_id=?1",
            [&mountain.id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()
        .ok()
        .flatten()
        .and_then(|(time, encoded)| {
            serde_json::from_str::<ForecastData>(&encoded)
                .ok()
                .map(|data| (time, data))
        })
    };
    if cached
        .as_ref()
        .is_some_and(|(time, _)| Utc::now().timestamp() - time < CLIMATE_CACHE_SECONDS)
    {
        return cached;
    }
    let year = today().year();
    let result = state
        .client
        .get("https://archive-api.open-meteo.com/v1/archive")
        .timeout(std::time::Duration::from_secs(60))
        .query(&[
            ("latitude", mountain.latitude.to_string()),
            ("longitude", mountain.longitude.to_string()),
            ("timezone", mountain.timezone.clone()),
            ("start_date", format!("{}-01-01", year - 5)),
            ("end_date", format!("{}-12-31", year - 1)),
            ("models", "era5".into()),
            ("temperature_unit", "fahrenheit".into()),
            ("wind_speed_unit", "mph".into()),
            ("precipitation_unit", "inch".into()),
            (
                "daily",
                "temperature_2m_max,temperature_2m_min,snowfall_sum,wind_speed_10m_max".into(),
            ),
        ])
        .send()
        .await;
    match result {
        Ok(response) => match response.error_for_status() {
            Ok(response) => {
                if let Ok(data) = response.json::<ForecastData>().await {
                    if !data.daily.time.is_empty() {
                        let fetched_at = Utc::now().timestamp();
                        if let Ok(encoded) = serde_json::to_string(&data) {
                            if let Err(error) = state.db.lock().unwrap().execute("INSERT INTO climate_cache VALUES (?1,?2,?3) ON CONFLICT(mountain_id) DO UPDATE SET fetched_at=excluded.fetched_at,data=excluded.data",params![mountain.id,fetched_at,encoded]) {tracing::warn!(%error,"historical weather cache write failed");}
                        }
                        return Some((fetched_at, data));
                    }
                }
            }
            Err(error) => {
                tracing::warn!(%error,mountain=%mountain.id,"historical weather service unavailable")
            }
        },
        Err(error) => {
            tracing::warn!(%error,mountain=%mountain.id,"historical weather request failed")
        }
    }
    cached
}
