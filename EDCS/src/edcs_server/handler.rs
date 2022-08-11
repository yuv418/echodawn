use std::sync::Arc;

use super::config;
use super::edcs_proto::{edcs_response, EdcsMessage, EdcsMessageType, EdcsResponse, EdcsStatus};
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
            EdcsMessageType::SetupStream => {
                // Initialize EDSS, don't start stream.

                // TODO stop hardcoding random stuff.
                // TODO autogenerate a random key and return it through the response.
                match EdssAdapter::new(
                    cfg.edss_config.plugin_name.clone(),
                    cfg.ip,
                    cfg.port,
                    1000000,
                    60,
                    "".to_owned(),
                ) {
                    Ok(adapter) => self.adapter = Some(adapter),
                    Err(e) => {
                        edcs_status = EdcsStatus::EdssErr;
                        response_payload = Some(edcs_response::Payload::EdssErrData(e.0));
                    }
                };
            }
            EdcsMessageType::StartStream => {
                if let Some(adapter) = &self.adapter {
                    if let Err(e) = adapter.init_server() {
                        edcs_status = EdcsStatus::EdssErr;
                        response_payload = Some(edcs_response::Payload::EdssErrData(e.0));
                    }
                } else {
                    edcs_status = EdcsStatus::UninitialisedEdss;
                }
            }
            EdcsMessageType::CloseStream | EdcsMessageType::UpdateStream => {}
        }

        // Send out the response
        Ok(EdcsResponse {
            status: edcs_status as i32, // NOTE is there a better way to do this?
            payload: response_payload,
        })
    }
}
