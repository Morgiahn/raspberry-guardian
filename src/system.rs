use crate::config::Config;
use log::warn;
use tokio::time::sleep;
use anyhow::Result;

pub async fn shutdown_with_grace(config: &Config) -> Result<()> {
    // Log délai de grâce
    warn!(
        "Shutdown dans {} secondes...",
        config.system.shutdown_grace_period_seconds
    );

    // Attendre le délai
    sleep(config.system.shutdown_grace_period()).await;

    // Effectuer le shutdown réel
    crate::perform_shutdown(config).await?;

    Ok(())
}