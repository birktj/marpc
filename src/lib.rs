//! # Example
//!
//! ```
//! # pollster::block_on(async {
//! use std::future::Future;
//! use std::pin::Pin;
//! 
//! struct Service;
//! 
//! impl marpc::RpcService for Service {
//!     type Format = marpc::Json;
//!     type ServerError = ();
//! }
//! 
//! impl marpc::ClientRpcService for Service {
//!     type ClientError = Box<dyn std::error::Error>;
//! 
//!     fn handle<'a>(
//!         uri: &'static str,
//!         payload: &'a [u8],
//!     ) -> Pin<Box<dyn 'a + Future<Output = Result<Vec<u8>, Self::ClientError>>>> {
//!         Box::pin(async move {
//!             Ok(marpc::handle_rpc::<Service>(uri, (), payload).await?)
//!         })
//!     }
//! }
//! 
//! impl marpc::ServerRpcService for Service {
//!     type ServerState = ();
//! }
//! 
//! marpc::register_service!(Service);
//! 
//! #[marpc::rpc(AddRpc, uri = "/api/add", service = Service)]
//! async fn add(a: i32, b: i32) -> Result<i32, ()> {
//!     Ok(a + b)
//! }
//! 
//! let res = add(5, 6).await;
//! assert_eq!(res.unwrap(), 11);
//! # })
//! ```
//! # Rpc call description
//!
//! Steps:
//!
//! 1. Client calls rpc function
//! 2. Rpc function serializes payload (can fail => `ClientSerializeError`)
//! 3. Rpc function gets handler (fails with panic)
//! 4. Handler sends payload to server (can fail => `ClientHandlerError(e)`)
//! 5. Server recievers payload (not handled by this library)
//! 6. Server finds handler for this endpoint (can fail => `ServerNoEndpointFound`)
//! 7. Server calls handler with payload and server state
//! 8. Handler deserializes payload (can fail => `ServerDeserializeError`)
//! 9. Handler execututes code (can fail => `ServerHandlerError(e)`)
//! 10. Handler serializes response (can fail => `ServerSerializeError`)
//! 11. Server sends response to client (not handled by this library)
//! 12. Client recieves response payload (can fail => `ClientHandlerError(e)`)
//! 13. Client deserializes response payload (can fail => `ClientDeserializedError`)
//! 14. Client returns response
//!
//! Server errors:
//!
//! - ServerNoEdnpointFound
//! - ServerDeserializeError
//! - ServerHandlerError(e)
//! - ServerSerializeError
//!
//! Client errors:
//!
//! - ClientSerializeError
//! - ClientHandlerError(e)
//! - ClientDeserializeError
//!
//! Errors in response:
//!
//! - ServerNoEndpointFound
//! - ServerDeserializeError
//! - CustomError(e)

#[cfg(feature = "server")]
pub use inventory;

pub use serde;

#[derive(thiserror::Error, Debug, serde::Serialize, serde::Deserialize)]
pub enum RpcError<E, FE> {
    #[error("no rpc endpoint found")]
    NoEndpointFound,
    #[error("server rpc deserialize error")]
    ServerDeserializeError(#[source] FE),
    #[error(transparent)]
    Other(#[from] E),
}

pub type RpcResult<T, E, FE> = Result<T, RpcError<E, FE>>;

pub trait RpcMethod<S: RpcService>: serde::Serialize + serde::de::DeserializeOwned {
    type Response: serde::Serialize + serde::de::DeserializeOwned;
    const URI: &'static str;
}

pub trait RpcService {
    type Format: RpcFormat<Self::ServerError>;
    type ServerError: serde::Serialize + serde::de::DeserializeOwned;
}

pub trait RpcFormat<E> {
    type Error: 'static + serde::Serialize + serde::de::DeserializeOwned;

    fn serialize_request<M: serde::Serialize>(val: M) -> Result<Vec<u8>, Self::Error>;

    fn deserialize_request<M: serde::de::DeserializeOwned>(buffer: &[u8]) -> Result<M, Self::Error>;

    fn serialize_response<R: serde::Serialize>(val: RpcResult<R, E, Self::Error>) -> Result<Vec<u8>, Self::Error>
        where R: serde::Serialize;

    fn deserialize_response<R>(buffer: &[u8]) -> Result<RpcResult<R, E, Self::Error>, Self::Error>
        where R: serde::de::DeserializeOwned;
}

pub mod formats;
pub use formats::Json;

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "server")]
pub use server::{
    ServerRpcService,
    find_rpc_handler,
    handle_rpc,
};

#[cfg(feature = "client")]
mod client;

#[cfg(feature = "client")]
pub use client::{
    ClientRpcError,
    ClientRpcService,
};

pub use marpc_macros::rpc;

#[cfg(any(feature = "server", feature = "client"))]
pub mod internal {
    use super::*;

    pub use client::rpc_call;

    pub use server::ServerRpcHandler;
    pub use server::ServerRpcRegistryItem;
    pub use server::ServerRpcRegistry;
}

#[cfg(feature = "server")]
#[macro_export]
macro_rules! register_service {
    ($service:ident) => {
        const _: () = {
            struct ServiceRegistryItem {
                cell: std::sync::OnceLock<$crate::internal::ServerRpcHandler<$service>>,
                handler_fn: fn() -> $crate::internal::ServerRpcHandler<$service>
            }

            $crate::inventory::collect!(&'static ServiceRegistryItem);

            impl $service {
                #[doc(hidden)]
                pub const fn __rpc_call_internal_create_handler(handler_fn: fn() -> $crate::internal::ServerRpcHandler<$service>) -> ServiceRegistryItem {
                    ServiceRegistryItem {
                        cell: std::sync::OnceLock::new(),
                        handler_fn,
                    }
                }
            }

            impl $crate::internal::ServerRpcRegistryItem<$service> for ServiceRegistryItem {
                fn handler(&self) -> &$crate::internal::ServerRpcHandler<$service> {
                    self.cell.get_or_init(|| (self.handler_fn)())
                }
            }

            impl $crate::internal::ServerRpcRegistry for $service {
                type RegistryItem = ServiceRegistryItem;
            }
        };
    };
}
