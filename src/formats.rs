use super::*;

/// A simple json-based rpc format.
///
/// This uses `serde_json` to (de)serialize rpc messages.
pub struct Json;

/// An error from json (de)serializing.
#[derive(Clone, Debug, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[error("json format error: {message}")]
pub struct JsonError {
    message: String,
}

impl From<serde_json::Error> for JsonError {
    fn from(err: serde_json::Error) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl RpcFormat for Json {
    type Error = JsonError;

    fn serialize_request<M: serde::Serialize>(val: M) -> Result<Vec<u8>, Self::Error> {
        Ok(serde_json::to_vec(&val)?)
    }

    fn deserialize_request<M: serde::de::DeserializeOwned>(
        buffer: &[u8],
    ) -> Result<M, Self::Error> {
        Ok(serde_json::from_slice(buffer)?)
    }

    fn serialize_response<R, E>(val: RpcResult<R, E, Self::Error>) -> Result<Vec<u8>, Self::Error>
    where
        R: serde::Serialize,
        E: serde::Serialize,
    {
        Ok(serde_json::to_vec(&val)?)
    }

    fn deserialize_response<R, E>(
        buffer: &[u8],
    ) -> Result<RpcResult<R, E, Self::Error>, Self::Error>
    where
        R: serde::de::DeserializeOwned,
        E: serde::de::DeserializeOwned,
    {
        Ok(serde_json::from_slice(buffer)?)
    }
}
