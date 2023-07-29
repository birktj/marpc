use super::*;

use std::pin::Pin;
use std::future::Future;

/// The set of rpc errors that the client can observe.
///
/// This is the error type that any rpc function defined by [`#[rpc]`][rpc] will end up having.
#[derive(thiserror::Error, Debug)]
pub enum ClientRpcError<SE, CE, FE> {
    /// The client could not serialize the rpc request.
    #[error("client rpc serialize error")]
    ClientSerializeError(#[source] FE),
    /// The client could not deserialize the rpc response.
    #[error("client rpc deserialize error")]
    ClientDeserializeError(#[source] FE),
    /// The server could not find any matching rpc endpoints.
    #[error("no rpc endpoint found")]
    NoEndpointFound,
    /// The server could not deserialize the rpc request.
    #[error("server rpc deserialize error")]
    ServerDeserializeError(#[source] FE),
    /// An error happened in the server rpc handler function.
    ///
    /// The rpc handler function is the one defined by [`#[rpc]`][rpc].
    #[error("server error")]
    ServerError(#[source] SE),
    /// An error happened in the client handler.
    ///
    /// THe client handler is [`ClientRpcService::handle`].
    #[error("client handler error")]
    HandlerError(#[source] CE),
}

impl<SE, CE, FE> From<RpcError<SE, FE>> for ClientRpcError<SE, CE, FE> {
    fn from(error: RpcError<SE, FE>) -> Self {
        match error {
            RpcError::NoEndpointFound => ClientRpcError::NoEndpointFound,
            RpcError::ServerDeserializeError(e) => ClientRpcError::ServerDeserializeError(e),
            RpcError::Other(e) => ClientRpcError::ServerError(e),
        }
    }
}

/// The client defintion of a rpc service.
pub trait ClientRpcService: RpcService {
    /// The error type of the client handler.
    type ClientError;

    /// The client handler function.
    fn handle<'a>(uri: &'static str, payload: &'a [u8]) 
        -> Pin<Box<dyn 'a + Future<Output = Result<Vec<u8>, Self::ClientError>>>>;
}

/// Perform a rpc call.
pub async fn rpc_call<S: ClientRpcService, M: RpcMethod<S>>(method: M) -> Result<M::Response, ClientRpcError<S::ServerError, S::ClientError, <S::Format as RpcFormat<S::ServerError>>::Error>> {
    let payload = <S::Format as RpcFormat<S::ServerError>>::serialize_request(method)
        .map_err(|e| ClientRpcError::ClientSerializeError(e))?;

    let res_buffer = S::handle(M::URI, &payload).await
        .map_err(|e| ClientRpcError::HandlerError(e))?;

    let res = <S::Format as RpcFormat<S::ServerError>>::deserialize_response(&res_buffer)
        .map_err(|e| ClientRpcError::ClientDeserializeError(e))??;

    Ok(res)
}
