use dawnjection::handler::{FromRequestBody, FromRequestMetadata, HandlerRegistry, HandlerRequest};

#[derive(Default)]
pub struct Body {
    incomming_message: String,
}

#[derive(Default)]
pub struct Meta {}

#[derive(Default)]
pub struct State {}

pub struct Config {
    msg: &'static str,
}

// trait implementations:
// trait from other crates can only be implemented for types of this crate
// trait implementations for other crates can only be done within the crate its defined

// retrieves data from the Message metadata and the state
#[async_trait::async_trait]
impl FromRequestMetadata<State, Meta, String> for Config {
    type Rejection = Result<String, eyre::Report>;
    async fn from_request_parts(
        _parts: &mut Meta,
        _state: &State,
    ) -> Result<Self, Self::Rejection> {
        Ok(Config {
            msg: "this is some static config",
        })
    }
}

pub struct MessagePayload(String);
#[async_trait::async_trait]
impl FromRequestBody<State, Body, Meta, String> for MessagePayload {
    type Rejection = Result<String, eyre::Report>;
    async fn from_request(
        req: HandlerRequest<Body, Meta>,
        _state: &State,
    ) -> Result<Self, Self::Rejection> {
        Ok(MessagePayload(req.payload.incomming_message))
    }
}

async fn first_handler_which_only_returns_a_string(config: Config) -> String {
    println!(stringify!(first_handler_which_does_nothing));
    println!("config value: {}", config.msg);
    "this is the way we want to do something".into()
}

async fn first_handler_which_does_nothing(config: Config) -> Result<String, eyre::Report> {
    println!(stringify!(first_handler_which_does_nothing));
    println!("config value: {}", config.msg);
    Ok("this is the way we want to do something".into())
}

async fn first_handler_which_returns_a_string(config: Config) -> Result<String, eyre::Report> {
    println!(stringify!(first_handler_which_returns_a_string));
    println!("config value: {}", config.msg);
    Ok("this is the way we want to do something".into())
}

async fn first_handler_which_takes_a_message_and_returns_a_string(
    config: Config,
    MessagePayload(msg): MessagePayload,
) -> Result<String, eyre::Report> {
    println!(stringify!(first_handler_which_returns_a_string));
    println!("config value: {} payload value: {}", config.msg, msg);
    Ok("this is the way we want to do something".into())
}

#[tokio::main]
async fn main() {
    let mut reg = HandlerRegistry::<Body, Meta, State, String>::default();
    let handler_name = stringify!(first_handler_which_does_nothing);
    reg.register(handler_name, first_handler_which_does_nothing);
    reg.register(
        stringify!(first_handler_which_returns_a_string),
        first_handler_which_returns_a_string,
    );

    reg.register(
        stringify!(first_handler_which_takes_a_message_and_returns_a_string),
        first_handler_which_takes_a_message_and_returns_a_string,
    );

    reg.register(
        stringify!(first_handler_which_only_returns_a_string),
        first_handler_which_only_returns_a_string,
    );

    let _resp = reg
        .handlers
        .get(stringify!(
            first_handler_which_takes_a_message_and_returns_a_string
        ))
        .unwrap()
        .call(
            HandlerRequest::<Body, Meta> {
                metadata: Meta {},
                payload: Body {
                    incomming_message: "this is some message".into(),
                },
            },
            State {},
        )
        .await;
}
