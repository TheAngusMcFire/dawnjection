use std::{
    str::from_utf8,
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};

use async_nats::jetstream::{self, consumer::PullConsumer};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), color_eyre::Report> {
    env_logger::init();
    let arg = std::env::args().nth(1).unwrap();

    match arg.as_str() {
        "p" => publish().await,
        "pj" => publish_js().await,
        "r" => request().await,
        "s" => subscribe().await,
        "ss" => example().await,
        _ => {
            todo!()
        }
    }?;

    Ok(())
}

async fn publish() -> Result<(), color_eyre::Report> {
    {
        let cnt = Arc::new(AtomicUsize::new(0));
        for _ in 0..1 {
            let acnt = cnt.clone();
            tokio::spawn(async move {
                let connection_string = std::env::var("NATS_CONNECTION_STRING").unwrap();
                let client = async_nats::connect(connection_string).await.unwrap();
                for _ in 0..10000000 {
                    let request = async_nats::Request::new()
                        .payload(
                            format!(
                                "this is the message: {}",
                                acnt.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                            )
                            .into(),
                        )
                        .timeout(Some(Duration::from_secs(600)));
                    let response = client
                        .send_request("simple_consumer", request)
                        .await
                        .unwrap();
                    dbg!(&response);
                    // let _res = ret.await.unwrap();
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                }
            });
        }
        tokio::time::sleep(Duration::from_secs(600)).await;
        Ok(())
    }
}

async fn publish_js() -> Result<(), color_eyre::Report> {
    {
        let cnt = Arc::new(AtomicUsize::new(0));
        for _ in 0..1 {
            let acnt = cnt.clone();
            tokio::spawn(async move {
                let connection_string = std::env::var("NATS_CONNECTION_STRING").unwrap();
                let client = async_nats::connect(connection_string).await.unwrap();
                let js = jetstream::new(client);
                for _ in 0..10000000 {
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
                    let _res = ret.await.unwrap();
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                }
            });
        }
        tokio::time::sleep(Duration::from_secs(600)).await;
        Ok(())
    }
}

async fn request() -> Result<(), color_eyre::Report> {
    let connection_string = std::env::var("NATS_CONNECTION_STRING").unwrap();
    let client = async_nats::connect(connection_string).await.unwrap();
    let res = client
        .request("template_and_print_label_consumer", "{}".into())
        .await?;
    dbg!(res.payload);

    Ok(())
}

async fn subscribe() -> Result<(), color_eyre::Report> {
    let connection_string = std::env::var("NATS_CONNECTION_STRING").unwrap();
    let client = async_nats::connect(connection_string).await.unwrap();
    let jetstream = jetstream::new(client);
    let stream = jetstream
        .get_or_create_stream(jetstream::stream::Config {
            name: "stream1".into(),
            retention: jetstream::stream::RetentionPolicy::Limits,
            subjects: vec!["test".into()],
            ..Default::default()
        })
        .await?;

    log::info!("created stream");

    let consumer: PullConsumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            durable_name: Some("consumer".to_string()),
            ..Default::default()
        })
        .await?;

    let mut msg = consumer.messages().await?;

    while let Some(t) = msg.next().await {
        let t = t?;
        dbg!(&t.payload);
        dbg!(&t.subject);
        dbg!(&t.reply);
        let r = t
            .context
            .publish(t.reply.clone().unwrap(), "this is some test".into())
            .await?;
        let rr = r.await;
        dbg!(&rr);
    }

    Ok(())
}

async fn example() -> Result<(), color_eyre::Report> {
    let nats_url =
        std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());

    let client = async_nats::connect(nats_url).await?;

    let requests = client.subscribe("greet.*").await.unwrap();

    // tokio::spawn({
    //     let client = client.clone();
    //     async move {
    //         while let Some(request) = requests.next().await {
    //             if let Some(reply) = request.reply {
    //                 let name = &request.subject[6..];
    //                 client
    //                     .publish(reply, format!("hello, {}", name).into())
    //                     .await?;
    //             }
    //         }
    //         Ok::<(), async_nats::Error>(())
    //     }
    // });

    let response = client.request("greet.sue", "".into()).await?;
    println!("got a response: {:?}", from_utf8(&response.payload)?);

    let response = client.request("greet.john", "".into()).await?;
    println!("got a response: {:?}", from_utf8(&response.payload)?);

    let response = tokio::time::timeout(
        Duration::from_millis(500),
        client.request("greet.bob", "".into()),
    )
    .await??;
    println!("got a response: {:?}", from_utf8(&response.payload)?);

    Ok(())
}
