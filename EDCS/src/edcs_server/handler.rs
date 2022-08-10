use std::sync::Arc;

use super::{config, edcs_proto_capnp};
use crate::edss_safe::edss::EdssAdapter;
use capnp::message;
use edcs_proto_capnp::edcs_protocol::{edcs_message, edcs_response, EdcsMessageType, EdcsStatus};

#[derive(Debug, Default)]
pub struct EdcsHandler {
    adapter: Option<EdssAdapter>,
}

impl EdcsHandler {
    pub fn handle_message(
        &mut self,
        cfg: Arc<config::EdcsConfig>,
        msg: edcs_message::Reader,
    ) -> capnp::Result<message::Builder<message::HeapAllocator>> {
        // Handle the input

        let mut response = message::Builder::new_default();
        let mut message_response = response.init_root::<edcs_response::Builder>();
        let mut edcs_status = EdcsStatus::Ok;

        match msg.get_message_type() {
            Ok(EdcsMessageType::SetupStream) => {
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
                        message_response
                            .reborrow()
                            .get_payload()
                            .set_edss_err_params(e.0);
                    }
                };
            }
            Ok(EdcsMessageType::StartStream) => {
                if let Some(adapter) = &self.adapter {
                    if let Err(e) = adapter.init_server() {
                        edcs_status = EdcsStatus::EdssErr;
                        message_response
                            .reborrow()
                            .get_payload()
                            .set_edss_err_params(e.0);
                    }
                } else {
                    edcs_status = EdcsStatus::UninitialisedEdss;
                }
            }
            Ok(EdcsMessageType::CloseStream) => {}
            Ok(EdcsMessageType::UpdateStream) => {}
            Err(capnp::NotInSchema(num)) => {
                println!("Invalid message type! {}", num)
            }
        }

        // Send out the response
        message_response.set_status(edcs_status);
        Ok(response)
    }
}
