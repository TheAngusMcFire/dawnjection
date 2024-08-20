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

// todo remove at some point
// trait implementations:
// trait from other crates can only be implemented for types of this crate
// trait implementations for other crates can only be done within the crate its defined

// retrieves data from the Message metadata and the state

// This example can be used to deserialize some object into some message
// pub struct Mp<T>(String, PhantomData<T>);
// #[async_trait::async_trait]
// impl<T> FromRequestBody<ServiceProviderContainer, Body, Meta, String> for Mp<T> {
//     type Rejection = Result<String, eyre::Report>;
//     async fn from_request(
//         req: HandlerRequest<Body, Meta>,
//         _state: &ServiceProviderContainer,
//     ) -> Result<Self, Self::Rejection> {
//         Ok(Mp(req.payload.incomming_message, Default::default()))
//     }
// }
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
pub struct Raw(String);
async fn test(I(config): I<Config>, raw: Raw) {
    // println!(stringify!(first_handler_which_does_nothing));
    // println!("config value: {} message payload\n{}", config.msg, raw.0);
}

#[tokio::main]
async fn main() -> Result<(), color_eyre::Report> {
    let connection_string = std::env::var("NATS_CONNECTION_STRING")?;
    env_logger::init();
    let reg = HandlerRegistry::<NatsPayload, NatsMetadata, ServiceProviderContainer, ()>::default()
        .register(test);

    log::info!("Start the nats dispatcher");
    let dispatcher = NatsDispatcher::new(
        reg,
        Default::default(),
        &connection_string,
        1000,
        ServiceProviderContainer(
            ServiceCollection::default()
                .reg_cloneable(Config {
                    msg: "This is the first message from the dispatcher",
                })
                .build_service_provider_arc(),
        ),
        "EVENTS".into(),
    );
    dispatcher.start().await?;
    Ok(())
}
