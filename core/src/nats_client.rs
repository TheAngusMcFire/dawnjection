use async_nats::{subject::ToSubject, Client, ToServerAddrs};

/*
todo features:
* jetstream publish

*/

#[derive(Clone)]
pub struct NatsClient {
    client: Client,
}

impl NatsClient {
    pub async fn new<A: ToServerAddrs>(address: A) -> Result<Self, eyre::Report> {
        let client = async_nats::connect(address).await?;
        Ok(NatsClient { client })
    }

    pub async fn request<
        Sub: ToSubject,
        Req: serde::Serialize,
        Resp: serde::de::DeserializeOwned,
    >(
        &self,
        subject: Sub,
        value: &Req,
    ) -> Result<Resp, eyre::Report> {
        let bytes = serde_json::to_vec(value)?;
        let msg = self.client.request(subject, bytes.into()).await?;
        let resp = serde_json::from_slice::<Resp>(&msg.payload)?;
        Ok(resp)
    }

    pub async fn publish<Sub: ToSubject, Req: serde::Serialize>(
        &self,
        subject: Sub,
        value: &Req,
    ) -> Result<(), eyre::Report> {
        let bytes = serde_json::to_vec(value)?;
        self.client.publish(subject, bytes.into()).await?;
        Ok(())
    }
}
