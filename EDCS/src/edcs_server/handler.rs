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
    ) -> anyhow::Result<EdcsResponse> {
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
                                return Ok(EdcsResponse {
                                    status: EdcsStatus::InvalidRequest as i32,
                                    payload: Some(edcs_response::Payload::InvalidRequestData(
                                        "The given payload is not of type SetupEdcsParams"
                                            .to_string(),
                                    )),
                                })
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
            | EdcsMessageType::CloseStream => {
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
                                    _ => return Ok(EdcsResponse {
                                        status: EdcsStatus::InvalidRequest as i32,
                                        payload: Some(edcs_response::Payload::InvalidRequestData(
                                            "The given payload is not of type SetupEdcsParams"
                                                .to_string(),
                                        )),
                                    }),
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
        Ok(EdcsResponse {
            status: edcs_status as i32, // NOTE is there a better way to do this?
            payload: response_payload,
        })
    }
}
