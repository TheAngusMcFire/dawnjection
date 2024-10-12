// features:
// setting for max concurrent tasks
// we might want to subscribe only for specific topics

use async_nats::{
    jetstream::{self, consumer::PullConsumer, Context},
    Client, Message, Subject,
};
use eyre::bail;
use itertools::{self, Itertools};
use std::{collections::HashMap, sync::Arc, time::Duration};

use futures::StreamExt;
use tokio::task::JoinSet;

use crate::handler::{
    FromRequestBody, HanderCall, HandlerRegistry, HandlerRequest, IntoResponse, Response,
};

#[derive(Clone)]
pub struct NatsPayload {
    pub data: bytes::Bytes,
}

#[derive(Clone)]
pub struct NatsResponse {
    pub data: Option<bytes::Bytes>,
}

#[derive(Clone)]
pub struct NatsMetadata {}

pub struct NatsDispatcher<P, M, S, R> {
    /// message is send to one singele consumer, but there might be multiple consumers
    /// use case: send email, process single task
    consumers: HandlerRegistry<P, M, S, R>,
    /// different subscibers can subscribe to the same toppic
    /// use case: multiple things to do with one data point e.g. one which analyzes the data point and one which saves it.
    subscribers: HandlerRegistry<P, M, S, R>,
    /// the max amount of tasks to spawn (the same for subscriber and consumer, so if the value is 8 16 tasks can run at the same time)
    max_concurrent_tasks: usize,
    connection_string: String,
    state: S,
    stream_name: String,
    subscriber_name: String,
}

#[async_trait::async_trait]
pub trait IntoNatsResponse {
    async fn into_nats_response(self) -> NatsResponse;
}

#[async_trait::async_trait]
impl IntoNatsResponse for () {
    async fn into_nats_response(self) -> NatsResponse {
        NatsResponse { data: None }
    }
}

#[async_trait::async_trait]
impl IntoNatsResponse for String {
    async fn into_nats_response(self) -> NatsResponse {
        NatsResponse {
            data: Some(self.into()),
        }
    }
}

#[async_trait::async_trait]
impl IntoNatsResponse for bytes::Bytes {
    async fn into_nats_response(self) -> NatsResponse {
        NatsResponse { data: Some(self) }
    }
}

impl IntoResponse<NatsResponse> for eyre::Report {
    fn into_response(self) -> Response<NatsResponse> {
        Response {
            success: false,
            report: Some(self),
            payload: None,
        }
    }
}

impl From<()> for NatsResponse {
    fn from(_value: ()) -> Self {
        Self { data: None }
    }
}

#[cfg(feature = "serde_json_requests")]
#[async_trait::async_trait]
impl<S, T> FromRequestBody<S, NatsPayload, NatsMetadata, NatsResponse> for T
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned,
{
    type Rejection = eyre::Report;

    async fn from_request(
        req: HandlerRequest<NatsPayload, NatsMetadata>,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        return match serde_json::from_slice::<T>(&req.payload.data.slice(..)) {
            Ok(value) => Ok(value),
            Err(error) => Err(eyre::eyre!("error during object deserialisation: {error}")),
        };
    }
}

#[async_trait::async_trait]
trait NatsMessageProvider {
    async fn get_nats_message(
        &mut self,
    ) -> Result<
        Option<(
            Subject,
            Option<Subject>,
            bytes::Bytes,
            Box<dyn NatsMessageAcker>,
        )>,
        eyre::Report,
    >;
}

#[async_trait::async_trait]
trait NatsMessageAcker: Send + Sync {
    async fn ack_message(&self, payload: Option<bytes::Bytes>) -> Result<(), eyre::Report>;
}

//////////////////////////////////////// Default Nats Message Provider \\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\

struct DefaultNatsMessageProvider {
    client: Arc<Client>,
    provider: async_nats::Subscriber,
}

struct DefaultNatsMessageAcker {
    reply: Option<Subject>,
    client: Arc<Client>,
}

