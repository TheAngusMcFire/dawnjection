use std::time::Duration;

use async_nats::jetstream;

#[tokio::main]
async fn main() -> Result<(), color_eyre::Report> {
    env_logger::init();
    let connection_string = std::env::var("NATS_CONNECTION_STRING")?;
    let client = async_nats::connect(connection_string).await?;
    let js = jetstream::new(client);
    for i in 0..1000000 {
        let ret = js
            .publish(format!("test"), "this is the message".into())
            .await?;
        ret.await?;
        // tokio::time::sleep(Duration::from_millis(1000)).await;
    }
    Ok(())
}
