extern crate edc;

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use clap::Parser;

use edc::edc_client::client::EdcClient;
use edc::edc_client::edcs_proto::{edcs_response, EdcsMessageType, EdcsResponse};
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
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%SZ"),
                record.file().unwrap_or("unknown file"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();

    info!("Starting up client!");
    let mut client = EdcClient::new(&args.config_file_path).await?;
    info!("Client connected to server");

    let response = client.setup_edcs(60, 10000000).await?;
    info!("Client setup EDCS returned response {:#?}", response);

    let mut data_map = match response.payload {
        Some(edcs_response::Payload::SetupEdcsData(m)) => m.cal_option_dict,
        _ => panic!("Invalid response payload"),
    };
    data_map.insert("vgpuId".to_string(), "2".to_string());

    let response = client.setup_stream(data_map).await?;
    info!("Client setup stream returned response {:#?}", response);

    if let Some(edcs_response::Payload::SetupStreamData(data)) = response.payload {
        std::fs::write("test.sdp", data.sdp);
        Command::new("mpv")
            .arg("--profile=low-latency")
            .arg("--rtsp-transport=lavf")
            .arg("--hwdec=yes")
            .arg("--untimed")
            .arg("--no-demuxer-thread")
            .arg("--vd-lavc-threads=1")
            .arg("--video-latency-hacks=yes")
            .arg("test.sdp")
            .spawn()
            .expect("Failed to spawn mpv");
        // tokio::time::sleep(Duration::from_millis(5000)).await;
        let response = client.init_stream().await?;
        info!("Client setup stream returned response {:#?}", response);
    }

    Ok(())
}
