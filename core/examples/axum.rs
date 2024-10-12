use std::sync::{atomic::AtomicU32, Arc};

use axum::{response::Html, routing::get, Router};
use dawnjection::{ServiceCollection, ServiceProviderContainer, I, R};

pub struct SomeSingleton(pub String);

async fn handler(I(msg): I<Arc<AtomicU32>>, singleton: R<SomeSingleton>) -> Html<String> {
    Html(format!(
        "<h1>Counter: {}</h1><h2>Singleton: {:?}</h2>",
        msg.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        singleton.get().0
    ))
}

#[tokio::main]
async fn main() {
    // build our application with a route
    let app = Router::new()
        .route("/", get(handler))
        .with_state(ServiceProviderContainer(
            ServiceCollection::default()
                .reg_cloneable(Arc::new(AtomicU32::new(0)))
                .reg_singleton(SomeSingleton("singleton message".to_string()))
                .build_service_provider_arc(),
        ));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
