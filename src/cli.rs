use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "bilive-coyote",
    about = "Bilibili live gift to DG-LAB Coyote strength LAN bridge"
)]
pub struct Cli {
    #[arg(long, env = "CONFIG_PATH", default_value = "config.json")]
    pub config: PathBuf,

    #[arg(long, env = "STATE_PATH", default_value = "state.json")]
    pub state: PathBuf,
}
