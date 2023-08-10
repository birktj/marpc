use super::*;

use std::future::Future;
use std::pin::Pin;

/// The client defintion of a rpc service.
pub trait ClientRpcService: RpcService {
    /// The error type of the client handler.
    type ClientError: From<ProtocolError<<Self::Format as RpcFormat>::Error>>;

    /// The client handler function.
    fn handle<'a>(
        uri: &'static str,
        payload: &'a [u8],
    ) -> Pin<Box<dyn 'a + Future<Output = Result<Vec<u8>, Self::ClientError>>>>;
}

/// Perform a rpc call.
pub async fn rpc_call<S: ClientRpcService, M: RpcMethod<S>>(
    method: M,
) -> Result<M::Response, S::ClientError>
    where S::ClientError: From<M::Error>
{
    let payload = <S::Format as RpcFormat>::serialize_request(method)
        .map_err(|e| ProtocolError::ClientSerializeError(e))?;

    let res_buffer = S::handle(M::URI, &payload).await?;

    let res = <S::Format as RpcFormat>::deserialize_response(&res_buffer)
        .map_err(|e| ProtocolError::ClientDeserializeError(e))?
        .map_err::<S::ClientError, _>(|e: RpcError<<M as RpcMethod<S>>::Error, _>| match e {
            RpcError::NoEndpointFound => ProtocolError::NoEndpointFound.into(),
            RpcError::ServerDeserializeError(e) => ProtocolError::ServerDeserializeError(e).into(),
            RpcError::HandlerError(e) => e.into(),
        })?;

    Ok(res)
}
