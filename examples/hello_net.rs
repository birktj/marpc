use std::future::Future;
use std::pin::Pin;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

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
        Box::pin(async move {
            let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
            let mut uri_buf = [0; 256];
            (&mut uri_buf[0..uri.as_bytes().len()]).copy_from_slice(uri.as_bytes());
            stream.write_all(&uri_buf).await?;
            stream.write_all(payload).await?;
            stream.shutdown().await?;
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).await?;
            Ok(buf)
        })
    }
}

#[cfg(feature = "server")]
marpc::register_service!(Service with String);

#[cfg(feature = "server")]
async fn server(greeting: String) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (mut socket, _) = listener.accept().await?;

        let mut uri_buf = [0; 256];
        socket.read_exact(&mut uri_buf).await?;
        let uri = std::str::from_utf8(&uri_buf[0..uri_buf.iter().position(|b| *b == 0).unwrap()])?;

        let mut buf = Vec::new();
        socket.read_to_end(&mut buf).await?;

        let res = marpc::handle_rpc::<Service>(uri, greeting.clone(), &buf).await?;
        socket.write_all(&res).await?;
        socket.shutdown().await?;
    }
}

#[cfg(feature = "client")]
async fn client(name: String) -> Result<(), Box<dyn std::error::Error>> {
    let full_greeting = say_hello(name).await?;
    println!("{full_greeting}");
    Ok(())
}

#[marpc::rpc(SayHello, uri = "/hello", service = Service)]
async fn say_hello(#[server] greeting: String, name: String) -> Result<String, String> {
    Ok(format!("{greeting} {name}!"))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();

    match &*args[1] {
        #[cfg(feature = "client")]
        "client" => {
            client(args[2].clone()).await?;
        }
        #[cfg(feature = "server")]
        "server" => {
            server(args[2].clone()).await?;
        }
        _ => Err("Must be called as `client` or `server`")?,
    }

    Ok(())
}
