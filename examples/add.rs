use std::future::Future;
use std::pin::Pin;
use std::sync::{mpsc, OnceLock};

struct Message {
    uri: &'static str,
    payload: Vec<u8>,
    response_channel: mpsc::Sender<Vec<u8>>,
}

static QUEUE: OnceLock<mpsc::SyncSender<Message>> = OnceLock::new();

struct Service;

impl marpc::RpcService for Service {
    type Format = marpc::Json;
}

impl marpc::ClientRpcService for Service {
    type ClientError = Box<dyn std::error::Error>;

    fn handle<'a>(
        uri: &'static str,
        payload: &'a [u8],
    ) -> Pin<Box<dyn 'a + Future<Output = Result<Vec<u8>, Self::ClientError>>>> {
        Box::pin(async move {
            let (send, recv) = mpsc::channel();

            let message = Message {
                uri,
                payload: payload.to_owned(),
                response_channel: send,
            };
            // Send the rpc call to the "server"
            QUEUE.get().unwrap().send(message).unwrap();

            // Wait for a response
            Ok(recv.recv().unwrap())
        })
    }
}

marpc::register_service!(Service);

#[marpc::rpc(MyTest, uri = "/api/add", service = Service)]
async fn test(a: i32, b: i32) -> Result<i32, ()> {
    Ok(a + b)
}

fn main() {
    let (send, recv) = mpsc::sync_channel(1);
    QUEUE.set(send).unwrap();

    std::thread::spawn(move || {
        pollster::block_on(async move {
            while let Ok(message) = recv.recv() {
                let res = marpc::handle_rpc::<Service>(message.uri, (), &message.payload)
                    .await
                    .unwrap();
                message.response_channel.send(res).unwrap();
            }
        });
    });

    pollster::block_on(async {
        let res = test(5, 6).await;
        println!("Res: {:?}", res);
    });
}
