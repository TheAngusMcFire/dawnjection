use async_nats::jetstream;

#[tokio::main]
async fn main() -> Result<(), color_eyre::Report> {
    env_logger::init();
    let connection_string = std::env::var("NATS_CONNECTION_STRING")?;
    let client = async_nats::connect(connection_string).await?;
    let js = jetstream::new(client);
    for i in 0..10000000 {
        let ret = js
            .publish(
                "test".to_string(),
                bytes::Bytes::from(format!("this is the message: {}", i)),
            )
            .await?;
        let res = ret.await?;
        // tokio::time::sleep(Duration::from_millis(1000)).await;
    }
    Ok(())
}
