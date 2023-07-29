# MARPC - macro-based, boilerplate-free rpc library

This is a simple rpc library inspired by [`server_fn`][1] from the [`leptos`][2]
ecosystem. It allows you to define functions the will be run on a server, but
called from client. The primary usecase is webapps with a rust frontend, but
the library is designed to be easily adapatable and places no restrictions on
transport protocol or serialization format.

# Features

- Define functions one place, call them from the client and execute them on the
  server.
- No boilerplate. Rpc's are defined in one place and feature flags control if
  code is generated for the client, server, or both.
- "Bring your own transport". Use `ClientRpcService::handle` on the client and 
  `handle_rpc` on the server to control how rpc calls reaches the server and 
  responses back.
- "Bring your own (de)serializer". You can use any kind of (de)serializer for
  your rpc calls. `marpc` also defines a simple `json` format that you can use.

# Example

Start by defining a rpc service:

```rust
struct Service;

impl marpc::RpcService for Service {
    type Format = marpc::Json;
}

#[cfg(feature = "client")]
impl marpc::ClientRpcService for Service {
    type ClientError = Box<dyn std::error::Error>;

    fn handle<'a>(
        uri: &'static str,
        payload: &'a [u8],
    ) -> Pin<Box<dyn 'a + Future<Output = Result<Vec<u8>, Self::ClientError>>>> {
        // Send payload to the server
    }
}

#[cfg(feature = "server")]
marpc::register_service!(Service);
```

Define rpc functions with the following:

```rust
#[marpc::rpc(AddRpc, uri = "/api/add", service = Service)]
async fn add(a: i32, b: i32) -> Result<i32, ()> {
    Ok(a + b)
}
```

And call them on the client with:

```rust
add(5, 6).await;
```

On the server you can handle rpc calls with:

```rust
marpc::handle_rpc::<Service>(uri, (), payload).await
```

See [`examples/add.rs`](examples/add.rs) for a simple example of two threads
communicating over a global queue. Note that this must be compiled with 
`--all-features` or `--features client,server` as both the client and server
code needs to be generated.

# License

This library is dual-licensed under the MIT license and Apache License 2.0.
Chose the license to use at your own discretion. See
[LICENSE-MIT](./LICENSE-MIT) and [LICENSE-APACHE](./LICENSE-APACHE).

[1]: https://docs.rs/server_fn/latest/server_fn/
[2]: https://github.com/leptos-rs/leptos
