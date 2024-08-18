use std::{collections::HashMap, marker::PhantomData, pin::Pin};

use futures::Future;

use crate::{ServiceProviderAccess, I};

#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!([], T1);
        $name!([T1], T2);
        $name!([T1, T2], T3);
        $name!([T1, T2, T3], T4);
        $name!([T1, T2, T3, T4], T5);
        $name!([T1, T2, T3, T4, T5], T6);
        $name!([T1, T2, T3, T4, T5, T6], T7);
        $name!([T1, T2, T3, T4, T5, T6, T7], T8);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13], T14);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14], T15);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15], T16);
    };
}

//////////////////////////////  implementations of the DI types \\\\\\\\\\\\\\\\\\\\\\\\\\\\\\

#[async_trait::async_trait]
impl<S, M, R, T: 'static> FromRequestMetadata<S, M, R> for I<T>
where
    S: Send + Sync + ServiceProviderAccess,
{
    type Rejection = Result<R, eyre::Report>;

    async fn from_request_parts(_parts: &mut M, state: &S) -> Result<Self, Self::Rejection> {
        let r = state.get_sp_arc().as_ref();
        Ok(I(r.try_get::<T>().unwrap_or_else(|| {
            panic!(
                "Expected registered type in Dependency Injection: {}",
                std::any::type_name::<T>()
            )
        })))
    }
}

#[async_trait::async_trait]
impl<S, M, R, T: 'static> FromRequestMetadata<S, M, R> for crate::R<T>
where
    S: Send + Sync + ServiceProviderAccess,
{
    type Rejection = Result<R, eyre::Report>;

    async fn from_request_parts(_parts: &mut M, state: &S) -> Result<Self, Self::Rejection> {
        Ok(crate::R::new(crate::ServiceProviderContainer(
            state.get_sp_arc().clone(),
        )))
    }
}

//////////////////////////////  the handler registration  \\\\\\\\\\\\\\\\\\\\\\\\\\\\\\

pub struct HandlerRegistry<P, M, S, R> {
    pub handlers: HashMap<String, Box<dyn HanderCall<P, M, S, R>>>,
}

impl<P, M, S, R> Default for HandlerRegistry<P, M, S, R> {
    fn default() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
}

pub struct HandlerEndpoint<T, H> {
    handler: H,
    pd: PhantomData<T>,
}

#[async_trait::async_trait]
impl<
        T: Sync,
        S: Send + 'static,
        P: Send + 'static,
        M: Send + 'static,
        R: Send + 'static,
        H: Handler<T, S, P, M, R> + Send + Sync,
    > HanderCall<P, M, S, R> for HandlerEndpoint<T, H>
{
    async fn call(&self, req: HandlerRequest<P, M>, state: S) -> Response<R> {
        self.handler.clone().call(req, state).await
    }
}

impl<P: Send + 'static, M: Send + 'static, S: Send + 'static, R: Send + 'static>
    HandlerRegistry<P, M, S, R>
{
    pub fn register<
        N: Into<String>,
        T: Sync + 'static,
        H: Handler<T, S, P, M, R> + Send + Sync + 'static,
    >(
        &mut self,
        name: N,
        handler: H,
    ) {
        self.handlers.insert(
            name.into(),
            Box::new(HandlerEndpoint {
                handler,
                pd: Default::default(),
            }),
        );
    }
}

#[async_trait::async_trait]
pub trait HanderCall<P, M, S, R> {
    async fn call(&self, req: HandlerRequest<P, M>, state: S) -> Response<R>;
}

//////////////////////////////  the actual handler abstractions  \\\\\\\\\\\\\\\\\\\\\\\\\\\\\\

pub struct HandlerRequest<P, M> {
    pub metadata: M,
    pub payload: P,
}

pub trait Handler<T, S, P, M, R>: Clone + Send + Sized + 'static {
    type Future: Future<Output = Response<R>> + Send + 'static;

    fn call(self, req: HandlerRequest<P, M>, state: S) -> Self::Future;
}

