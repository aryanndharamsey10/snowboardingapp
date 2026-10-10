#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "weekend_club=info,tower_http=info".into()),
        )
        .init();
    let database =
        std::env::var("DATABASE_PATH").unwrap_or_else(|_| "data/weekend-club.sqlite".into());
    if let Some(parent) = std::path::Path::new(&database)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let state = weekend_club::AppState::new(&database)?;
    tokio::spawn(weekend_club::poll_weather(state.clone()));
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".into());
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    println!("Weekend Club is ready at http://{host}:{port}");
    axum::serve(listener, weekend_club::app(state, &static_dir))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = interrupt => {}, _ = terminate => {} }
}
