use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};

use crate::{ServiceProviderContainer, I, R};

impl<S, T> FromRequestParts<S> for I<T>
where
    S: Send + Sync,
    T: Send + Sync + 'static,
    ServiceProviderContainer: FromRef<S>,
{
    type Rejection = StatusCode;

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let r = ServiceProviderContainer::from_ref(state);
        Ok(I(r.0.try_get::<T>().unwrap()))
    }
}

impl<S, T: 'static> FromRequestParts<S> for R<T>
where
    S: Send + Sync,
    ServiceProviderContainer: FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(R::new(ServiceProviderContainer::from_ref(state)))
    }
}
