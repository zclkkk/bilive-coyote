use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "bilive-coyote",
    about = "Bilibili live gift to DG-LAB Coyote strength LAN bridge"
)]
pub struct Cli {
    #[arg(long, default_value = "config.json")]
    pub config: String,

    #[arg(long, default_value = "state.json")]
    pub state: String,
}
