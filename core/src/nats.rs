// features:
// setting for max concurrent tasks
// we might want to subscribe only for specific topics

use std::{collections::HashMap, str::Bytes, sync::Arc};

use async_nats::{
    header::IntoHeaderName,
    jetstream::{
        self,
        consumer::{self, PullConsumer},
    },
    Message, Subject,
};
use eyre::bail;
// use color_eyre::owo_colors::OwoColorize;
use futures::StreamExt;

use crate::handler::{HanderCall, HandlerRegistry, HandlerRequest};

pub struct NatsPayload {
    data: bytes::Bytes,
}
pub struct NatsMetadata {}

pub struct NatsDispatcher<P, M, S, R> {
    // message is send to one singele consumer, but there might be multiple consumers
    // use case: send email, process single task
    consumers: HandlerRegistry<P, M, S, R>,
    // different subscibers can subscribe to the same toppic
    // use case: multiple things to do with one data point e.g. one which analyzes the data point and one which saves it.
    subscribers: HandlerRegistry<P, M, S, R>,
    max_concurrent_tasks: u32,
    connection_string: String,
    state: S,
    stream_name: String,
}

impl<S: Clone + 'static + Send, R: 'static + Send> NatsDispatcher<NatsPayload, NatsMetadata, S, R> {
    pub fn new(
        consumers: HandlerRegistry<NatsPayload, NatsMetadata, S, R>,
        subscribers: HandlerRegistry<NatsPayload, NatsMetadata, S, R>,
        connection_string: &str,
        max_concurrent_tasks: u32,
        state: S,
        stream_name: String,
    ) -> Self {
        Self {
            consumers,
            subscribers,
            max_concurrent_tasks,
            connection_string: connection_string.into(),
            state,
            stream_name,
        }
    }

    pub async fn start(self) -> Result<(), eyre::Report> {
        self.dispatch_loop().await?;
        return Ok(());
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

        log::info!("all subjects:       {:?}", all_subjects);
        log::info!("subscriber_subjects {:?}", subscriber_subjects);
        log::info!("conumer_subjects    {:?}", consumer_subjects);

        let client = async_nats::connect(&self.connection_string).await?;
        let jetstream = jetstream::new(client);
        let stream = jetstream
            .create_stream(jetstream::stream::Config {
                name: self.stream_name.clone(),
                retention: jetstream::stream::RetentionPolicy::Interest,
                subjects: all_subjects,
                ..Default::default()
            })
            .await?;

        log::info!("created stream");

        // the next thing is not race condition safe... this is really not good
        let mut consumer_names = Vec::<String>::new();
        let mut cni = stream.consumer_names();
        while let Some(x) = cni.next().await {
            dbg!(&x);
            consumer_names.push(x?);
        }

        log::info!("read consumer names: {:?}", consumer_names);

        let subscriber_name = (0..1000)
            .map(|_| format!("subscriber{}", uuid::Uuid::new_v4()))
            .find(|x| !consumer_names.contains(x))
            .expect(
                "OMG, the odds of that happening are super small, you are the luckiest person ever",
            );

        log::info!("used supscriber name: {}", subscriber_name);

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
                durable_name: Some(subscriber_name),
                ..Default::default()
            })
            .await?;

        log::info!("created subscriber");
        let handler_consumers = self
            .consumers
            .handlers
            .into_iter()
            .collect::<HashMap<String, Arc<dyn HanderCall<NatsPayload, NatsMetadata, S, R> + Send + Sync>>>();

        let consumer_handle = tokio::spawn(async move {
            let mut msg_iter = match consumer.messages().await {
                Ok(x) => x,
                Err(x) => {
                    log::error!("Error during creation of the message stream: {}", x);
                    return false;
                }
            };

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

                print!("-");
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
                let state = self.state.clone();

                let msg_handle = tokio::spawn(async move {
                    let res = handler
                        .call(
                            HandlerRequest {
                                payload: pl,
                                metadata: mt,
                            },
                            state,
                        )
                        .await;
                    match ack(&context, reply).await {
                        Ok(_) => {}
                        Err(_) => todo!(),
                    }
                    res
                });
                // msg_handle.await.unwrap();
            }
        });

        let rest = consumer_handle.await;

        // println!("consumer name: {:?}", stream.consumer_names().next().await);
        // println!(
        //     "consumer name: {:?}",
        //     stream.consumer_names().skip(1).next().await
        // );
        // println!(
        //     "consumer name: {:?}",
        //     stream.consumer_names().skip(2).next().await
        // );

        // let consumer: PullConsumer = stream
        //     .create_consumer(jetstream::consumer::pull::Config {
        //         durable_name: Some("consumer2".to_string()),
        //         ..Default::default()
        //     })
        //     .await?; // client.

        // while let Some(msg) = consumer.messages().await?.next().await {
        //     let msg = msg?;
        //     dbg!(&msg.subject);
        //     dbg!(&msg.message);
        //     let ret = msg.ack().await;
        // }

        Ok(())
    }
}

pub async fn ack(
    context: &async_nats::jetstream::Context,
    reply: Option<Subject>,
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
