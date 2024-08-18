use std::{collections::HashMap, marker::PhantomData, pin::Pin};

use futures::Future;

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

#[derive(Default)]
pub struct HandlerRegistry<P, M, S> {
    pub handlers: HashMap<String, Box<dyn HanderCall<P, M, S>>>,
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
        H: Handler<T, S, P, M> + Send + Sync,
    > HanderCall<P, M, S> for HandlerEndpoint<T, H>
{
    async fn call(&self, req: Request<P, M>, state: S) -> Response {
        self.handler.clone().call(req, state).await
    }
}

impl<P: Send + 'static, M: Send + 'static, S: Send + 'static> HandlerRegistry<P, M, S> {
    pub fn register<
        N: Into<String>,
        T: Sync + 'static,
        H: Handler<T, S, P, M> + Send + Sync + 'static,
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
// Body, the body of the request, defaults to body
// State, the state of the entire service

#[async_trait::async_trait]
pub trait HanderCall<P, M, S> {
    async fn call(&self, req: Request<P, M>, state: S) -> Response;
}

pub struct Request<P, M> {
    pub metadata: M,
    pub payload: P,
}

pub trait Handler<T, S, P, M>: Clone + Send + Sized + 'static {
    type Future: Future<Output = Response> + Send + 'static;

    fn call(self, req: Request<P, M>, state: S) -> Self::Future;
}

// Request gets consumed
#[async_trait::async_trait]
impl<S, P, M, T> FromRequestBody<S, P, M, private::ViaMetadata> for T
where
    P: Send + 'static,
    M: Send + 'static,
    S: Send + Sync,
    T: FromRequestMetadata<S, M>,
{
    type Rejection = <Self as FromRequestMetadata<S, M>>::Rejection;

    async fn from_request(req: Request<P, M>, state: &S) -> Result<Self, Self::Rejection> {
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
pub trait FromRequestMetadata<S, M>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    async fn from_request_parts(metadata: &mut M, state: &S) -> Result<Self, Self::Rejection>;
}

// from request consumes the request, so it is used to get the payload out of the body
#[async_trait::async_trait]
pub trait FromRequestBody<S, P, M, A = private::ViaRequest>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    async fn from_request(req: Request<P, M>, state: &S) -> Result<Self, Self::Rejection>;
}

pub struct Response {}

pub trait IntoResponse {
    /// Create a response.
    fn into_response(self) -> Response;
}

impl IntoResponse for () {
    fn into_response(self) -> Response {
        Response {}
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        Response {}
    }
}

impl<P, M> Request<P, M> {
    #[inline]
    pub fn into_comps(self) -> (M, P) {
        (self.metadata, self.payload)
    }

    #[inline]
    pub fn from_comps(metadata: M, payload: P) -> Request<P, M> {
        Request { metadata, payload }
    }
}

impl<F, Fut, Res, S, P, M> Handler<((),), S, P, M> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    P: Send + 'static,
    M: Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, _req: Request<P, M>, _state: S) -> Self::Future {
        Box::pin(async move { self().await.into_response() })
    }
}

macro_rules! impl_handler {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<F, Fut, S, P, M, Res, A, $($ty,)* $last> Handler<(A, $($ty,)* $last,), S, P, M> for F
        where
            F: FnOnce($($ty,)* $last,) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Res> + Send,
            P: Send + 'static,
            M: Send + 'static,
            S: Send + Sync + 'static,
            Res: IntoResponse,
            $( $ty: FromRequestMetadata<S, M> + Send, )*
            $last: FromRequestBody<S, P, M, A> + Send,
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

            fn call(self, req: Request<P, M>, state: S) -> Self::Future {
                Box::pin(async move {
                    let (mut metadata, body) = req.into_comps();
                    let state = &state;

                    $(
                        let $ty = match $ty::from_request_parts(&mut metadata, state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )*

                    let req = Request::from_comps(metadata, body);

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
