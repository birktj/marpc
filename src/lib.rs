#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

//! # MARPC - simple macro-based and boilerplate-free rpc library
//!
//! ## Usage
//!
//! - You start by creating a rpc service type that implements [`RpcService`]. This defines what
//!   format will be used for rpc transactions. This type needs to be accessible to both your
//!   client code and server code.
//! - The on the *client* you implement [`ClientRpcService`] for this type. This allows you to
//!   define a *client handler*, that is the function that sends the rpc request to the server
//!   and recieves a response.
//! - On the *server* you *register* your service type with
//!   [`register_service!`][register_service]. This allows [`find_rpc_handler`] [`handle_rpc`] to
//!   find your rpc handler. It also allows you to specify a server state type.
//! - Finally you can define rpc functions with the [`#[rpc]`][rpc] macro. Use the `client` and
//!   `server` feature flags to control whether to generate code for the client or server in the
//!   [`#[rpc]`][rpc] macro.
//! - On the *client* you can simply call yout rpc functions as they where normal functions.
//! - On the *server* use [`handle_rpc`] to handle any incomming rpc requests.
//!
//! ## Example
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
//! }
//!
//! #[cfg(feature = "client")]
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
//! #[cfg(feature = "server")]
//! marpc::register_service!(Service);
//!
//! #[marpc::rpc(AddRpc, uri = "/api/add", service = Service)]
//! async fn add(a: i32, b: i32) -> Result<i32, String> {
//!     Ok(a + b)
//! }
//!
//! let res = add(5, 6).await;
//! assert_eq!(res.unwrap(), 11);
//! # })
//! ```

#[cfg(feature = "server")]
pub use inventory;

pub use serde;

/// The various protocol errors that can happen.
#[derive(thiserror::Error, Debug, Clone)]
pub enum ProtocolError<FE> {
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
}

