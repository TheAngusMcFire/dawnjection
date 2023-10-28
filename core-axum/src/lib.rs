use std::sync::Arc;

use axum::{extract::{FromRequestParts, FromRef}, http::{StatusCode, request::Parts}, async_trait};
use dawnjection::{ServiceProvider,IServiceProvider};

#[derive(Clone)]
pub struct AxumServiceProvider(pub Arc<ServiceProvider>);

pub struct I<T>(pub T);


impl<T> I<T> {
    pub fn get(self) -> T {
        self.0
    }
}

#[async_trait]
impl<S, T: 'static> FromRequestParts<S> for I<T> 
where
    S: Send + Sync,
    AxumServiceProvider: FromRef<S>
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let r = AxumServiceProvider::from_ref(state);
        Ok(I(r.0.try_get::<T>().unwrap()))
    }
}