#[async_trait::async_trait]
impl NatsMessageAcker for DefaultNatsMessageAcker {
    async fn ack_message(&self, payload: Option<bytes::Bytes>) -> Result<(), eyre::Report> {
        if let (Some(ref reply), Some(payload)) = (&self.reply, payload) {
            self.client.publish(reply.clone(), payload).await?;
            return Ok(());
        } else {
            // bail!("No reply subject, not a JetStream message");
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl NatsMessageProvider for DefaultNatsMessageProvider {
    async fn get_nats_message(
        &mut self,
    ) -> Result<
        Option<(
            Subject,
            Option<Subject>,
            bytes::Bytes,
            Box<dyn NatsMessageAcker>,
        )>,
        eyre::Report,
    > {
        let msg = self.provider.next().await;
        let Some(Message {
            subject,
            reply,
            payload,
            ..
        }) = msg
        else {
            return Ok(None);
        };
        Ok(Some((
            subject,
            reply.clone(),
            payload,
            Box::new(DefaultNatsMessageAcker {
                reply,
                client: self.client.clone(),
            }),
        )))
    }
}

//////////////////////////////////////// jetstream Nats Message Provider \\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\

struct JetStreamNatsMessageProvider {
    stream: jetstream::consumer::pull::Stream,
}

impl JetStreamNatsMessageProvider {
    async fn new(consumer: PullConsumer) -> Result<Self, eyre::Report> {
        let msg_iter = consumer.messages().await?;
        Ok(Self { stream: msg_iter })
    }
}

struct JetStreamNatsMessageAcker {
    reply: Option<Subject>,
    context: Context,
}

#[async_trait::async_trait]
impl NatsMessageAcker for JetStreamNatsMessageAcker {
    async fn ack_message(&self, payload: Option<bytes::Bytes>) -> Result<(), eyre::Report> {
        if let Some(ref reply) = &self.reply {
            self.context
                .publish(
                    reply.clone(),
                    if let Some(x) = payload { x } else { "".into() },
                )
                .await?;
            return Ok(());
        } else {
            bail!("No reply subject, not a JetStream message");
        }
    }
}

#[async_trait::async_trait]
impl NatsMessageProvider for JetStreamNatsMessageProvider {
    async fn get_nats_message(
        &mut self,
    ) -> Result<
        Option<(
            Subject,
            Option<Subject>,
            bytes::Bytes,
            Box<dyn NatsMessageAcker>,
        )>,
        eyre::Report,
    > {
        let async_nats::jetstream::Message {
            message:
                async_nats::Message {
                    subject,
                    payload,
                    reply,
                    ..
                },
            context,
        } = match self.stream.next().await {
            Some(x) => match x {
                Ok(x) => x,
                Err(e) => return Err(e.into()),
            },
            None => return Ok(None),
        };
        Ok(Some((
            subject,
            reply.clone(),
            payload,
            Box::new(JetStreamNatsMessageAcker { reply, context }),
        )))
    }
}

// we also should implement some means of clean shutdown
impl<S: Clone + 'static + Send, R: IntoNatsResponse + 'static + Send>
    NatsDispatcher<NatsPayload, NatsMetadata, S, R>
{
    pub fn new(
        consumers: HandlerRegistry<NatsPayload, NatsMetadata, S, R>,
        subscribers: HandlerRegistry<NatsPayload, NatsMetadata, S, R>,
        connection_string: &str,
        max_concurrent_tasks: usize,
        state: S,
        stream_name: String,
        // this needs to be unique for ervery Service which wants to subscribe to topics
        subscriber_name: String,
    ) -> Self {
        Self {
            consumers,
            subscribers,
            max_concurrent_tasks,
            connection_string: connection_string.into(),
            state,
            stream_name,
            subscriber_name,
        }
    }

    pub async fn start_jetstrem_dispatching(self) -> Result<(), eyre::Report> {
        // consumer name is unique
        // but the subscriber name must be different for every instance of the
        let (all_subjects, subscriber_subjects, consumer_subjects) =
            extract_unique_subjects(&self.consumers, &self.subscribers);

        log::info!("all subjects:        {:?}", all_subjects);
        log::info!("subscriber_subjects: {:?}", subscriber_subjects);
        log::info!("conumer_subjects:    {:?}", consumer_subjects);

        let client = async_nats::connect(&self.connection_string).await?;
        let jetstream = jetstream::new(client);

        let stream = jetstream
            .get_or_create_stream(jetstream::stream::Config {
                name: self.stream_name.clone(),
                retention: jetstream::stream::RetentionPolicy::Interest,
                subjects: all_subjects,
                ..Default::default()
            })
            .await?;

        log::info!("created stream");

        let consumer: PullConsumer = stream
            .create_consumer(jetstream::consumer::pull::Config {
                durable_name: Some("consumer".to_string()),
                filter_subjects: consumer_subjects,
                ..Default::default()
            })
            .await?;

        log::info!("created consumer whith name: consumer");

        let subscriber: PullConsumer = stream
            .create_consumer(jetstream::consumer::pull::Config {
                durable_name: Some(self.subscriber_name.clone()),
                ..Default::default()
            })
            .await?;

        log::info!("created subscriber with name: {}", self.subscriber_name);

        let handler_consumers = build_consumer_handlers(&self.consumers);

        let handler_subscribers = build_subscriber_handlers(&self.subscribers);

        let provider = JetStreamNatsMessageProvider::new(consumer).await?;
        let state = self.state.clone();
        let consuemer_join_handle = tokio::spawn(async move {
            start_consumer_dispatcher(
                provider,
                handler_consumers,
                state,
                self.max_concurrent_tasks,
            )
            .await
        });

        let provider = JetStreamNatsMessageProvider::new(subscriber).await?;
        let state = self.state.clone();
        let subscriber_join_handle = tokio::spawn(async move {
            start_subscriber_dispatcher(
                provider,
                handler_subscribers,
                state,
                self.max_concurrent_tasks,
            )
            .await
        });

        let result = tokio::join!(consuemer_join_handle, subscriber_join_handle);

        log::error!(
            "Error during joining of the main dispatch loops: {:?}",
            result
        );

        Ok(())
    }

    pub async fn start_nats_dispatching(self) -> Result<(), eyre::Report> {
        let (all_subjects, subscriber_subjects, consumer_subjects) =
            extract_unique_subjects(&self.consumers, &self.subscribers);

        log::info!("all subjects:        {:?}", all_subjects);
        log::info!("subscriber_subjects: {:?}", subscriber_subjects);
        log::info!("conumer_subjects:    {:?}", consumer_subjects);

        // if !consumer_subjects.is_empty() {
        //     panic!("Consumers not supported for the default nats dispatcher, only subscriptions")
        // }

        let client = Arc::new(async_nats::connect(&self.connection_string).await?);

        let handler_consumers = build_consumer_handlers(&self.consumers);
        let handler_subscribers = build_subscriber_handlers(&self.subscribers);

        let mut join_set = JoinSet::new();

        // start subscribers
        for s in subscriber_subjects {
            let client = client.clone();
            let handlers = handler_subscribers.clone();
            let provider = DefaultNatsMessageProvider {
                provider: client.subscribe(s).await?,
                client,
            };
            let state = self.state.clone();
            join_set.spawn(async move {
                start_subscriber_dispatcher(provider, handlers, state, self.max_concurrent_tasks)
                    .await
            });
        }

        // start consumers
        for s in consumer_subjects {
            let client = client.clone();
            let handlers = handler_consumers.clone();
            let provider = DefaultNatsMessageProvider {
                provider: client.subscribe(s).await?,
                client,
            };
            let state = self.state.clone();
            join_set.spawn(async move {
                start_consumer_dispatcher(provider, handlers, state, self.max_concurrent_tasks)
                    .await
            });
        }

        while (join_set.join_next().await).is_some() {}

        Ok(())
    }
}

fn extract_unique_subjects<S: Send + 'static, R: Send + 'static>(
    consumers: &HandlerRegistry<NatsPayload, NatsMetadata, S, R>,
    subscribers: &HandlerRegistry<NatsPayload, NatsMetadata, S, R>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut all_subjects = Vec::<String>::new();
    let mut subscriber_subjects: Vec<String> =
        subscribers.handlers.iter().map(|x| x.0.clone()).collect();
    let mut consumer_subjects: Vec<String> =
        consumers.handlers.iter().map(|x| x.0.clone()).collect();

    subscriber_subjects.dedup();
    consumer_subjects.dedup();
    all_subjects.extend_from_slice(&subscriber_subjects);
    all_subjects.extend_from_slice(&consumer_subjects);
    all_subjects.dedup();
    (all_subjects, subscriber_subjects, consumer_subjects)
}

fn build_consumer_handlers<S: Send + 'static, R: Send + 'static>(
    consumers: &HandlerRegistry<NatsPayload, NatsMetadata, S, R>,
) -> HashMap<String, Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Sync + Send>> {
    let handler_consumers = consumers
            .handlers
            .iter()
            .map(|(x, y)| (x.clone(), y.clone()))
            .collect::<HashMap<String, Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Send + Sync>>>();
    handler_consumers
}

fn build_subscriber_handlers<S: Send + 'static, R: Send + 'static>(
    consumers: &HandlerRegistry<NatsPayload, NatsMetadata, S, R>,
) -> HashMap<String, Vec<Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Sync + Send>>> {
    let handler_subscribers = consumers
            .handlers
            .iter()
            .into_group_map_by(|x| x.0.clone())
            .iter()
            .map(|x| (x.0.clone(), x.1.iter().map(|x| x.1.clone()).collect::<Vec<Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Send + Sync>>>()))
            .collect::<HashMap<
                String,
                Vec<Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Send + Sync>>,
            >>();
    handler_subscribers
}

async fn start_subscriber_dispatcher<
    Mp: NatsMessageProvider + Send + 'static,
    S: Clone + Send + 'static,
    R: IntoNatsResponse + Send + 'static,
>(
    mut message_provider: Mp,
    handler_consumers: HashMap<
        String,
        Vec<Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Send + Sync>>,
    >,
    state: S,
    max_concurrent_tasks: usize,
) -> bool {
    // let handle = tokio::spawn(async move {
    let mut join_set = tokio::task::JoinSet::<Response<R>>::new();

    loop {
        let (subject, reply, payload, acker) = match message_provider.get_nats_message().await {
            Ok(Some(x)) => x,
            Ok(None) => todo!("hm, not really shure when this is supposed to happen..."),
            Err(e) => {
                log::error!("Error in recieved message: {}", e);
                continue;
            }
        };

        let subject = subject.as_str();
        let handlers = match handler_consumers.get(subject) {
            Some(x) => x.clone(),
            None => {
                log::error!("There is no registered handler for subject: {}", subject);
                continue;
            }
        };

        let (pl, mt) = (NatsPayload { data: payload }, NatsMetadata {});
        let req = HandlerRequest {
            payload: pl,
            metadata: mt,
        };

        // the check is to prevent to clone the request if only one copy is needed
        if handlers.len() == 1 {
            let handler = handlers.first().expect("there should be a check").clone();
            let state = state.clone();
            join_set.spawn(async move { handler.call(req, state).await });
        } else {
            for handler in handlers {
                let state = state.clone();
                let req = req.clone();
                join_set.spawn(async move { handler.call(req, state).await });
            }
        }

        // we ack all of the subjects at once, retransmissions of whole subjects might lead to more problems
        // because every subscriber would need to handle retransmission logic
        match acker.ack_message(None).await {
            Ok(_) => {}
            // we probably want to retry the ack if we got to this point
            Err(x) => {
                log::error!(
                    "Error during acking (message id: {:?}) of the message: {}",
                    reply,
                    x
                );
            }
        }

        // perform garbage collection, everything after this while is running probably
        let mut wait_loop = false;
        loop {
            while let Some(x) = join_set.try_join_next() {
                // r.f.u maybe we can do something with the reponse in the future
                let _rest = match x {
                    Ok(x) => x,
                    Err(x) => {
                        log::error!("Something went wrong during the join the process: {}", x);
                        continue;
                    }
                };
            }

            // if there are more active tasks in the queue, we wait until there is space
            if join_set.len() < max_concurrent_tasks {
                break;
            } else if !wait_loop {
                wait_loop = true;
                log::info!("Too much running Tasks, wait for some to finish");
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    // });
    // handle
}

async fn start_consumer_dispatcher<
    Mp: NatsMessageProvider + Send + 'static,
    S: Clone + Send + 'static,
    R: IntoNatsResponse + Send + 'static,
>(
    mut message_provider: Mp,
    handler_consumers: HashMap<
        String,
        Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Send + Sync>,
    >,
    state: S,
    max_concurrent_tasks: usize,
) -> bool {
    // let handle = tokio::spawn(async move {
    let mut join_set = tokio::task::JoinSet::<()>::new();

    loop {
        let (subject, _reply, payload, acker) = match message_provider.get_nats_message().await {
            Ok(Some(x)) => x,
            Ok(None) => todo!("hm, not really shure when this is supposed to happen..."),
            Err(e) => {
                log::error!("Error in recieved message: {}", e);
                continue;
            }
        };

        let subject = subject.as_str();
        let handler = match handler_consumers.get(subject) {
            Some(x) => x.clone(),
            None => {
                log::error!("There is no registered handler for subject: {}", subject);
                continue;
            }
        };

        let (pl, mt) = (NatsPayload { data: payload }, NatsMetadata {});
        // this is costly, so more than an arc is not acceptible
        let state = state.clone();

        join_set.spawn(async move {
            let res = handler
                .call(
                    HandlerRequest {
                        payload: pl,
                        metadata: mt,
                    },
                    state,
                )
                .await;

            let payload = if let Some(x) = res.payload {
                x.into_nats_response().await.data
            } else {
                None
            };

            match acker.ack_message(payload).await {
                Ok(_) => {}
                // we probably want to retry the ack if we got to this point
                Err(x) => {
                    log::error!("Error during acking of the message: {}", x);
                }
            }
        });

        // perform garbage collection, everything after this while is running probably
        let mut wait_loop = false;
        loop {
            while let Some(x) = join_set.try_join_next() {
                // r.f.u maybe we can do something with the reponse in the future
                match x {
                    Ok(x) => x,
                    Err(x) => {
                        log::error!("Something went wrong during the join the process: {}", x);
                        continue;
                    }
                };
            }

            // if there are more active tasks in the queue, we wait until there is space
            if join_set.len() < max_concurrent_tasks {
                break;
            } else if !wait_loop {
                wait_loop = true;
                log::info!("Too much running Tasks, wait for some to finish");
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    // });

    // handle
}

pub async fn ack(
    context: &async_nats::jetstream::Context,
    reply: &Option<Subject>,
) -> Result<(), eyre::Report> {
    if let Some(ref reply) = reply {
        context.publish(reply.clone(), "".into()).await?;
        Ok(())
    } else {
        bail!("No reply subject, not a JetStream message");
    }
}

#[cfg(test)]
mod test {
    async fn this_is_a_test() {}

    #[test]
    fn name_test() {
        print_name(this_is_a_test);
    }

    fn print_name<T>(_: T) {
        println!(
            "name of the functions: {}",
            std::any::type_name::<T>().split("::").last().unwrap()
        )
    }
}
