extern crate edc;

use std::io::Write;
use std::path::PathBuf;

use clap::Parser;

use edc::edc_client::client::EdcClient;
use log::info;

#[derive(Parser, Debug)]
struct CLIArgs {
    #[clap(short, long, default_value = "edcConfig.toml")]
    config_file_path: PathBuf,
}

// Mostly copied from https://github.com/tokio-rs/tls/blob/master/tokio-rustls/examples/client/src/main.rs (I mean… it's all boilerplate anyway)
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CLIArgs::parse();
    // TODO share code between EDCS and EDC for these kinds of things
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Debug)
        .format(|buf, record| {
            use env_logger::fmt::Color;
            use log::Level;

            let mut level_style = buf.style();
            match record.level() {
                Level::Trace => level_style.set_bg(Color::Cyan),
                Level::Error => level_style.set_bg(Color::Red),
                Level::Warn => level_style.set_bg(Color::Yellow),
                Level::Info => level_style.set_bg(Color::Blue),
                Level::Debug => level_style.set_bg(Color::Magenta),
            };
            level_style.set_color(Color::Black);

            writeln!(
                buf,
                "[{} {} {}:{}] {}",
                level_style.value(record.level()),
                chrono::Local::now().format("%Y-%m-%dT%H:%M%SZ"),
                record.file().unwrap_or("unknown file"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();

    info!("Starting up client!");
    let mut client = EdcClient::new(&args.config_file_path).await?;
    info!("Client connected to server");
    let response = client.setup_stream(60, 100000).await?;
    info!("Client setup stream returned response {:#?}", response);

    Ok(())
}