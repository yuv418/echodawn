use std::sync::Arc;

use log::{debug, info};

use super::config::{self, EdcsConfig};
use super::edcs_proto::{
    edcs_message, edcs_response, EdcsMessage, EdcsMessageType, EdcsResponse, EdcsSetupEdcsData,
    EdcsSetupStreamData, EdcsStatus, EdcsStreamParams,
};
use crate::edss_safe::edss::EdssAdapter;

#[derive(Debug, Default)]
pub struct EdcsHandler {
    adapter: Option<EdssAdapter>,
}

impl EdcsHandler {
    pub fn handle_message(
        &mut self,
        cfg: Arc<config::EdcsConfig>,
        msg: EdcsMessage,
        // Some of the events (eg. keyboard/mouse) won't return a response since we don't want to waste time sending back thousands of "Ok" messages
        // In the future if we want to find failed K/M events we can create a separate request for that
    ) -> anyhow::Result<Option<EdcsResponse>> {
        // Handle the input

        let mut response_payload = None;
        let mut edcs_status = EdcsStatus::Ok;

        match msg.message_type() {
            EdcsMessageType::SetupEdcs => {
                // Initialize EDSS, don't start stream.

                match self.adapter {
                    None => {
                        debug!("HANDLER Setting up EDCS.");
                        let stream_params = match msg.payload {
                            Some(edcs_message::Payload::SetupEdcsParams(p)) => p,
                            _ => {
                                return Ok(Some(EdcsResponse {
                                    status: EdcsStatus::InvalidRequest as i32,
                                    payload: Some(edcs_response::Payload::InvalidRequestData(
                                        "The given payload is not of type SetupEdcsParams"
                                            .to_string(),
                                    )),
                                }))
                            }
                        };

                        // TODO autogenerate a random key and return it through the response.
                        match EdssAdapter::new(
                            cfg.edss_config.plugin_name.clone(),
                            cfg.edss_config.ip,
                            cfg.edss_config.port,
                            stream_params.bitrate,
                            stream_params.framerate,
                        ) {
                            Ok(adapter) => {
                                self.adapter = Some(adapter);
                                response_payload = Some(edcs_response::Payload::SetupEdcsData(
                                    EdcsSetupEdcsData {
                                        cal_option_dict: self
                                            .adapter
                                            .as_ref()
                                            .unwrap()
                                            .cal_option_dict
                                            .clone(), // unwrap will never fail here
                                    },
                                ));
                            }
                            Err(e) => {
                                edcs_status = EdcsStatus::EdssErr;
                                response_payload = Some(edcs_response::Payload::EdssErrData(e.0));
                            }
                        };
                        debug!("HANDLER Finished setting up stream.");
                    }
                    Some(_) => {
                        edcs_status = EdcsStatus::EdcsAlreadySetup;
                    }
                }
            }
            EdcsMessageType::SetupStream
            | EdcsMessageType::StartStream
            | EdcsMessageType::CloseStream
            | EdcsMessageType::WriteMouseEvent
            | EdcsMessageType::WriteKeyboardEvent => {
                // TODO: DRY here

                if let Some(adapter) = &mut self.adapter {
                    match msg.message_type() {
                        EdcsMessageType::SetupStream => {
                            if !adapter.stream_setup() {
                                adapter.cal_option_dict = match msg.payload {
                                    Some(edcs_message::Payload::SetupStreamParams(d)) => {
                                        d.cal_option_dict
                                    }
                                    // TODO keep it dry (we will have to check requestss for all message types)
                                    _ => return Ok(Some(EdcsResponse {
                                        status: EdcsStatus::InvalidRequest as i32,
                                        payload: Some(edcs_response::Payload::InvalidRequestData(
                                            "The given payload is not of type SetupEdcsParams"
                                                .to_string(),
                                        )),
                                    })),
                                };
                                match adapter.init_server() {
                                    Ok(_) => {
                                        response_payload =
                                            Some(edcs_response::Payload::SetupStreamData(
                                                EdcsSetupStreamData {
                                                    out_stream_params: Some(EdcsStreamParams {
                                                        framerate: adapter.framerate,
                                                        bitrate: adapter.bitrate,
                                                    }),
                                                    sdp: adapter.sdp.clone().unwrap(), // Guaranteed to be Some at this point
                                                },
                                            ))
                                    }
                                    Err(e) => {
                                        edcs_status = EdcsStatus::EdssErr;
                                        response_payload =
                                            Some(edcs_response::Payload::EdssErrData(e.0));
                                    }
                                }
                            } else {
                                edcs_status = EdcsStatus::StreamAlreadySetup;
                            }
                        }
                        EdcsMessageType::StartStream => {
                            if !adapter.streaming() {
                                info!("Initialising streaming");
                                match adapter.init_streaming() {
                                    Ok(_) => {
                                        response_payload = None;
                                    }
                                    Err(e) => {
                                        edcs_status = EdcsStatus::EdssErr;
                                        response_payload =
                                            Some(edcs_response::Payload::EdssErrData(e.0));
                                    }
                                }
                            } else {
                                edcs_status = EdcsStatus::StreamAlreadyStarted;
                            }
                        }
                        EdcsMessageType::CloseStream => {
                            // TODO there is a bug here --> we need to figure out how to handle an already setup EDCS/stream that hasn't
                            // started streaming, since EDSS will try to free invalid pointers in some cases.
                            // It may be a good idea to free resources on client disconnect.
                            if adapter.streaming() {
                                match adapter.close_streaming() {
                                    Err(e) => {
                                        edcs_status = EdcsStatus::EdssErr;
                                        response_payload =
                                            Some(edcs_response::Payload::EdssErrData(e.0));
                                    }
                                    Ok(_) => {
                                        // No more adapter, I guess
                                        // When we having multiple clients connec to the same server, we will change this
                                        self.adapter = None;
                                    }
                                }
                            } else {
                                // Edcs might be setup, who knows
                                edcs_status = EdcsStatus::StreamNotStarted;
                            }
                        }
                        // TODO for both of these events, we can make the EDSS/EDCS return void
                        EdcsMessageType::WriteMouseEvent => {
                            if adapter.streaming() {
                                match adapter.write_mouse_event(match msg.payload {
                                    Some(edcs_message::Payload::MouseEvent(mev)) => mev,
                                    _ => return Ok(None),
                                }) {
                                    Err(e) => {
                                        edcs_status = EdcsStatus::EdssErr;
                                        response_payload =
                                            Some(edcs_response::Payload::EdssErrData(e.0));
                                    }
                                    Ok(_) => response_payload = None,
                                }
                            } else {
                                edcs_status = EdcsStatus::StreamNotStarted;
                            }
                        }
                        EdcsMessageType::WriteKeyboardEvent => {
                            if adapter.streaming() {
                                match adapter.write_keyboard_event(match msg.payload {
                                    Some(edcs_message::Payload::KeyboardEvent(kev)) => kev,
                                    _ => return Ok(None),
                                }) {
                                    Err(e) => {
                                        edcs_status = EdcsStatus::EdssErr;
                                        response_payload =
                                            Some(edcs_response::Payload::EdssErrData(e.0));
                                    }
                                    Ok(_) => response_payload = None,
                                }
                            } else {
                                edcs_status = EdcsStatus::StreamNotStarted;
                            }
                        }
                        _ => {}
                    };
                } else {
                    edcs_status = EdcsStatus::UninitialisedEdss;
                }
            }
            EdcsMessageType::UpdateStream => {
                todo!()
            }
        }

        // Send out the response
        Ok(Some(EdcsResponse {
            status: edcs_status as i32, // NOTE is there a better way to do this?
            payload: response_payload,
        }))
    }
}
