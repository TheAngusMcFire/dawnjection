use std::time::Duration;

use dawnjection::{
    handler::{FromRequestBody, HandlerRegistry, HandlerRequest},
    nats::{NatsDispatcher, NatsMetadata, NatsPayload},
    ServiceCollection, ServiceProviderContainer, I,
};

#[derive(Default)]
pub struct State {}

#[derive(Clone)]
pub struct Config {
    msg: &'static str,
}

pub struct Raw(String);
#[async_trait::async_trait]
impl FromRequestBody<ServiceProviderContainer, NatsPayload, NatsMetadata, ()> for Raw {
    type Rejection = Result<(), eyre::Report>;
    async fn from_request(
        req: HandlerRequest<NatsPayload, NatsMetadata>,
        _state: &ServiceProviderContainer,
    ) -> Result<Self, Self::Rejection> {
        let msg = String::from_utf8_lossy(req.payload.data.to_vec().as_slice()).to_string();
        Ok(Raw(msg))
    }
}

async fn simple_consumer(I(config): I<Config>, raw: Raw) {
    println!(stringify!(simple_consumer));
    println!("config value: {} message payload\n{}", config.msg, raw.0);
}

async fn simple_subscriber_one(I(config): I<Config>, raw: Raw) {
    println!(stringify!(simple_subscriber_one));
    println!("config value: {} message payload\n{}", config.msg, raw.0);
    // tokio::time::sleep(Duration::from_secs(2)).await;
}

async fn simple_subscriber_two(I(config): I<Config>, raw: Raw) {
    println!(stringify!(simple_subscriber_two));
    println!("config value: {} message payload\n{}", config.msg, raw.0);
    // tokio::time::sleep(Duration::from_secs(2)).await;
}

async fn simple_subscriber_three(I(config): I<Config>, raw: Raw) {
    println!(stringify!(simple_subscriber_three));
    println!("config value: {} message payload\n{}", config.msg, raw.0);
}

async fn simple_subscriber_four(I(config): I<Config>, raw: Raw) {
    println!(stringify!(simple_subscriber_four));
    println!("config value: {} message payload\n{}", config.msg, raw.0);
}

async fn simple_subscriber_five(I(config): I<Config>, raw: Raw) {
    println!(stringify!(simple_subscriber_five));
    println!("config value: {} message payload\n{}", config.msg, raw.0);
}

async fn simple_subscriber_other_topic(I(config): I<Config>, raw: Raw) {
    println!(stringify!(simple_subscriber_other_topic));
    println!("config value: {} message payload\n{}", config.msg, raw.0);
    tokio::time::sleep(Duration::from_secs(2)).await;
}

#[tokio::main]
async fn main() -> Result<(), color_eyre::Report> {
    env_logger::init();
    color_eyre::install()?;

    let connection_string = std::env::var("NATS_CONNECTION_STRING")?;
    let subscriber_name = std::env::var("NATS_SUBSCRIBER_NAME")?;

    let consumer_registry =
        HandlerRegistry::<NatsPayload, NatsMetadata, ServiceProviderContainer, ()>::default()
            .register(simple_consumer);

    let subscriber_registry =
        HandlerRegistry::<NatsPayload, NatsMetadata, ServiceProviderContainer, ()>::default()
            .register_with_name("topic1", simple_subscriber_one)
            .register_with_name("topic1", simple_subscriber_two)
            .register_with_name("topic3", simple_subscriber_three)
            .register_with_name("topic4", simple_subscriber_four)
            .register_with_name("topic5", simple_subscriber_five)
            .register_with_name("topic2", simple_subscriber_other_topic);

    log::info!("Start the nats dispatcher");

    NatsDispatcher::new(
        consumer_registry,
        subscriber_registry,
        &connection_string,
        16,
        ServiceProviderContainer(
            ServiceCollection::default()
                .reg_cloneable(Config {
                    msg: "This is the first message from the dispatcher",
                })
                .build_service_provider_arc(),
        ),
        "EVENTS1".into(),
        subscriber_name,
    )
    .start()
    .await?;

    Ok(())
}
