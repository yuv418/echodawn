// Based on the example provided here: https://tokio.rs/tokio/topics/bridging#a-synchronous-interface-to-mini-redis

use crate::edcs_client::{self, client::EdcsClient, edcs_proto::EdcsResponse};
use anyhow::anyhow;
use log::debug;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{
    runtime::Builder,
    sync::{mpsc, Mutex},
};

// At this point, we may as well get rid of the methods in EdcsClient and just have the GUI send over the structs we want
#[derive(Debug)]
pub enum ChannelEdcsRequest {
    NewClient(PathBuf),
    SetupEdcs { bitrate: u32, framerate: u32 },
    SetupStream(HashMap<String, String>),
    InitStream,
    CloseStream,
}
#[derive(Debug)]
pub enum ChannelEdcsResponse {
    EdcsClientInitialised,
    EdcsClientInitError(anyhow::Error),
    InvalidClient,
    EdcsResponse(anyhow::Result<EdcsResponse>),
}

pub struct BlockingEdcsClient {
    pub push: mpsc::Sender<ChannelEdcsRequest>,
    pub recv: mpsc::Receiver<ChannelEdcsResponse>,
}

impl BlockingEdcsClient {
    pub fn new() -> Self {
        // There may be a lot of messages in the ring
        let (ui_send, mut client_recv) = mpsc::channel(32);
        let (client_send, mut ui_recv) = mpsc::channel(32);

        // No client until it's requested
        let mut client = Self {
            push: ui_send,
            recv: ui_recv,
        };
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime");

        let client_send = Arc::new(client_send);

        std::thread::spawn(move || {
            runtime.block_on(async move {
                let mut edcs_client: Arc<Mutex<Option<EdcsClient>>> = Arc::new(Mutex::new(None));
                while let Some(req) = client_recv.recv().await {
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
        client_push: Arc<mpsc::Sender<ChannelEdcsResponse>>,
    ) {
        debug!("Handling incoming request {:#?}", req);
        let mut edcs_client_opt = edcs_client_lck.lock().await;
        match req {
            // TODO DRY
            ChannelEdcsRequest::SetupEdcs { .. }
            | ChannelEdcsRequest::SetupStream(_)
            | ChannelEdcsRequest::InitStream
            | ChannelEdcsRequest::CloseStream => client_push
                .send(if let Some(edcs_client) = &mut *edcs_client_opt {
                    match req {
                        ChannelEdcsRequest::SetupEdcs { bitrate, framerate } => {
                            ChannelEdcsResponse::EdcsResponse(
                                edcs_client.setup_edcs(bitrate, framerate).await,
                            )
                        }
                        ChannelEdcsRequest::SetupStream(options) => {
                            ChannelEdcsResponse::EdcsResponse(
                                edcs_client.setup_stream(options).await,
                            )
                        }
                        ChannelEdcsRequest::InitStream => {
                            ChannelEdcsResponse::EdcsResponse(edcs_client.init_stream().await)
                        }
                        ChannelEdcsRequest::CloseStream => {
                            ChannelEdcsResponse::EdcsResponse(edcs_client.close_stream().await)
                        }
                        // Should never go here
                        ChannelEdcsRequest::NewClient(_) => panic!(),
                    }
                } else {
                    ChannelEdcsResponse::InvalidClient
                })
                .await
                .unwrap(),
            ChannelEdcsRequest::NewClient(path) => {
                let edcs_client_res = EdcsClient::new(&path).await;
                if let Ok(c) = edcs_client_res {
                    client_push
                        .send(ChannelEdcsResponse::EdcsClientInitialised)
                        .await
                        .unwrap();
                    *edcs_client_opt = Some(c)
                } else if let Err(e) = edcs_client_res {
                    client_push
                        .send(ChannelEdcsResponse::EdcsClientInitError(e))
                        .await
                        .unwrap()
                } else {
                    // The Rust compiler is amazing /s
                    panic!()
                };
            }
        }
    }
}
