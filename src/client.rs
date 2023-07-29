use super::*;

use std::pin::Pin;
use std::future::Future;

#[derive(thiserror::Error, Debug)]
pub enum ClientRpcError<SE, CE, FE> {
    #[error("client rpc serialize error")]
    ClientSerializeError(#[source] FE),
    #[error("client rpc deserialize error")]
    ClientDeserializeError(#[source] FE),
    #[error("no rpc endpoint found")]
    NoEndpointFound,
    #[error("server rpc deserialize error")]
    ServerDeserializeError(#[source] FE),
    #[error("server error")]
    ServerError(#[source] SE),
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

pub trait ClientRpcService: RpcService {
    type ClientError;

    fn handle<'a>(uri: &'static str, payload: &'a [u8]) 
        -> Pin<Box<dyn 'a + Future<Output = Result<Vec<u8>, Self::ClientError>>>>;
}

pub async fn rpc_call<S: ClientRpcService, M: RpcMethod<S>>(method: M) -> Result<M::Response, ClientRpcError<S::ServerError, S::ClientError, <S::Format as RpcFormat<S::ServerError>>::Error>> {
    let payload = <S::Format as RpcFormat<S::ServerError>>::serialize_request(method)
        .map_err(|e| ClientRpcError::ClientSerializeError(e))?;

    let res_buffer = S::handle(M::URI, &payload).await
        .map_err(|e| ClientRpcError::HandlerError(e))?;

    let res = <S::Format as RpcFormat<S::ServerError>>::deserialize_response(&res_buffer)
        .map_err(|e| ClientRpcError::ClientDeserializeError(e))??;

    Ok(res)
}
