// Based on the example provided here: https://tokio.rs/tokio/topics/bridging#a-synchronous-interface-to-mini-redis

use crate::edcs_client::{
    self,
    client::EdcsClient,
    edcs_proto::{EdcsMouseButton, EdcsResponse},
};
use anyhow::anyhow;
use async_mutex::Mutex;
use flume::{Receiver, Sender};
use futures::TryFutureExt;
use log::debug;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::runtime::Builder;

// At this point, we may as well get rid of the methods in EdcsClient and just have the GUI send over the structs we want
#[derive(Debug)]
pub enum ChannelEdcsRequest {
    NewClient(PathBuf),
    SetupEdcs {
        bitrate: u32,
        framerate: u32,
    },
    SetupStream(HashMap<String, String>),
    StartStream,
    CloseStream,
    WriteMouseMove {
        x: u32,
        y: u32,
    },
    WriteMouseButton {
        button_typ: EdcsMouseButton,
        pressed: bool,
    },
}
#[derive(Debug)]
pub enum ChannelEdcsResponse {
    EdcsClientInitialised,
    EdcsClientInitError(anyhow::Error),
    InvalidClient,
    EdcsResponse(anyhow::Result<EdcsResponse>),
}

pub struct BlockingEdcsClient {
    pub push: Sender<ChannelEdcsRequest>,
    pub recv: Receiver<ChannelEdcsResponse>,
}

impl BlockingEdcsClient {
    pub fn new() -> Self {
        // There may be a lot of messages in the ring
        let (ui_send, client_recv) = flume::unbounded(); // channel(32);
        let (client_send, ui_recv) = flume::unbounded(); // channel(32);

        // No client until it's requested
        let mut client = Self {
            push: ui_send,
            recv: ui_recv,
        };
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime");

        let client_send = Arc::new(client_send);

        std::thread::spawn(move || {
            runtime.block_on(async move {
                let edcs_client: Arc<Mutex<Option<EdcsClient>>> = Arc::new(Mutex::new(None));
                while let Ok(req) = client_recv.recv() {
                    tokio::spawn(Self::handle_req(
                        req,
                        edcs_client.clone(),
                        client_send.clone(),
                    ));
                }
            })
        });

        client
    }

    pub async fn handle_req(
        req: ChannelEdcsRequest,
        edcs_client_lck: Arc<Mutex<Option<EdcsClient>>>,
        client_push: Arc<Sender<ChannelEdcsResponse>>,
    ) {
        debug!("client req: {:?}", req);
        let mut edcs_client_opt = edcs_client_lck.lock().await;
        debug!("Acquired client lock!");
        let resp = match req {
            // TODO DRY
            ChannelEdcsRequest::SetupEdcs { .. }
            | ChannelEdcsRequest::SetupStream(_)
            | ChannelEdcsRequest::StartStream
            | ChannelEdcsRequest::CloseStream
            | ChannelEdcsRequest::WriteMouseButton { .. }
            | ChannelEdcsRequest::WriteMouseMove { .. } => client_push
                .send(if let Some(edcs_client) = &mut *edcs_client_opt {
                    match req {
                        ChannelEdcsRequest::SetupEdcs { bitrate, framerate } => {
                            ChannelEdcsResponse::EdcsResponse(
                                edcs_client.setup_edcs(framerate, bitrate).await,
                            )
                        }
                        ChannelEdcsRequest::SetupStream(options) => {
                            ChannelEdcsResponse::EdcsResponse(
                                edcs_client.setup_stream(options).await,
                            )
                        }
                        ChannelEdcsRequest::StartStream => {
                            ChannelEdcsResponse::EdcsResponse(edcs_client.init_stream().await)
                        }
                        ChannelEdcsRequest::CloseStream => {
                            ChannelEdcsResponse::EdcsResponse(edcs_client.close_stream().await)
                        }
                        // Should never go here
                        ChannelEdcsRequest::NewClient(_) => panic!(),
                        ChannelEdcsRequest::WriteMouseMove { x, y } => {
                            ChannelEdcsResponse::EdcsResponse({
                                let ret = edcs_client.write_mouse_move(x, y).await;
                                debug!("Finished mouse move");
                                ret
                            })
                        }
                        ChannelEdcsRequest::WriteMouseButton {
                            button_typ,
                            pressed,
                        } => ChannelEdcsResponse::EdcsResponse(
                            edcs_client.write_mouse_button(button_typ, pressed).await,
                        ),
                    }
                } else {
                    ChannelEdcsResponse::InvalidClient
                })
                .expect("Failed to send resp to client push"),
            ChannelEdcsRequest::NewClient(path) => {
                let edcs_client_res = EdcsClient::new(&path).await;
                if let Ok(c) = edcs_client_res {
                    client_push
                        .send(ChannelEdcsResponse::EdcsClientInitialised)
                        .unwrap();
                    *edcs_client_opt = Some(c)
                } else if let Err(e) = edcs_client_res {
                    client_push
                        .send(ChannelEdcsResponse::EdcsClientInitError(e))
                        .unwrap()
                } else {
                    // The Rust compiler is amazing /s
                    panic!()
                };
            }
        };
        debug!("dropping mutex");
        std::mem::drop(edcs_client_opt);
        debug!("dropped mutex");
        resp
    }
}
