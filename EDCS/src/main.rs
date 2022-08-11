extern crate edcs;

use std::{io::Write, path::PathBuf};

use clap::Parser;
use edcs::edcs_server::server;
use log::{error, info, warn};

#[derive(Parser, Debug)]
struct CLIArgs {
    #[clap(short, long, default_value = "edcsConfig.toml")]
    config_file: PathBuf,
}

fn main() {
    let args = CLIArgs::parse();
    // https://stackoverflow.com/questions/61810740/log-source-file-and-line-numbers

    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Trace)
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

    info!("Starting EDCS server");

    match server::start(args.config_file) {
        Ok(()) => info!("EDCS exited successfully."),
        Err(e) => {
            error!("EDCS failed to start: {:?}", e);
        }
    };
}
