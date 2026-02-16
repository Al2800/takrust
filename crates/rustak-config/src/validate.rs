use crate::{ConfigError, RustakConfig};

pub(crate) fn validate_startup(config: &RustakConfig) -> Result<(), ConfigError> {
    config.validate()?;

    let Some(bridge) = &config.bridge else {
        return Ok(());
    };

    if !bridge.validation.strict_startup {
        return Ok(());
    }

    if bridge.limits.max_frame_bytes > config.transport.limits.max_frame_bytes {
        return Err(ConfigError::StrictStartupBridgeFrameLimitExceedsTransport {
            bridge_max_frame_bytes: bridge.limits.max_frame_bytes,
            transport_max_frame_bytes: config.transport.limits.max_frame_bytes,
        });
    }

    if bridge.emitter.max_pending_events > config.transport.limits.max_queue_messages {
        return Err(
            ConfigError::StrictStartupBridgePendingEventsExceedTransport {
                bridge_pending_events: bridge.emitter.max_pending_events,
                transport_max_queue_messages: config.transport.limits.max_queue_messages,
            },
        );
    }

    Ok(())
}
