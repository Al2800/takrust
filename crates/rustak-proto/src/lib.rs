use prost::Message;
use thiserror::Error;

#[derive(Clone, PartialEq, Message)]
struct TakV1Payload {
    #[prost(bytes = "vec", tag = "1")]
    cot_message: Vec<u8>,
}

pub fn decode_v1_payload(bytes: &[u8]) -> Result<Vec<u8>, ProtoError> {
    let payload = TakV1Payload::decode(bytes)?;
    if payload.cot_message.is_empty() {
        return Err(ProtoError::EmptyCotMessage);
    }

    Ok(payload.cot_message)
}

pub fn encode_v1_payload(message: &[u8]) -> Result<Vec<u8>, ProtoError> {
    if message.is_empty() {
        return Err(ProtoError::EmptyCotMessage);
    }

    let payload = TakV1Payload {
        cot_message: message.to_vec(),
    };
    let mut encoded = Vec::with_capacity(payload.encoded_len());
    payload.encode(&mut encoded)?;
    Ok(encoded)
}

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error(transparent)]
    Decode(#[from] prost::DecodeError),
    #[error(transparent)]
    Encode(#[from] prost::EncodeError),
    #[error("payload must contain a non-empty CoT message")]
    EmptyCotMessage,
}
