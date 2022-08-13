use anyhow::{anyhow, Context};
use log::{debug, trace};
use prost::{decode_length_delimiter, encode_length_delimiter, length_delimiter_len, Message};
use tokio::io::{split, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::{self, OwnedTrustAnchor};
use tokio_rustls::TlsConnector;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use crate::edc_client::{
    config::ClientConfig,
    edcs_proto::{
        edcs_message, EdcsCalParams, EdcsMessage, EdcsMessageType, EdcsResponse, EdcsStreamParams,
    },
};

struct NoCertVerify {}
impl rustls::client::ServerCertVerifier for NoCertVerify {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::Certificate,
        intermediates: &[rustls::Certificate],
        server_name: &rustls::ServerName,
        scts: &mut dyn Iterator<Item = &[u8]>,
        ocsp_response: &[u8],
        now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

pub struct EdcClient {
    reader: ReadHalf<TlsStream<TcpStream>>,
    writer: WriteHalf<TlsStream<TcpStream>>,
}

impl EdcClient {
    pub async fn new(config_file_path: &Path) -> anyhow::Result<EdcClient> {
        let client_options: ClientConfig = toml::from_str(
            &fs::read_to_string(config_file_path)
                .with_context(|| "Failed to unwrap the config file name")?,
        )
        .with_context(|| "Failed to parse client options")?;

        let mut root_cert_store = rustls::RootCertStore::empty();
        let mut pem = BufReader::new(
            File::open(client_options.cert).with_context(|| "Failed to CA cert file")?,
        );

        let certs = rustls_pemfile::certs(&mut pem)
            .with_context(|| "Failed to get certs from CA cert file")?;
        let trust_anchors = certs.iter().map(|cert| {
            let ta = webpki::TrustAnchor::try_from_cert_der(&cert[..])
                .expect("Failed to get trust anchor from cert DER"); // I'm not sure if there's a better solution to panicking here at the moment.
            OwnedTrustAnchor::from_subject_spki_name_constraints(
                ta.subject,
                ta.spki,
                ta.name_constraints,
            )
        });
        root_cert_store.add_server_trust_anchors(trust_anchors);

        let mut config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth(); // again TODO, what does this mean?

        // TODO: Trust individual certificates somehow
        if client_options.disable_tls_verification {
            config
                .dangerous()
                .set_certificate_verifier(Arc::new(NoCertVerify {}))
        }

        let connector = TlsConnector::from(Arc::new(config));
        let stream = TcpStream::connect(
            client_options.host.to_string() + ":" + &client_options.port.to_string(),
        )
        .await
        .with_context(|| "Failed to set up TCP stream")?;

        // Domain can also be an IP address, I think
        let domain = rustls::ServerName::try_from(client_options.domain.as_str())
            .with_context(|| "Failed to get the TLS server name")?;
        let stream = connector
            .connect(domain, stream)
            .await
            .with_context(|| "Failed to connect to the EDCS server")?;

        let (reader, writer) = split(stream);
        Ok(EdcClient { reader, writer })
    }

    // Handle sending RPCs to the EDCS
    async fn send_message(&mut self, msg: EdcsMessage) -> anyhow::Result<EdcsResponse> {
        let mut delimiter_buf: Vec<u8> = vec![];
        // Write length delimiter first
        encode_length_delimiter(msg.encoded_len(), &mut delimiter_buf)?;

        self.writer.flush().await?;
        // Pad the delimiter buffer so it is 10 bytes in length
        while delimiter_buf.len() < 10 {
            delimiter_buf.push(0)
        }
        self.writer.write_all(&mut delimiter_buf).await?;

        debug!(
            "Writing length delimiter {:?}, encoded_len is {}",
            delimiter_buf,
            msg.encoded_len()
        );

        // We write the delimiter separately, so we don't need to encode with delimiter.
        let mut msg_buf = msg.encode_to_vec();
        trace!("Writing data {:?} to PB", msg_buf);
        self.writer.write_all(&mut msg_buf).await?;

        let mut resp_len = 0;
        // Read response delimiter
        while let Ok(_) = self.reader.read_exact(&mut delimiter_buf).await {
            trace!("Read delimiter buf {:?}", delimiter_buf);
            trace!("resp_len = {}", resp_len);

            resp_len = decode_length_delimiter(&delimiter_buf[..])?;
            let mut resp_buf = vec![0; resp_len];
            while let Ok(_) = self.reader.read_exact(&mut resp_buf).await {
                trace!("EDCS response data {:?}", &resp_buf[..]);
                return EdcsResponse::decode(&resp_buf[..])
                    .with_context(|| "Failed to parse EDCS response");
            }
        }
        trace!("Resp len is {}", resp_len);

        Err(anyhow!("Did not read EDCS response"))
    }

    pub async fn setup_edcs(
        &mut self,
        framerate: u32,
        bitrate: u32,
    ) -> anyhow::Result<EdcsResponse> {
        self.send_message(EdcsMessage {
            message_type: EdcsMessageType::SetupEdcs as i32,
            payload: Some(edcs_message::Payload::SetupEdcsParams(EdcsStreamParams {
                framerate,
                bitrate,
            })),
        })
        .await
    }
    pub async fn setup_stream(
        &mut self,
        cal_option_dict: HashMap<String, String>,
    ) -> anyhow::Result<EdcsResponse> {
        self.send_message(EdcsMessage {
            message_type: EdcsMessageType::SetupStream as i32,
            payload: Some(edcs_message::Payload::SetupStreamParams(EdcsCalParams {
                cal_option_dict,
            })),
        })
        .await
    }
    pub async fn init_stream(&mut self) -> anyhow::Result<EdcsResponse> {
        self.send_message(EdcsMessage {
            message_type: EdcsMessageType::StartStream as i32,
            payload: None,
        })
        .await
    }

    pub async fn close_stream(&mut self) -> anyhow::Result<EdcsResponse> {
        self.send_message(EdcsMessage {
            message_type: EdcsMessageType::CloseStream as i32,
            payload: None,
        })
        .await
    }
}
