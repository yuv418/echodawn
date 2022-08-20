// Based on the example provided here: https://tokio.rs/tokio/topics/bridging#a-synchronous-interface-to-mini-redis

use crate::edcs_client::{
    self,
    client::EdcsClient,
    edcs_proto::{EdcsKeyData, EdcsMouseButton, EdcsResponse},
};
use anyhow::anyhow;
use async_mutex::Mutex;
use flume::{Receiver, Sender};
use futures::TryFutureExt;
use log::{debug, trace};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use thread_priority::{RealtimeThreadSchedulePolicy, ThreadPriority, ThreadSchedulePolicy};
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
    WriteKeyboardEvent {
        key_typ: i32,
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
        let (ui_send, client_recv) = flume::bounded(1); // channel(32);
        let (client_send, ui_recv) = flume::unbounded(); // channel(32);

        // No client until it's requested
        let mut client = Self {
            push: ui_send,
            recv: ui_recv,
        };
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime");

        std::thread::spawn(move || {
            let edcs_client: Arc<Mutex<Option<EdcsClient>>> = Arc::new(Mutex::new(None));
            runtime.block_on(async move {
                for req in client_recv.iter() {
                    tokio::spawn(Self::handle_req(
                        req,
                        edcs_client.clone(),
                        client_send.clone(),
                    ))
                    .await;
                }
            });
        });

        client
    }

    pub async fn handle_req(
        req: ChannelEdcsRequest,
        edcs_client_lck: Arc<Mutex<Option<EdcsClient>>>,
        client_push: Sender<ChannelEdcsResponse>,
    ) {
        debug!("client req: {:?}", req);
        let mut edcs_client_opt = edcs_client_lck.lock().await;
        let resp = match req {
            // TODO DRY
            ChannelEdcsRequest::SetupEdcs { .. }
            | ChannelEdcsRequest::SetupStream(_)
            | ChannelEdcsRequest::StartStream
            | ChannelEdcsRequest::CloseStream
            | ChannelEdcsRequest::WriteMouseButton { .. }
            | ChannelEdcsRequest::WriteMouseMove { .. }
            | ChannelEdcsRequest::WriteKeyboardEvent { .. } => {
                let ret = if let Some(edcs_client) = &mut *edcs_client_opt {
                    match req {
                        ChannelEdcsRequest::SetupEdcs { bitrate, framerate } => {
                            ChannelEdcsResponse::EdcsResponse(
                                edcs_client.setup_edcs(framerate, bitrate).await,
                            )
                        }
                        ChannelEdcsRequest::SetupStream(ref options) => {
                            ChannelEdcsResponse::EdcsResponse(
                                edcs_client.setup_stream(options.clone()).await,
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
                                trace!("finished writing mouse move {:?}", ret);
                                ret
                            })
                        }
                        ChannelEdcsRequest::WriteMouseButton {
                            button_typ,
                            pressed,
                        } => ChannelEdcsResponse::EdcsResponse({
                            let ret = edcs_client.write_mouse_button(button_typ, pressed).await;
                            debug!("write mouse button finished");
                            ret
                        }),
                        ChannelEdcsRequest::WriteKeyboardEvent { key_typ, pressed } => {
                            ChannelEdcsResponse::EdcsResponse({
                                edcs_client.write_keyboard_event(key_typ, pressed).await
                            })
                        }
                    }
                } else {
                    ChannelEdcsResponse::InvalidClient
                };

                if let ChannelEdcsRequest::WriteMouseMove { .. } = &req {
                } else {
                    trace!("pushed response {:?} to ui", client_push.send(ret));
                }
            }

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
        trace!("dropping mutex");
        std::mem::drop(edcs_client_opt);
        resp
    }
}
