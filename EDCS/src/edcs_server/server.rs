use super::config::EdcsConfig;
use super::edcs_proto_capnp;
use super::handler;
use capnp::message::ReaderOptions;
use capnp_futures::serialize;
use edcs_proto_capnp::edcs_protocol;
use rustls_pemfile::{certs, rsa_private_keys};
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::io::{copy, sink, split, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::rustls::{self, Certificate, PrivateKey};
use tokio_rustls::TlsAcceptor;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tokio_util::io::StreamReader;

// Somewhat inspired by https://github.com/tokio-rs/tls/blob/master/tokio-rustls/examples/server/src/main.rs

// get_certs and get_keys are directly copied from the tokio-rs codebase since they are just boilerplate.
fn get_certs(path: &Path) -> io::Result<Vec<Certificate>> {
    certs(&mut io::BufReader::new(fs::File::open(path)?))
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid certificate path provided",
            )
        })
        .map(|mut certs| certs.drain(..).map(Certificate).collect())
}
fn get_keys(path: &Path) -> io::Result<Vec<PrivateKey>> {
    certs(&mut io::BufReader::new(fs::File::open(path)?))
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid private key path provided",
            )
        })
        .map(|mut certs| certs.drain(..).map(PrivateKey).collect())
}

#[tokio::main]
async fn start() -> io::Result<()> {
    let edcs_config: Arc<EdcsConfig> = Arc::new(toml::from_str(
        &fs::read_to_string(
            env::var("EDCS_CONFIG_FILE").expect("Failed to get the EDCS config file"),
        )
        .expect("Failed to read the EDCS config file"),
    )?);

    let mut keys = get_keys(&edcs_config.key_path)?;
    let certs = get_certs(&edcs_config.cert_path)?;

    let s_config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth() // TODO: What does this mean exactly? Documentation is a bit vague
        .with_single_cert(certs, keys.remove(0))
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let acceptor = TlsAcceptor::from(Arc::new(s_config));
    let listener =
        TcpListener::bind(edcs_config.ip.to_string() + &edcs_config.port.to_string()).await?;

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let acceptor = acceptor.clone();

        let cfg_copy = Arc::clone(&edcs_config);
        let handle_future = async move {
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

            let edcs_response = handler::handle_message(cfg_copy, edcs_message).map_err(|e| {
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

            Ok(()) as io::Result<()>
        };
    }

    Ok(())
}
