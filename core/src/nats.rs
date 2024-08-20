// features:
// setting for max concurrent tasks
// we might want to subscribe only for specific topics

use async_nats::{
    jetstream::{self, consumer::PullConsumer, context::CreateStreamErrorKind},
    Subject,
};
use eyre::bail;
use itertools::{self, Itertools};
use std::{collections::HashMap, sync::Arc};
// use color_eyre::owo_colors::OwoColorize;
use futures::StreamExt;
use tokio::task::JoinHandle;

use crate::handler::{HanderCall, HandlerRegistry, HandlerRequest, Response};

#[derive(Clone)]
pub struct NatsPayload {
    pub data: bytes::Bytes,
}
#[derive(Clone)]
pub struct NatsMetadata {}

pub struct NatsDispatcher<P, M, S, R> {
    // message is send to one singele consumer, but there might be multiple consumers
    // use case: send email, process single task
    consumers: HandlerRegistry<P, M, S, R>,
    // different subscibers can subscribe to the same toppic
    // use case: multiple things to do with one data point e.g. one which analyzes the data point and one which saves it.
    subscribers: HandlerRegistry<P, M, S, R>,
    max_concurrent_tasks: usize,
    connection_string: String,
    state: S,
    stream_name: String,
    subscriber_name: String,
}

// we also should implement some means of clean shutdown
impl<S: Clone + 'static + Send, R: 'static + Send> NatsDispatcher<NatsPayload, NatsMetadata, S, R> {
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

    pub async fn start(self) -> Result<(), eyre::Report> {
        self.dispatch_loop().await?;
        Ok(())
    }

    async fn dispatch_loop(self) -> Result<(), eyre::Report> {
        // consumer name is unique
        // but the subscriber name must be different for every instance of the
        let mut all_subjects = Vec::<String>::new();
        let mut subscriber_subjects: Vec<String> = self
            .subscribers
            .handlers
            .iter()
            .map(|x| x.0.clone())
            .collect();
        let mut consumer_subjects: Vec<String> = self
            .consumers
            .handlers
            .iter()
            .map(|x| x.0.clone())
            .collect();

        subscriber_subjects.dedup();
        consumer_subjects.dedup();
        all_subjects.extend_from_slice(&subscriber_subjects);
        all_subjects.extend_from_slice(&consumer_subjects);
        all_subjects.dedup();

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

        log::info!("created consumer");

        let subscriber: PullConsumer = stream
            .create_consumer(jetstream::consumer::pull::Config {
                durable_name: Some(self.subscriber_name),
                ..Default::default()
            })
            .await?;

        log::info!("created subscriber");
        // we build some HashMaps for consumers and subscribers to speed up the handler lookup
        let handler_consumers = self
            .consumers
            .handlers
            .into_iter()
            .collect::<HashMap<String, Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Send + Sync>>>();

        let handler_subscribers = self
            .subscribers
            .handlers
            .into_iter()
            .into_group_map_by(|x| x.0.clone())
            .iter()
            .map(|x| (x.0.clone(), x.1.iter().map(|x| x.1.clone()).collect::<Vec<Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Send + Sync>>>()))
            .collect::<HashMap<
                String,
                Vec<Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Send + Sync>>,
            >>();

        let consuemer_join_handle = start_consumer_dispatcher(
            consumer,
            handler_consumers,
            self.state.clone(),
            self.max_concurrent_tasks,
        );

        let subscriber_join_handle = start_subscriber_dispatcher(
            subscriber,
            handler_subscribers,
            self.state.clone(),
            self.max_concurrent_tasks,
        );

        // let _res = consuemer_join_handle.await;

        let result = tokio::join!(consuemer_join_handle, subscriber_join_handle);

        log::error!(
            "Error during joining of the main dispatch loops: {:?}",
            result
        );

        Ok(())
    }
}

pub fn start_subscriber_dispatcher<S: Clone + Send + 'static, R: Send + 'static>(
    consumer: PullConsumer,
    handler_consumers: HashMap<
        String,
        Vec<Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Send + Sync>>,
    >,
    state: S,
    max_concurrent_tasks: usize,
) -> JoinHandle<bool> {
    let handle = tokio::spawn(async move {
        let mut msg_iter = match consumer.messages().await {
            Ok(x) => x,
            Err(x) => {
                log::error!("Error during creation of the message stream: {}", x);
                return false;
            }
        };
        let mut join_set = tokio::task::JoinSet::<Response<R>>::new();

        loop {
            let async_nats::jetstream::Message {
                message:
                    async_nats::Message {
                        subject,
                        payload,
                        reply,
                        ..
                    },
                context,
            } = match msg_iter.next().await {
                Some(x) => match x {
                    Ok(x) => x,
                    Err(e) => {
                        log::error!("Error in recieved message: {}", e);
                        continue;
                    }
                },
                None => return true,
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

            match ack(&context, &reply).await {
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
            // we cleanup the spawned tasks so when self.max_consurrent_tasks is reached
            if join_set.len() > max_concurrent_tasks {
                log::debug!(
                    "Trigger cleanup event: {} > {}",
                    join_set.len(),
                    max_concurrent_tasks
                );

                while !join_set.is_empty() {
                    log::debug!("join: {}", join_set.len());
                    let Some(x) = join_set.try_join_next() else {
                        break;
                    };
                    log::debug!("cleanup");
                    let _rest = match x {
                        Ok(x) => x,
                        Err(x) => {
                            log::error!("Something went wrong during the join the process: {}", x);
                            continue;
                        }
                    };
                }
            }
        }
    });
    handle
}

pub fn start_consumer_dispatcher<S: Clone + Send + 'static, R: Send + 'static>(
    consumer: PullConsumer,
    handler_consumers: HashMap<
        String,
        Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Send + Sync>,
    >,
    state: S,
    max_concurrent_tasks: usize,
) -> JoinHandle<bool> {
    let handle = tokio::spawn(async move {
        let mut msg_iter = match consumer.messages().await {
            Ok(x) => x,
            Err(x) => {
                log::error!("Error during creation of the message stream: {}", x);
                return false;
            }
        };

        let mut join_set = tokio::task::JoinSet::<Response<R>>::new();

        loop {
            let async_nats::jetstream::Message {
                message:
                    async_nats::Message {
                        subject,
                        payload,
                        reply,
                        ..
                    },
                context,
            } = match msg_iter.next().await {
                Some(x) => match x {
                    Ok(x) => x,
                    Err(e) => {
                        log::error!("Error in recieved message: {}", e);
                        continue;
                    }
                },
                None => return true,
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
                // this might be a problem if this panics before the ack is send, e.g. the email is send, then it crashes.
                match ack(&context, &reply).await {
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
                res
            });

            // we cleanup the spawned tasks so when self.max_consurrent_tasks is reached
            if join_set.len() > max_concurrent_tasks {
                log::debug!(
                    "Trigger cleanup event: {} > {}",
                    join_set.len(),
                    max_concurrent_tasks
                );

                while !join_set.is_empty() {
                    log::debug!("join: {}", join_set.len());
                    let Some(x) = join_set.try_join_next() else {
                        break;
                    };
                    log::debug!("cleanup");
                    let _rest = match x {
                        Ok(x) => x,
                        Err(x) => {
                            log::error!("Something went wrong during the join the process: {}", x);
                            continue;
                        }
                    };
                }
            }
        }
    });

    handle
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
