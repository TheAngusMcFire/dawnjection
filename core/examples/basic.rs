use std::sync::{Mutex, Arc};

use dawnjection::{ServiceCollection, IServiceProvider};

#[derive(Debug, Default, Clone)]
struct SomeService {
    some_handle : usize
}

#[derive(Default)]
struct SomePool {
    pub insts : Arc<Mutex<Vec<SomeService>>>
}


fn main() {
    let mut sc = ServiceCollection::default();

    sc.reg_singleton(SomePool::default());
    sc.reg_factory(|sp| {
        let pool = sp.try_get_ref::<SomePool>().unwrap();

        let mut insts = pool.insts.lock().unwrap();
        let ninst = SomeService {some_handle: insts.len()};
        insts.push(ninst.clone());

        return Some(ninst);
    });

    let sp = sc.build_service_provider();

    for _ in 0..20 {
        println!("{:?}", sp.try_get::<SomeService>().unwrap());
    }

    /* and dispose */
    {
        let mut insts = sp.try_get_ref::<SomePool>().unwrap().insts.lock().unwrap();
        insts.clear();
        assert!(insts.len() == 0);
    }

}