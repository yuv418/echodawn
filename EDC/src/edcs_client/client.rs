use anyhow::{anyhow, Context};
use log::trace;
use prost::{decode_length_delimiter, encode_length_delimiter, Message};
use tokio::io::{split, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::{self, OwnedTrustAnchor};
use tokio_rustls::TlsConnector;

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use std::sync::Arc;

use crate::edcs_client::edcs_proto::{
    edcs_message, edcs_mouse_event, EdcsCalParams, EdcsKeyData, EdcsKeyboardEvent, EdcsMessage,
    EdcsMessageType, EdcsMouseButton, EdcsMouseEvent, EdcsMouseMove, EdcsResponse, EdcsStatus,
    EdcsStreamParams,
};
use crate::edcs_config::ClientConfig;

struct NoCertVerify {}
impl rustls::client::ServerCertVerifier for NoCertVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

#[derive(Debug)]
pub struct EdcsClient {
    reader: ReadHalf<TlsStream<TcpStream>>,
    writer: WriteHalf<TlsStream<TcpStream>>,
    delimiter_buf: Vec<u8>,
}

unsafe impl Send for EdcsClient {}

impl EdcsClient {
    pub async fn new(client_options: ClientConfig) -> anyhow::Result<Self> {
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
        Ok(Self {
            reader,
            writer,
            delimiter_buf: vec![],
        })
    }

    // Handle sending RPCs to the EDCS
    async fn send_message(
        &mut self,
        msg: EdcsMessage,
        ignore_response: bool,
    ) -> anyhow::Result<EdcsResponse> {
        // Write length delimiter first
        self.delimiter_buf.clear();
        encode_length_delimiter(msg.encoded_len(), &mut self.delimiter_buf)?;

        // Pad the delimiter buffer so it is 10 bytes in length
        self.delimiter_buf.resize(10, 0);

        self.writer.write_all(&mut self.delimiter_buf).await?;

        trace!(
            "Writing length delimiter {:?}, encoded_len is {}",
            self.delimiter_buf,
            msg.encoded_len()
        );

        // We write the delimiter separately, so we don't need to encode with delimiter.
        let mut msg_buf = msg.encode_to_vec();
        self.writer.write_all(&mut msg_buf).await?;
        self.writer.flush().await?;
        trace!("Wrote data {:?} to PB", msg_buf);

        if !ignore_response {
            let mut resp_len = 0;
            // Read response delimiter
            while let Ok(_) = self.reader.read_exact(&mut self.delimiter_buf).await {
                trace!("Read delimiter buf {:?}", self.delimiter_buf);
                trace!("resp_len = {}", resp_len);

                resp_len = decode_length_delimiter(&self.delimiter_buf[..])?;
                let mut resp_buf = vec![0; resp_len];
                while let Ok(_) = self.reader.read_exact(&mut resp_buf).await {
                    trace!("EDCS response data {:?}", &resp_buf[..]);
                    return EdcsResponse::decode(&resp_buf[..])
                        .with_context(|| "Failed to parse EDCS response");
                }
            }
            trace!("Resp len is {}", resp_len);
        } else {
            return Ok(EdcsResponse {
                status: EdcsStatus::Ok as i32,
                payload: None,
            });
        }

        Err(anyhow!("Did not read EDCS response"))
    }

    pub async fn setup_edcs(
        &mut self,
        framerate: u32,
        bitrate: u32,
    ) -> anyhow::Result<EdcsResponse> {
        self.send_message(
            EdcsMessage {
                message_type: EdcsMessageType::SetupEdcs as i32,
                payload: Some(edcs_message::Payload::SetupEdcsParams(EdcsStreamParams {
                    framerate,
                    bitrate,
                })),
            },
            false,
        )
        .await
    }
    pub async fn setup_stream(
        &mut self,
        cal_option_dict: HashMap<String, String>,
    ) -> anyhow::Result<EdcsResponse> {
        self.send_message(
            EdcsMessage {
                message_type: EdcsMessageType::SetupStream as i32,
                payload: Some(edcs_message::Payload::SetupStreamParams(EdcsCalParams {
                    cal_option_dict,
                })),
            },
            false,
        )
        .await
    }
    pub async fn init_stream(&mut self) -> anyhow::Result<EdcsResponse> {
        self.send_message(
            EdcsMessage {
                message_type: EdcsMessageType::StartStream as i32,
                payload: None,
            },
            false,
        )
        .await
    }

    pub async fn close_stream(&mut self) -> anyhow::Result<EdcsResponse> {
        self.send_message(
            EdcsMessage {
                message_type: EdcsMessageType::CloseStream as i32,
                payload: None,
            },
            false,
        )
        .await
    }
    pub async fn write_mouse_move(&mut self, x: f64, y: f64) -> anyhow::Result<EdcsResponse> {
        trace!("Writing mouse move!");
        let ret = self
            .send_message(
                EdcsMessage {
                    message_type: EdcsMessageType::WriteMouseEvent as i32,
                    payload: Some(edcs_message::Payload::MouseEvent(EdcsMouseEvent {
                        payload: Some(edcs_mouse_event::Payload::Move(EdcsMouseMove { x, y })),
                    })),
                },
                false,
            )
            .await;

        trace!("finished writing mouse move {:?}", ret);
        ret
    }
    pub async fn write_mouse_button(
        &mut self,
        btn_typ: EdcsMouseButton,
        pressed: bool,
    ) -> anyhow::Result<EdcsResponse> {
        let ret = self
            .send_message(
                EdcsMessage {
                    message_type: EdcsMessageType::WriteMouseEvent as i32,
                    payload: Some(edcs_message::Payload::MouseEvent(EdcsMouseEvent {
                        payload: Some(edcs_mouse_event::Payload::Button(EdcsKeyData {
                            btn_typ: btn_typ as i32,
                            pressed,
                        })),
                    })),
                },
                true,
            )
            .await;
        trace!("finished writing mouse button {:?}", ret);
        ret
    }
    // Using the struct wholesale here seems a bit inconsistent with the other functions
    pub async fn write_keyboard_event(
        &mut self,
        key_typ: i32,
        pressed: bool,
    ) -> anyhow::Result<EdcsResponse> {
        let ret = self
            .send_message(
                EdcsMessage {
                    message_type: EdcsMessageType::WriteKeyboardEvent as i32,
                    payload: Some(edcs_message::Payload::KeyboardEvent(EdcsKeyboardEvent {
                        key_dat: Some(EdcsKeyData {
                            btn_typ: key_typ,
                            pressed,
                        }),
                    })),
                },
                true,
            )
            .await;
        trace!("finished writing mouse button {:?}", ret);
        ret
    }
}
