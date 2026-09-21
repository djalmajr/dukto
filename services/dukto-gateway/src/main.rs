use std::{net::SocketAddr, sync::Arc};

use dukto_gateway::{router, GatewayState, SystemClock};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let bind: SocketAddr = std::env::var("DUKTO_GATEWAY_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8090".to_owned())
        .parse()?;
    let allow_proxy_bind = matches!(
        std::env::var("DUKTO_GATEWAY_ALLOW_PROXY_BIND").as_deref(),
        Ok("1" | "true")
    );
    if !bind.ip().is_loopback() && !allow_proxy_bind {
        return Err("gateway non-loopback bind requires an explicit proxy-bind opt-in".into());
    }
    let relay_bearer = std::env::var("DUKTO_GATEWAY_RELAY_BEARER_TOKEN")?;
    let state = GatewayState::new(relay_bearer, Arc::new(SystemClock))?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, "dukto gateway listening");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
