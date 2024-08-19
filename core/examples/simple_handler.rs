use std::marker::PhantomData;

use dawnjection::{
    handler::{FromRequestBody, HandlerRegistry, HandlerRequest},
    ServiceCollection, ServiceProviderContainer, I,
};

#[derive(Default)]
pub struct Body {
    incomming_message: String,
}

#[derive(Default)]
pub struct Meta {}

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
pub struct Mp<T>(String, PhantomData<T>);
#[async_trait::async_trait]
impl<T> FromRequestBody<ServiceProviderContainer, Body, Meta, String> for Mp<T> {
    type Rejection = Result<String, eyre::Report>;
    async fn from_request(
        req: HandlerRequest<Body, Meta>,
        _state: &ServiceProviderContainer,
    ) -> Result<Self, Self::Rejection> {
        Ok(Mp(req.payload.incomming_message, Default::default()))
    }
}

async fn first_handler_which_only_returns_a_string(I(config): I<Config>) -> String {
    println!(stringify!(first_handler_which_does_nothing));
    println!("config value: {}", config.msg);
    "this is the way we want to do something".into()
}

async fn first_handler_which_does_nothing(I(config): I<Config>) -> Result<String, eyre::Report> {
    println!(stringify!(first_handler_which_does_nothing));
    println!("config value: {}", config.msg);
    Ok("this is the way we want to do something".into())
}

async fn first_handler_which_returns_a_string(
    I(config): I<Config>,
) -> Result<String, eyre::Report> {
    println!(stringify!(first_handler_which_returns_a_string));
    println!("config value: {}", config.msg);
    Ok("this is the way we want to do something".into())
}

async fn first_handler_which_takes_a_message_and_returns_a_string(
    I(config): I<Config>,
    Mp(msg, _): Mp<String>,
) -> Result<String, eyre::Report> {
    println!(stringify!(first_handler_which_returns_a_string));
    println!("config value: {} payload value: {}", config.msg, msg);
    Ok("this is the way we want to do something".into())
}

#[tokio::main]
async fn main() {
    let reg = HandlerRegistry::<Body, Meta, ServiceProviderContainer, String>::default()
        .register(first_handler_which_does_nothing)
        .register(first_handler_which_returns_a_string)
        .register(first_handler_which_takes_a_message_and_returns_a_string)
        .register(first_handler_which_only_returns_a_string);

    let _resp = reg
        .handlers
        .iter()
        .find(|x| x.0 == stringify!(first_handler_which_takes_a_message_and_returns_a_string))
        .unwrap()
        .1
        .call(
            HandlerRequest::<Body, Meta> {
                metadata: Meta {},
                payload: Body {
                    incomming_message: "this is some message".into(),
                },
            },
            ServiceProviderContainer(
                ServiceCollection::default()
                    .reg_cloneable(Config {
                        msg: "this is a const string",
                    })
                    .build_service_provider_arc(),
            ),
        )
        .await;
}
