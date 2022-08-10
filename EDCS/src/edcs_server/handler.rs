use std::sync::Arc;

use super::{config, edcs_proto_capnp};
use crate::edss_safe::edss;
use capnp::message;
use edcs_proto_capnp::edcs_protocol::{edcs_message, edcs_response, EdcsMessageType, EdcsStatus};

pub fn handle_message(
    cfg: Arc<config::EdcsConfig>,
    msg: edcs_message::Reader,
) -> capnp::Result<message::Builder<message::HeapAllocator>> {
    // Handle the input

    match msg.get_message_type() {
        Ok(EdcsMessageType::SetupStream) => {
            // Initialize EDSS, don't start stream.

            // TODO stop hardcoding random stuff.
            // TODO autogenerate a random key and return it through the response.
            edss::EdssAdapter::new(
                cfg.edss_config.plugin_name.clone(),
                cfg.ip,
                cfg.port,
                1000000,
                60,
                "".to_owned(),
            );
        }
        Ok(EdcsMessageType::StartStream) => {}
        Ok(EdcsMessageType::CloseStream) => {}
        Ok(EdcsMessageType::UpdateStream) => {}
        Err(capnp::NotInSchema(num)) => {
            println!("Invalid message type! {}", num)
        }
    }

    // Send out the response
    let mut response = message::Builder::new_default();
    let mut message_response = response.init_root::<edcs_response::Builder>();
    message_response.set_status(EdcsStatus::Ok);
    Ok(response)
}
