use super::config::EdcsConfig;
use super::edcs_proto_capnp;
use super::handler;
use super::handler::EdcsHandler;
use anyhow::anyhow;
use anyhow::Context;
use capnp::message::ReaderOptions;
use capnp_futures::serialize;
use edcs_proto_capnp::edcs_protocol;
use log::debug;
use log::info;
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::io::{split, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::rustls::{self, Certificate, PrivateKey};
use tokio_rustls::TlsAcceptor;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
// Somewhat inspired by https://github.com/tokio-rs/tls/blob/master/tokio-rustls/examples/server/src/main.rs

// get_certs and get_keys are directly copied from the tokio-rs codebase since they are just boilerplate.
fn get_certs(path: &Path) -> anyhow::Result<Vec<Certificate>> {
    certs(&mut io::BufReader::new(
        fs::File::open(path).with_context(|| "Failed to open cert file")?,
    ))
    .with_context(|| "Could not get certs from cert file")
    .map(|mut certs| certs.drain(..).map(Certificate).collect())
}
fn get_keys(path: &Path) -> anyhow::Result<Vec<PrivateKey>> {
    pkcs8_private_keys(&mut io::BufReader::new(
        fs::File::open(path).with_context(|| "Failed to open key file")?,
    ))
    .with_context(|| "Could not get keys from key file")
    .map(|mut keys| keys.drain(..).map(PrivateKey).collect())
}

#[tokio::main]
pub async fn start(config_file_path: PathBuf) -> anyhow::Result<()> {
    let edcs_config: Arc<EdcsConfig> = Arc::new(toml::from_str(
        &fs::read_to_string(config_file_path).with_context(|| "Failed to read EDCS config file")?,
    )?);

    let mut keys = get_keys(&edcs_config.key_path)?;
    let certs = get_certs(&edcs_config.cert_path)?;

    debug!("number of keys == {}", keys.len());
    debug!("number of certs == {}", certs.len());
    if keys.len() == 0 {
        return Err(anyhow!(
            "Zero private keys were found in the file, bailing."
        ));
    }
    if certs.len() == 0 {
        return Err(anyhow!(
            "Zero certificate keys were found in the file, bailing."
        ));
    }

    let s_config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth() // TODO: What does this mean exactly? Documentation is a bit vague
        .with_single_cert(certs, keys.remove(0))
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let acceptor = TlsAcceptor::from(Arc::new(s_config));
    let listener =
        TcpListener::bind(edcs_config.ip.to_string() + ":" + &edcs_config.port.to_string()).await?;
    let handler = Arc::new(Mutex::new(EdcsHandler::default()));

    info!("Server bound and main loop starting");
    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let acceptor = acceptor.clone();

        let cfg_copy = Arc::clone(&edcs_config);
        let handler_copy = Arc::clone(&handler);
        let handle_future = async move {
            debug!("Received connection from peer");

            let mut stream = acceptor.accept(stream).await?;
            let (reader, mut writer) = split(stream);
            // Handle things with stream.read_buf/write_buf

            let message_reader = serialize::read_message(reader.compat(), ReaderOptions::default())
                .await
                .map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Failed to deserialize the EDCS message + payload: {:#?}", e),
                    )
                })?;

            let edcs_message = message_reader
                .get_root::<edcs_protocol::edcs_message::Reader>()
                .map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Failed to get root of EDCS message: {:#?}", e),
                    )
                })?;

            {
                // So that the locked mutex gets unlocked when it goes out of scope
                let mut handler_unlock = handler_copy.lock().map_err(|e| {
                    io::Error::new(io::ErrorKind::Other, "Failed to unlock EDCS handler mutex")
                })?;
                let edcs_response = handler_unlock
                    .handle_message(cfg_copy, edcs_message)
                    .map_err(|e| {
                        io::Error::new(
                            io::ErrorKind::InvalidData, // what error kind are we exactly supposed to use here?
                            format!("Failed to get EDCS response: {:#?}", e),
                        )
                    })?;

                // Write the response data after flushing the writer
                writer.flush().await?;
                serialize::write_message(writer.compat_write(), &edcs_response)
                    .await
                    .map_err(|e| {
                        io::Error::new(
                            io::ErrorKind::Other,
                            format!("Failed to write the EDCS response to the client {:#?}", e),
                        )
                    })?;
            }

            Ok(()) as io::Result<()>
        };
    }
}
