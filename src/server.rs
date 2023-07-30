use super::*;

use std::future::Future;
use std::pin::Pin;

type ServerRpcHandlerResponse<E> = Result<Vec<u8>, ServerRpcProtocolError<E>>;
type ServerRpcHandlerFuture<'a, E> =
    Pin<Box<dyn Send + Future<Output = ServerRpcHandlerResponse<E>> + 'a>>;
type ServerRpcHandlerType<S, E> =
    Box<dyn 'static + Send + Sync + for<'a> Fn(S, &'a [u8]) -> ServerRpcHandlerFuture<'a, E>>;

/// A rpc handler that runs on the server.
///
/// This is used internaly by the [`#[rpc]`][rpc] macro.
#[allow(dead_code)]
pub struct ServerRpcHandler<S: ServerRpcService> {
    uri: &'static str,
    handler: ServerRpcHandlerType<S::ServerState, <S::Format as RpcFormat>::Error>,
}

impl<S: ServerRpcService> ServerRpcHandler<S> {
    /// Create a new rpc handler.
    pub fn new<M, H, F>(uri: &'static str, handler: H) -> Self
    where
        M: Send + RpcMethod<S>,
        H: 'static + Send + Sync + Clone + Fn(S::ServerState, M) -> F,
        F: Send + Future<Output = Result<M::Response, M::Error>>,
        S::ServerState: 'static + Send,
    {
        Self {
            uri,
            handler: Box::new(move |state, buffer| {
                let handler = handler.clone();
                Box::pin(async move {
                    let inner = async move {
                        let req = <S::Format as RpcFormat>::deserialize_request(buffer)
                            .map_err(|e| RpcError::ServerDeserializeError(e))?;

                        let res = handler(state, req)
                            .await
                            .map_err(|e| RpcError::HandlerError(e))?;
                        <Result<M::Response, RpcError<M::Error, _>>>::Ok(res)
                    };

                    let res = <S::Format as RpcFormat>::serialize_response(inner.await)?;

                    Ok(res)
                })
            }),
        }
    }
}

/// Server definition of a rpc service.
///
/// This should not be implemented directly, but rather via the
/// [`register_service!`][register_service] macro.
pub trait ServerRpcService: RpcService + Sized {
    type ServerState;
    type RegistryItem: ServerRpcRegistryItem<Self>;
}

pub trait ServerRpcRegistryItem<S: ServerRpcService> {
    fn handler(&self) -> &ServerRpcHandler<S>;
}

/// Errors that rpc handlers can return on the server.
#[derive(thiserror::Error, Debug)]
pub enum ServerRpcProtocolError<E> {
    /// The server was unable to serialize the rpc response.
    #[error("rpc serialize error")]
    SerializeError(#[from] E),
}

/// Find a matching rpc handler given a rpc service and uri.
#[cfg(feature = "server")]
pub fn find_rpc_handler<S: ServerRpcService>(uri: &str) -> Option<&'static ServerRpcHandler<S>>
where
    &'static S::RegistryItem: inventory::Collect,
{
    inventory::iter::<&'static S::RegistryItem>
        .into_iter()
        .map(|h| h.handler())
        .filter(|h| h.uri == uri)
        .next()
}

/// Handle a rpc call on the server.
#[cfg(feature = "server")]
pub async fn handle_rpc<S: 'static + ServerRpcService>(
    uri: &str,
    state: S::ServerState,
    payload: &[u8],
) -> Result<Vec<u8>, ServerRpcProtocolError<<S::Format as RpcFormat>::Error>>
where
    &'static S::RegistryItem: inventory::Collect,
{
    if let Some(handler) = find_rpc_handler::<S>(uri) {
        (handler.handler)(state, payload).await
    } else {
        Ok(<S::Format as RpcFormat>::serialize_response::<(), ()>(
            Err(RpcError::NoEndpointFound),
        )?)
    }
}
