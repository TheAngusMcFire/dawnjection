use std::sync::{Arc, Mutex};

use dawnjection::{IServiceProvider, ServiceCollection};

#[derive(Debug, Default, Clone)]
struct SomeService {
    some_handle: usize,
}

impl Drop for SomeService {
    fn drop(&mut self) {
        println!("dropped {}", self.some_handle)
    }
}

#[derive(Default)]
struct SomePool {
    pub insts: Arc<Mutex<Vec<SomeService>>>,
}

fn main() {
    let sc = ServiceCollection::default()
        .reg_singleton(SomePool::default())
        .reg_factory(|sp| {
            let pool = sp.try_get_ref::<SomePool>().unwrap();

            let mut insts = pool.insts.lock().unwrap();
            let ninst = SomeService {
                some_handle: insts.len(),
            };
            insts.push(ninst.clone());

            Some(ninst)
        });

    let sp = sc.build_service_provider();

    for _ in 0..1 {
        println!("{:?}", sp.try_get::<SomeService>().unwrap());
    }

    /* and dispose */
    {
        let mut insts = sp.try_get_ref::<SomePool>().unwrap().insts.lock().unwrap();
        insts.clear();
        assert!(insts.len() == 0);
    }
}
