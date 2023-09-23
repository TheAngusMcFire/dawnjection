use std::ops::Deref;

use dawnjection::ServiceProvider;
use rocket::{request::{FromRequest, self}, Request, outcome::Outcome, http::Status, State};
use dawnjection::IServiceProvider;
use rocket::outcome::Outcome::Failure;

pub struct I<T>{
    data: T
}

impl<T> I<T> {
    pub fn get(self) -> T {
        self.data
    }
}

#[rocket::async_trait]
impl<'r, T: 'static> FromRequest<'r> for I<T> {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, (Status, ()), ()>
    {
        return match request.guard::<&State<ServiceProvider>>().await {
            Outcome::Success(x) => {
                match x.try_get::<T>() {
                    Some(x) => Outcome::Success(I{data: x}),
                    _ => Failure((Status::InternalServerError,()))  
                }
            },
            _ => Failure((Status::InternalServerError,())) 
        };
    }
}

pub struct R<'r, T> {
    data: &'r T
}

impl<'r, T> R<'r, T>{
    pub fn get(&self) -> &T{
        self.data
    } 
}

#[rocket::async_trait]
impl<'r, T: 'static> FromRequest<'r> for R<'r, T> {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, ()>
    {
        return match request.guard::<&State<ServiceProvider>>().await {
            Outcome::Success(x) => {
                match x.try_get_ref::<T>() {
                    Some(x) => Outcome::Success(R{data: x}),
                    _ => Failure((Status::InternalServerError,()))  
                }
            },
            _ => Failure((Status::InternalServerError,())) 
        };
    }
}


impl<T> Deref for I<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'r, T> Deref for R<'r, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}