use std::{net::SocketAddr, sync::{Arc, atomic::AtomicU32}};

use axum::{response::Html, Router, routing::get};
use dawnjection::ServiceCollection;
use dawnjection_axum::{I, AxumServiceProvider};

async fn handler(I(msg): I<Arc<AtomicU32>>) -> Html<String> {
    Html(format!("<h1>Hello, World! {}</h1>", msg.fetch_add(1, std::sync::atomic::Ordering::Relaxed)))
}


#[tokio::main]
async fn main() {
     // build our application with a route
     let app = Router::new()
        .route("/", get(handler))
        .with_state(AxumServiceProvider(ServiceCollection::default()
            .reg_cloneable(Arc::new(AtomicU32::new(0)))
            .build_service_provider_arc()))
        ;

     let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
     println!("listening on {}", addr);
     axum::Server::bind(&addr)
         .serve(app.into_make_service())
         .await
         .unwrap();
}