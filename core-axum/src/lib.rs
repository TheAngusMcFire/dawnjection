use std::{marker::PhantomData, sync::Arc};

use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use dawnjection::{IServiceProvider, ServiceProvider};

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
    AxumServiceProvider: FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let r = AxumServiceProvider::from_ref(state);
        Ok(I(r.0.try_get::<T>().unwrap()))
    }
}

pub struct R<T: 'static> {
    provider: AxumServiceProvider,
    phantom: PhantomData<T>,
}

impl<T> R<T> {
    fn new(provider: AxumServiceProvider) -> Self {
        Self {
            provider,
            phantom: PhantomData,
        }
    }

    pub fn get(&self) -> Option<&T> {
        self.provider.0.try_get_ref()
    }
}

#[async_trait]
impl<S, T: 'static> FromRequestParts<S> for R<T>
where
    S: Send + Sync,
    AxumServiceProvider: FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(R::new(AxumServiceProvider::from_ref(state)))
    }
}
