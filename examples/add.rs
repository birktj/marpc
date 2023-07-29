use std::future::Future;
use std::pin::Pin;

struct Service;

impl marpc::RpcService for Service {
    type Format = marpc::Json;
    type ServerError = ();
}

impl marpc::ClientRpcService for Service {
    type ClientError = Box<dyn std::error::Error>;

    fn handle<'a>(
        uri: &'static str,
        payload: &'a [u8],
    ) -> Pin<Box<dyn 'a + Future<Output = Result<Vec<u8>, Self::ClientError>>>> {
        Box::pin(async move {
            Ok(marpc::handle_rpc::<Service>(uri, (), payload).await?)
        })
    }
}

impl marpc::ServerRpcService for Service {
    type ServerState = ();
}

marpc::register_service!(Service);

#[marpc::rpc(MyTest, uri = "/api/add", service = Service)]
async fn test(a: i32, b: i32) -> Result<i32, ()> {
    Ok(a + b)
}

fn main() {
    pollster::block_on(async {
        let res = test(5, 6).await;
        eprintln!("Res: {:?}", res);
    })
}