// Request gets consumed
#[async_trait::async_trait]
impl<S, P, M, T, R> FromRequestBody<S, P, M, R, private::ViaMetadata> for T
where
    P: Send + 'static,
    M: Send + 'static,
    R: Send + 'static,
    S: Send + Sync,
    T: FromRequestMetadata<S, M, R>,
{
    type Rejection = <Self as FromRequestMetadata<S, M, R>>::Rejection;

    async fn from_request(req: HandlerRequest<P, M>, state: &S) -> Result<Self, Self::Rejection> {
        let (mut metadata, _) = req.into_comps();
        Self::from_request_parts(&mut metadata, state).await
    }
}

// those 2 types are used for some black magic to destinguish between empty handlers and handlers with parameters
mod private {
    #[derive(Debug, Clone, Copy)]
    pub enum ViaMetadata {}

    #[derive(Debug, Clone, Copy)]
    pub enum ViaRequest {}
}

#[async_trait::async_trait]
pub trait FromRequestMetadata<S, M, R>: Sized {
    type Rejection: IntoResponse<R>;
    async fn from_request_parts(metadata: &mut M, state: &S) -> Result<Self, Self::Rejection>;
}

// from request consumes the request, so it is used to get the payload out of the body
#[async_trait::async_trait]
pub trait FromRequestBody<S, P, M, R, A = private::ViaRequest>: Sized {
    type Rejection: IntoResponse<R>;
    async fn from_request(req: HandlerRequest<P, M>, state: &S) -> Result<Self, Self::Rejection>;
}

pub struct Response<P> {
    pub success: bool,
    pub report: Option<eyre::Report>,
    pub payload: Option<P>,
}

pub trait IntoResponse<P> {
    fn into_response(self) -> Response<P>;
}

impl<T> IntoResponse<T> for T {
    fn into_response(self) -> Response<T> {
        Response {
            success: true,
            report: None,
            payload: Some(self),
        }
    }
}

impl<T> IntoResponse<T> for Result<T, eyre::Report> {
    fn into_response(self) -> Response<T> {
        match self {
            Ok(p) => Response {
                success: true,
                report: None,
                payload: Some(p),
            },
            Err(x) => Response {
                success: false,
                report: Some(x),
                payload: None,
            },
        }
    }
}

impl<P, M> HandlerRequest<P, M> {
    #[inline]
    pub fn into_comps(self) -> (M, P) {
        (self.metadata, self.payload)
    }

    #[inline]
    pub fn from_comps(metadata: M, payload: P) -> HandlerRequest<P, M> {
        HandlerRequest { metadata, payload }
    }
}

impl<F, Fut, Res, S, P, M, R> Handler<((),), S, P, M, R> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse<R>,
    P: Send + 'static,
    M: Send + 'static,
    R: Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response<R>> + Send>>;

    fn call(self, _req: HandlerRequest<P, M>, _state: S) -> Self::Future {
        Box::pin(async move { self().await.into_response() })
    }
}

macro_rules! impl_handler {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<F, Fut, S, P, M, Res, R, A, $($ty,)* $last> Handler<(A, $($ty,)* $last,), S, P, M, R> for F
        where
            F: FnOnce($($ty,)* $last,) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Res> + Send,
            P: Send + 'static,
            M: Send + 'static,
            R: Send + 'static,
            S: Send + Sync + 'static,
            Res: IntoResponse<R>,
            $( $ty: FromRequestMetadata<S, M, R> + Send, )*
            $last: FromRequestBody<S, P, M, R, A> + Send,
        {
            type Future = Pin<Box<dyn Future<Output = Response<R>> + Send>>;

            fn call(self, req: HandlerRequest<P, M>, state: S) -> Self::Future {
                Box::pin(async move {
                    let (mut metadata, body) = req.into_comps();
                    let state = &state;

                    $(
                        let $ty = match $ty::from_request_parts(&mut metadata, state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )*

                    let req = HandlerRequest::from_comps(metadata, body);

                    let $last = match $last::from_request(req, state).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response(),
                    };

                    let res = self($($ty,)* $last,).await;

                   res.into_response()
                })
            }
        }
    };
}

all_the_tuples!(impl_handler);
