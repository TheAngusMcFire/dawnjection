use dawnflow::handlers::FromRequestMetadata;

use crate::{ServiceProviderAccess, ServiceProviderContainer, I, R};

#[async_trait::async_trait]
impl<S, M, T: Send + Sync + 'static, R: Send + Sync + 'static> FromRequestMetadata<S, M, R> for I<T>
where
    S: ServiceProviderAccess + Sync + Send,
{
    type Rejection = eyre::Report;
    async fn from_request_metadata(_meta: &mut M, state: &S) -> Result<Self, Self::Rejection> {
        let ret = state.get_sp_arc().get::<T>()?;
        Ok(I(ret))
    }
}

#[async_trait::async_trait]
impl<S, M, T: Send + Sync + 'static, Resp: Send + Sync + 'static> FromRequestMetadata<S, M, Resp>
    for R<T>
where
    S: ServiceProviderAccess + Sync + Send,
{
    type Rejection = eyre::Report;
    async fn from_request_metadata(_meta: &mut M, state: &S) -> Result<Self, Self::Rejection> {
        Ok(R::new(ServiceProviderContainer(state.get_sp_arc().clone())))
    }
}
