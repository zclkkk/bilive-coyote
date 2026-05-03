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

    let app = app::App::init(&cli.config, &cli.state).await?;
    app.run().await?;

    Ok(())
}
