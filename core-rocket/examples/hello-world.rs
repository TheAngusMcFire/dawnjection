use std::sync::atomic::AtomicU32;
use dawnjection::{ServiceCollection,IServiceProvider};
use dawnjection_rocket::{I,R};
use rocket::{get, launch, routes};

#[get("/")]
fn hello(msg: R<String>, number: I<u32>) -> String {
    format!("Message: {} CNT: {}", *msg, *number)
}

#[launch]
fn rocket() -> _ {
    let sc = ServiceCollection::default()
        .reg_singleton("Hello world".to_string())
        .reg_singleton(AtomicU32::new(0))
        .reg_mutable_singleton(2i32)
        .reg_factory(|x| {
            let at = x.try_get_ref::<AtomicU32>().unwrap();
            let mut m = x.try_get_mut::<i32>().unwrap();
            *m += 1;
            Some(at.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + *m as u32)
        });

    rocket::build()
    .manage(sc.build_service_provider())
    .mount("/", routes![hello])
}
