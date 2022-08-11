use anyhow::Context;
use capnp::message::{self, ReaderOptions};
use capnp::serialize::OwnedSegments;
use capnp_futures::serialize;
use tokio::io::{split, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::{self, OwnedTrustAnchor};
use tokio_rustls::TlsConnector;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use crate::edc_client::{
    config::ClientConfig,
    edcs_proto_capnp::edcs_protocol::{edcs_message, EdcsMessageType},
};

pub struct EdcClient {
    reader: Compat<ReadHalf<TlsStream<TcpStream>>>,
    writer: Compat<WriteHalf<TlsStream<TcpStream>>>,
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

        let config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth(); // again TODO, what does this mean?

        let connector = TlsConnector::from(Arc::new(config));
        let stream = TcpStream::connect(
            client_options.host.to_string() + ":" + &client_options.port.to_string(),
        )
        .await
        .with_context(|| "Failed to set up TCP stream")?;

        // Domain can also be an IP address, I think
        let domain = rustls::ServerName::try_from(client_options.host.to_string().as_str())
            .with_context(|| "Failed to get the TLS server name")?;
        let stream = connector
            .connect(domain, stream)
            .await
            .with_context(|| "Failed to connect to the EDCS server")?;

        let (reader, writer) = split(stream);
        Ok(EdcClient {
            reader: reader.compat(),
            writer: writer.compat_write(),
        })
    }

    // Handle sending RPCs to the EDCS
    async fn send_message(
        &mut self,
        msg: message::Builder<message::HeapAllocator>,
    ) -> anyhow::Result<message::Reader<OwnedSegments>> {
        serialize::write_message(&mut self.writer, &msg)
            .await
            .with_context(|| "Failed to write serialized message to EDCS wire")?;
        serialize::read_message(&mut self.reader, ReaderOptions::default())
            .await
            .with_context(|| "Failed to read response from EDCS")
    }

    pub async fn setup_stream(
        &mut self,
        framerate: u32,
        bitrate: u32,
    ) -> anyhow::Result<message::Reader<OwnedSegments>> {
        let mut response = message::Builder::new_default();
        let mut message: edcs_message::Builder = response.init_root();
        message.set_message_type(EdcsMessageType::SetupStream);
        let mut setup_stream_params = message.init_payload().init_setup_stream_params();
        setup_stream_params.set_bitrate(bitrate);
        setup_stream_params.set_framerate(framerate);
        self.send_message(response).await
    }
}
