use std::{
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};

use async_nats::jetstream;

#[tokio::main]
async fn main() -> Result<(), color_eyre::Report> {
    env_logger::init();
    let cnt = Arc::new(AtomicUsize::new(0));
    for n in 0..1 {
        let acnt = cnt.clone();
        tokio::spawn(async move {
            let connection_string = std::env::var("NATS_CONNECTION_STRING").unwrap();
            let client = async_nats::connect(connection_string).await.unwrap();
            let js = jetstream::new(client);
            for i in 0..10000000 {
                let ret = js
                    .publish(
                        "topic1".to_string(),
                        bytes::Bytes::from(format!(
                            "this is the message: {}",
                            acnt.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        )),
                    )
                    .await
                    .unwrap();
                let res = ret.await.unwrap();
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
        });
    }
    tokio::time::sleep(Duration::from_secs(600)).await;
    Ok(())
}
