use super::config::EdcsConfig;
use super::edcs_proto::EdcsMessage;
use super::handler::EdcsHandler;
use anyhow::anyhow;
use anyhow::Context;
use log::debug;
use log::error;
use log::info;
use log::trace;
use prost::decode_length_delimiter;
use prost::encode_length_delimiter;
use prost::length_delimiter_len;
use prost::Message;
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::{split, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_rustls::rustls::{self, Certificate, PrivateKey};
use tokio_rustls::TlsAcceptor;
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

        info!("Received connection from peer with address {}", peer_addr);

        let acceptor = acceptor.clone();

        let handler_copy = Arc::clone(&handler);
        let cfg_copy = Arc::clone(&edcs_config);
        let handle_future = async move {
            let stream = acceptor.accept(stream).await?;
            let (mut reader, mut writer) = split(stream);
            // Handle things with stream.read_buf/write_buf

            // Get the length delimiter
            let mut delimiter_buf: Vec<u8> = vec![0; 10]; // I hate heap allocations
            while let Ok(sz) = reader.read_exact(&mut delimiter_buf).await {
                let pb_len = decode_length_delimiter(&delimiter_buf[..])
                    .with_context(|| "Failed to decode the protobuf length delimiter")?;

                trace!(
                    "Protobuf length delimiter slice is {:?}, size read is {}, pb_len is {}",
                    &delimiter_buf[..],
                    sz,
                    pb_len
                );
                let mut message_buffer = vec![0; pb_len];
                while let Ok(_) = reader.read_exact(&mut message_buffer).await {
                    trace!("Protobuf data {:?}", message_buffer);
                    let edcs_message = EdcsMessage::decode(&message_buffer[..])?;
                    debug!("EDCS Message {:#?}", edcs_message);

                    {
                        // So that the locked mutex gets unlocked when it goes out of scope
                        let mut handler_unlock = handler_copy.lock().await;
                        let edcs_response = handler_unlock
                            .handle_message(Arc::clone(&cfg_copy), edcs_message)
                            .with_context(|| "Failed to get EDCS response")?;

                        // Write the response data after flushing the writer
                        writer.flush().await?;
                        // For performance reasons, not all requests return a response since it would be
                        // unnecessary to respond to a mouse move event.
                        if let Some(edcs_response) = edcs_response {
                            // TODO is this the most efficient way to do this?
                            // The first ten bytes are the length delimiter, then the next bit is the actual protobuf
                            let encoded_len = edcs_response.encoded_len();
                            let mut send_delimiter_buf: Vec<u8> = vec![];
                            encode_length_delimiter(
                                edcs_response.encoded_len(),
                                &mut send_delimiter_buf,
                            )?;
                            // Pad the buffer.
                            while send_delimiter_buf.len() < 10 {
                                send_delimiter_buf.push(0);
                            }
                            trace!("Delimiter buffer is {:?}", send_delimiter_buf);
                            trace!("Response length should be {}", edcs_response.encoded_len());
                            trace!(
                                "Need {} bytes for delim buffer",
                                length_delimiter_len(encoded_len)
                            );
                            writer.write_all(&mut send_delimiter_buf).await?;

                            let mut response_buffer = edcs_response.encode_to_vec();
                            trace!("Response buffer is {:?}", response_buffer);
                            writer.write_all(&mut response_buffer).await?;
                        }
                    }
                    break;
                }
            }

            debug!("Finished RPC handler.");
            Ok(()) as anyhow::Result<()>
        };
        tokio::spawn(async move {
            if let Err(e) = handle_future.await {
                error!("Future handle (main loop) failed with {:?}", e)
            }
        });
    }
}