/// The rpc errors that can happen on the server and that are sent to the client.
#[derive(thiserror::Error, Debug, serde::Serialize, serde::Deserialize)]
pub enum RpcError<E, FE> {
    /// The server could not find a matching rpc endpoint.
    #[error("no rpc endpoint found")]
    NoEndpointFound,
    /// The server got an error when deserializing the rpc request.
    #[error("server rpc deserialize error")]
    ServerDeserializeError(#[source] FE),
    /// An error happened in the rpc handler function.
    ///
    /// The rpc handler function is the one defined by [`#[rpc]`][rpc].
    #[error(transparent)]
    HandlerError(#[from] E),
}

pub type RpcResult<T, E, FE> = Result<T, RpcError<E, FE>>;

/// An rpc method definition.
///
/// The [`#[rpc]`][rpc] macro defines this trait for the type named in its first argument. Eg.
/// given:
///
/// ```
/// # struct Service;
/// #
/// # impl marpc::RpcService for Service {
/// #     type Format = marpc::Json;
/// # }
/// #
/// # marpc::register_service!(Service);
/// #
/// # impl marpc::ClientRpcService for Service {
/// #     type ClientError = Box<dyn std::error::Error>;
/// #     fn handle<'a>(_uri: &'static str, _payload: &'a [u8])
/// #         -> std::pin::Pin<Box<dyn 'a + std::future::Future<Output = Result<Vec<u8>, Self::ClientError>>>>
/// #     {
/// #         Box::pin(async { Err("".into()) })
/// #     }
/// # }
/// #[marpc::rpc(Test, uri = "/test", service = Service)]
/// async fn test() -> Result<(), String> {
///     Ok(())
/// }
/// ```
///
/// In this example a struct named `Test` will be defined and it will implement
/// `RpcMethod<Service>`.
pub trait RpcMethod<S: RpcService>: serde::Serialize + serde::de::DeserializeOwned {
    type Response: serde::Serialize + serde::de::DeserializeOwned;
    type Error: serde::Serialize + serde::de::DeserializeOwned;
    const URI: &'static str;
}

/// A rpc service.
///
/// See also [`ClientRpcService`] and [`register_service!`][register_service].
pub trait RpcService {
    type Format: RpcFormat;
}

/// A format for (de)serializing rpc messages.
pub trait RpcFormat {
    type Error: 'static + serde::Serialize + serde::de::DeserializeOwned;

    /// Serialize the rpc request to send to the server from the client.
    fn serialize_request<M: serde::Serialize>(val: M) -> Result<Vec<u8>, Self::Error>;

    /// Deserialize the rpc response from the client on the server.
    fn deserialize_request<M: serde::de::DeserializeOwned>(buffer: &[u8])
        -> Result<M, Self::Error>;

    /// Serialize the rpc response to send to the client from the server.
    fn serialize_response<R, E>(val: RpcResult<R, E, Self::Error>) -> Result<Vec<u8>, Self::Error>
    where
        R: serde::Serialize,
        E: serde::Serialize;

    /// Deserialize the rpc response from the server on the client.
    fn deserialize_response<R, E>(
        buffer: &[u8],
    ) -> Result<RpcResult<R, E, Self::Error>, Self::Error>
    where
        R: serde::de::DeserializeOwned,
        E: serde::de::DeserializeOwned;
}

pub mod formats;
pub use formats::Json;

mod server;

#[cfg(feature = "server")]
pub use server::{find_rpc_handler, handle_rpc};

mod client;

pub use client::ClientRpcService;

/// Define a rpc function.
///
/// This function must be `async` and must return a [`Result`].
///
/// Use the `client` and `server` feature flags to control whether to generate code for the
/// client, server, or both.
///
/// # Arguments
///
/// - The first argument to this attribute must be the name of the *rpc method struct*.
/// - There must be exactly one `uri = "<uri>"` argument describing the rpc uri.
/// - There must be exactly one `service = SomeService` argument giving the coresponding rpc
///   service.
///
/// # `#[server]` attribute
///
/// Functions arguments that are decorated with the `#[server]` attribute are *server arguments*.
/// They correspond to [`internal::ServerRpcService::ServerState`] which is given by the
/// `SomeState` argument in [`register_service!(MyService with SomeState)`][register_service].
///
/// - If no `#[server]` arguments are given then `ServerState` must be `()`.
/// - If one `#[server]` arguments are given then `ServerState` must of the same type as this
///   argument.
/// - If multiple `#[server]` arguments are given the `ServerState` must be a tuple of all the
///   types of these arguments.
///
/// These arguments can not be used by the client function.
///
/// # Workings
///
/// `#[rpc]` defines two items:
/// - A struct with fields correspoing to all non-`#[server]` arguments. This struct implements
///   [`Serialize`][serde::Serialize] and [`Deserialize`][serde::Deserialize] as well as
///   [`RpcMethod`].
/// - On the *client* a function with the same name and arguments (except for `#[server]`
///   arguments) as the given function. Its return type will be [`Result`] with the same `Ok`
///   type as the given function but [`ClientRpcService::ClientError`] as the `Err` type. This 
///   function calls [`ClientRpcService::handle`] to perform the rpc call.
///
/// One the *server* it also creates an handler that discoverable by [`find_rpc_handler`] and
/// [`handle_rpc`]. This contains the code from the function body.
///
/// # Examples
///
/// ```
/// # struct Service;
/// #
/// # impl marpc::RpcService for Service {
/// #     type Format = marpc::Json;
/// # }
/// #
/// # marpc::register_service!(Service);
/// #
/// # impl marpc::ClientRpcService for Service {
/// #     type ClientError = Box<dyn std::error::Error>;
/// #     fn handle<'a>(_uri: &'static str, _payload: &'a [u8])
/// #         -> std::pin::Pin<Box<dyn 'a + std::future::Future<Output = Result<Vec<u8>, Self::ClientError>>>>
/// #     {
/// #         Box::pin(async { Err("".into()) })
/// #     }
/// # }
/// #[marpc::rpc(Test, uri = "/test", service = Service)]
/// async fn test() -> Result<(), String> {
///     Ok(())
/// }
/// ```
///
/// Server state with `#[server]`:
///
/// ```
/// # struct Service;
/// #
/// # impl marpc::RpcService for Service {
/// #     type Format = marpc::Json;
/// # }
/// #
/// #
/// #
/// # impl marpc::ClientRpcService for Service {
/// #     type ClientError = Box<dyn std::error::Error>;
/// #     fn handle<'a>(_uri: &'static str, _payload: &'a [u8])
/// #         -> std::pin::Pin<Box<dyn 'a + std::future::Future<Output = Result<Vec<u8>, Self::ClientError>>>>
/// #     {
/// #         Box::pin(async { Err("".into()) })
/// #     }
/// # }
/// marpc::register_service!(Service with i32);
///
/// #[marpc::rpc(Test, uri = "/test", service = Service)]
/// async fn test(#[server] number: i32) -> Result<i32, String> {
///     Ok(number)
/// }
/// ```
///
pub use marpc_macros::rpc;

pub mod internal {
    use super::*;

    pub use client::rpc_call;

    pub use server::ServerRpcHandler;
    pub use server::ServerRpcRegistryItem;
    pub use server::ServerRpcService;

    pub trait ResultTypes {
        type Ok;
        type Err;
    }

    impl<T, E> ResultTypes for Result<T, E> {
        type Ok = T;
        type Err = E;
    }
}

/// Register a rpc service.
///
/// This macro defines the correct types and methods such that [`handle_rpc`] is able to discover
/// any rpc handlers. This is based on the [`inventory`] crate. It also allows you to specify a
/// server state type.
///
/// In order to specify a server state type use `register_service!(MyService with MyState)`.
///
/// This macro must be placed outside of any function body.
///
/// # Example
///
/// ```
/// struct Service;
///
/// impl marpc::RpcService for Service {
///     type Format = marpc::Json;
/// }
///
/// # impl marpc::ClientRpcService for Service {
/// #     type ClientError = Box<dyn std::error::Error>;
/// #     fn handle<'a>(_uri: &'static str, _payload: &'a [u8])
/// #         -> std::pin::Pin<Box<dyn 'a + std::future::Future<Output = Result<Vec<u8>, Self::ClientError>>>>
/// #     {
/// #         Box::pin(async { Err("".into()) })
/// #     }
/// # }
/// marpc::register_service!(Service with i32);
///
/// #[marpc::rpc(Test, uri = "/test", service = Service)]
/// async fn test(#[server] number: i32) -> Result<i32, String> {
///     Ok(number)
/// }
///
/// assert!(marpc::find_rpc_handler::<Service>("/test").is_some());
/// assert!(marpc::find_rpc_handler::<Service>("/test2").is_none());
/// ```
///
#[cfg(feature = "server")]
#[macro_export]
macro_rules! register_service {
    ($service:ident) => {
        $crate::register_service!($service with ());
    };
    ($service:ident with $state:ty) => {
        const _: () = {
            pub struct ServiceRegistryItem {
                cell: std::sync::OnceLock<$crate::internal::ServerRpcHandler<$service>>,
                handler_fn: fn() -> $crate::internal::ServerRpcHandler<$service>,
            }

            $crate::inventory::collect!(&'static ServiceRegistryItem);

            impl $service {
                #[doc(hidden)]
                pub const fn __rpc_call_internal_create_handler(
                    handler_fn: fn() -> $crate::internal::ServerRpcHandler<$service>,
                ) -> ServiceRegistryItem {
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

            impl $crate::internal::ServerRpcService for $service {
                type ServerState = $state;
                type RegistryItem = ServiceRegistryItem;
            }
        };
    };
}
