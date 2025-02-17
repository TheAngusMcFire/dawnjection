use std::sync::{atomic::AtomicU32, Arc};

use dawnflow::{
    in_memory::{InMemoryMetadata, InMemoryPayload, InMemoryResponse},
    registry::HandlerRegistry,
};
use dawnjection::{ServiceCollection, ServiceProviderContainer, I, R};

pub struct SomeSingleton(pub String);

pub enum Consumeable {}

async fn handler(I(msg): I<Arc<AtomicU32>>, singleton: R<SomeSingleton>) -> eyre::Result<()> {
    Ok(())
}

#[tokio::main]
async fn main() {
    let sc = ServiceProviderContainer(
        ServiceCollection::default()
            .reg_cloneable(Arc::new(AtomicU32::new(0)))
            .reg_singleton(SomeSingleton("singleton message".to_string()))
            .build_service_provider_arc(),
    );
    let handlers = HandlerRegistry::<
        InMemoryPayload,
        InMemoryMetadata,
        ServiceProviderContainer,
        InMemoryResponse,
    >::default()
    .register_subscriber::<Consumeable, _, _>(handler);
}
