mod app;
mod bilibili;
mod cli;
mod config;
mod coyote;
mod engine;
mod http;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = cli::Cli::parse();

    let config_path = if let Ok(env_path) = std::env::var("CONFIG_PATH") {
        if !env_path.is_empty() {
            if std::path::Path::new(&env_path).is_absolute() {
                env_path
            } else {
                format!("{}/{}", std::env::current_dir()?.display(), env_path)
            }
        } else {
            cli.config
        }
    } else {
        cli.config
    };

    let state_path = if let Ok(env_path) = std::env::var("STATE_PATH") {
        if !env_path.is_empty() {
            if std::path::Path::new(&env_path).is_absolute() {
                env_path
            } else {
                format!("{}/{}", std::env::current_dir()?.display(), env_path)
            }
        } else {
            cli.state
        }
    } else {
        cli.state
    };

    let app = app::App::init(&config_path, &state_path).await?;
    app.run().await?;

    Ok(())
}
