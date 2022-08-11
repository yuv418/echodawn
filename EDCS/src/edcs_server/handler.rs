use std::sync::Arc;

use log::debug;

use super::config;
use super::edcs_proto::{
    edcs_message, edcs_response, EdcsMessage, EdcsMessageType, EdcsResponse, EdcsSetupEdcsData,
    EdcsStatus,
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

                debug!("HANDLER Setting up EDCS.");

                let stream_params = match msg.payload {
                    Some(edcs_message::Payload::SetupEdcsParams(p)) => p,
                    _ => {
                        return Ok(EdcsResponse {
                            status: EdcsStatus::InvalidRequest as i32,
                            payload: Some(edcs_response::Payload::InvalidRequestData(
                                "The given payload is not of type SetupEdcsParams".to_string(),
                            )),
                        })
                    }
                };

                // TODO stop hardcoding random stuff.
                // TODO autogenerate a random key and return it through the response.
                match EdssAdapter::new(
                    cfg.edss_config.plugin_name.clone(),
                    cfg.ip,
                    cfg.port,
                    stream_params.bitrate,
                    stream_params.framerate,
                    "".to_owned(),
                ) {
                    Ok(adapter) => {
                        self.adapter = Some(adapter);
                        response_payload =
                            Some(edcs_response::Payload::SetupEdcsData(EdcsSetupEdcsData {
                                cal_option_dict: self
                                    .adapter
                                    .as_ref()
                                    .unwrap()
                                    .cal_option_dict
                                    .clone(), // unwrap will never fail here
                            }));
                    }
                    Err(e) => {
                        edcs_status = EdcsStatus::EdssErr;
                        response_payload = Some(edcs_response::Payload::EdssErrData(e.0));
                    }
                };

                debug!("HANDLER Finished setting up stream.");
            }
            EdcsMessageType::SetupStream => {
                // TODO: DRY here
                if let Some(adapter) = &mut self.adapter {
                    adapter.cal_option_dict = match msg.payload {
                        Some(edcs_message::Payload::SetupStreamParams(d)) => d.cal_option_dict,
                        // TODO keep it dry
                        _ => {
                            return Ok(EdcsResponse {
                                status: EdcsStatus::InvalidRequest as i32,
                                payload: Some(edcs_response::Payload::InvalidRequestData(
                                    "The given payload is not of type SetupEdcsParams".to_string(),
                                )),
                            })
                        }
                    };
                    if let Err(e) = adapter.init_server() {
                        edcs_status = EdcsStatus::EdssErr;
                        response_payload = Some(edcs_response::Payload::EdssErrData(e.0));
                    }
                } else {
                    edcs_status = EdcsStatus::UninitialisedEdss;
                }
            }
            EdcsMessageType::StartStream
            | EdcsMessageType::CloseStream
            | EdcsMessageType::UpdateStream => {
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
