use super::*;

use std::pin::Pin;
use std::future::Future;

type ServerRpcHandlerResponse<E> = Result<Vec<u8>, ServerRpcProtocolError<E>>;
type ServerRpcHandlerFuture<'a, E> = Pin<Box<dyn Future<Output = ServerRpcHandlerResponse<E>> + 'a>>;
type ServerRpcHandlerType<S, E> = Box<dyn 'static + Send + Sync + for<'a> Fn(S, &'a [u8]) -> ServerRpcHandlerFuture<'a, E>>;

/// A rpc handler that runs on the server.
///
/// This is used internaly by the [`#[rpc]`][rpc] macro.
pub struct ServerRpcHandler<S: ServerRpcService> {
    uri: &'static str,
    handler: ServerRpcHandlerType<S::ServerState, <S::Format as RpcFormat<S::ServerError>>::Error>,
}

impl<S: ServerRpcService> ServerRpcHandler<S> {
    /// Create a new rpc handler.
    pub fn new<M, H, F>(uri: &'static str, handler: H) -> Self
        where M: RpcMethod<S>,
              H: 'static + Send + Sync + Clone + Fn(S::ServerState, M) -> F,
              F: Future<Output = Result<M::Response, S::ServerError>>,
              S::ServerState: 'static
    {
        Self {
            uri,
            handler: Box::new(move |state, buffer| {
                let handler = handler.clone();
                Box::pin(async move {
                    let inner = async move {
                        let req = <S::Format as RpcFormat<S::ServerError>>::deserialize_request(buffer)
                            .map_err(|e| RpcError::ServerDeserializeError(e))?;

                        let res = handler(state, req).await
                            .map_err(|e| RpcError::Other(e))?;
                        Ok(res)
                    };

                    let res = <S::Format as RpcFormat<S::ServerError>>::serialize_response(inner.await)?;

                    Ok(res)
                })
            })
        }
    }
}

/// Server definition of a rpc service.
pub trait ServerRpcService: RpcService {
    type ServerState;
}

pub trait ServerRpcRegistryItem<S: ServerRpcService> {
    fn handler(&self) -> &ServerRpcHandler<S>;
}

pub trait ServerRpcRegistry: ServerRpcService + Sized {
    type RegistryItem: ServerRpcRegistryItem<Self>;
}

/// Errors that rpc handlers can return on the server.
#[derive(thiserror::Error, Debug)]
pub enum ServerRpcProtocolError<E> {
    /// The server was unable to serialize the rpc response.
    #[error("rpc serialize error")]
    SerializeError(#[from] E),
}

/// Find a matching rpc handler given a rpc service and uri.
pub fn find_rpc_handler<S: ServerRpcRegistry>(uri: &str) -> Option<&'static ServerRpcHandler<S>> 
    where &'static S::RegistryItem: inventory::Collect
{
    inventory::iter::<&'static S::RegistryItem>.into_iter()
        .map(|h| h.handler()).filter(|h| h.uri == uri).next()
}

/// Handle a rpc call on the server.
pub async fn handle_rpc<S: 'static + ServerRpcRegistry>(uri: &str, state: S::ServerState, payload: &[u8]) ->
    Result<Vec<u8>, ServerRpcProtocolError<<S::Format as RpcFormat<S::ServerError>>::Error>>
    where &'static S::RegistryItem: inventory::Collect
{
    if let Some(handler) = find_rpc_handler::<S>(uri) {
        (handler.handler)(state, payload).await
    } else {
        Ok(<S::Format as RpcFormat<S::ServerError>>::serialize_response::<()>(Err(RpcError::NoEndpointFound))?)
    }
}
