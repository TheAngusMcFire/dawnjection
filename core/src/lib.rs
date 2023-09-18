use std::any::{Any, TypeId};
use std::clone::Clone;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};


struct ServiceFactory<T> {
    pub factory: fn(&ServiceProvider) -> Option<T>,
}


struct CloneServiceFactory<T> {
    pub factory: fn(&CloneServiceFactory<T>) -> T,
    pub obj: T
}


enum ServiceDescriptor {
    // get references to only this object instance, but just read only
    Singleton(Box<dyn Any + Sync + Send>),
    // get access to one single object instance but also writeable
    MutableSingleton(Box<dyn Any + Sync + Send>),
    // get clones from object instance
    Clone(Box<dyn Any + Sync + Send>),
    // factory to create specific object instances
    Factory(Box<dyn Any + Sync + Send>),
}


#[derive(Default)]
pub struct ServiceCollection {
    map: HashMap<std::any::TypeId, ServiceDescriptor>,
}

impl ServiceCollection {

    fn check_if_already_registered<T: 'static>(&self) {
        if self.map.contains_key(&TypeId::of::<T>()) { panic!() }
    }
    pub fn reg_cloneable<T: Sync + Send>(&mut self, instance: T) where T: Clone + 'static {
        self.check_if_already_registered::<T>();

        let csf = CloneServiceFactory {
            obj: instance,
            factory: |x| {
                x.obj.clone()
            }
        };

        self.map.insert(std::any::TypeId::of::<T>(), ServiceDescriptor::Clone(Box::new(csf)));
    }

    pub fn reg_singleton<T:  Sync + Send>(&mut self, instance: T) where T: 'static {
        self.check_if_already_registered::<T>();
        self.map.insert(std::any::TypeId::of::<T>(), ServiceDescriptor::Singleton(Box::new(instance)));
    }

    pub fn reg_mutable_singleton<T:  Sync + Send>(&mut self, instance: T) where T: 'static {
        self.check_if_already_registered::<T>();
        self.map.insert(std::any::TypeId::of::<T>(), ServiceDescriptor::MutableSingleton(Box::new(Arc::new(Mutex::new(instance)))));
    }

    pub fn reg_factory<T: 'static + Sync + Send>(&mut self, factory: fn(&ServiceProvider) -> Option<T>) {
        self.check_if_already_registered::<T>();
        self.map.insert(std::any::TypeId::of::<T>(), ServiceDescriptor::Factory(Box::new(ServiceFactory { factory })));
    }

    pub fn build_service_provider(self) -> ServiceProvider {
        ServiceProvider {
            map: self.map
        }
    }
}


pub trait IServiceProvider {
    fn try_get<T: 'static>(&self) -> Option<T>;
    fn try_get_ref<T: 'static>(&self) -> Option<&T>;
    fn try_get_mut<T: 'static>(&self) -> Option<MutexGuard<T>>;
}

pub struct ServiceProvider {
    map: HashMap<std::any::TypeId, ServiceDescriptor>,
}

impl IServiceProvider for ServiceProvider {
    fn try_get<T: 'static>(&self) -> Option<T> {
        match self.map.get(&TypeId::of::<T>()) {
            Some(ServiceDescriptor::Factory(x)) => 
            x.downcast_ref::<ServiceFactory<T>>()
                .map(|fun| (fun.factory)(self)).flatten(),
            Some(ServiceDescriptor::Clone(x)) => 
            x.downcast_ref::<CloneServiceFactory<T>>()
                .map(|fun| (fun.factory)(fun)),
            _ => None
        }
    }

    fn try_get_ref<T: 'static>(&self) -> Option<&T> {
        match self.map.get(&TypeId::of::<T>()) {
            Some(ServiceDescriptor::Clone(x)) => x.downcast_ref::<T>(),
            Some(ServiceDescriptor::Singleton(x)) => x.downcast_ref::<T>(),
            _ => None
        }
    }

    fn try_get_mut<T: 'static>(&self) -> Option<MutexGuard<T>> {
        match self.map.get(&TypeId::of::<T>()) {
            Some(ServiceDescriptor::MutableSingleton(x)) => 
            if let Some(x) = x.downcast_ref::<Arc<Mutex<T>>>() {
                // not sure what the correct handling of this is
                match x.lock() {
                    Ok(x) => Some(x),
                    _ => None
                }
            } else { None }
            _ => None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{IServiceProvider, ServiceCollection};

    #[test]
    fn basic_clone() {
        let mut collection = ServiceCollection::default();
        collection.reg_cloneable(42_i32);
        let pro = collection.build_service_provider();
        assert_eq!(pro.try_get::<i32>(), Some(42));
    }

    #[test]
    fn basic_factory() {
        let mut collection = ServiceCollection::default();
        collection.reg_mutable_singleton(42_u32);
        collection.reg_factory(|x| {
            let int = x.try_get_mut::<u32>().unwrap();
            (*int) as i32
        });
        let pro = collection.build_service_provider();
        assert_eq!(pro.try_get::<i32>(), Some(42));
    }

    #[test]
    fn basic_singleton() {
        let mut collection = ServiceCollection::default();
        collection.reg_singleton(42_i32);
        let pro = collection.build_service_provider();
        assert_eq!(pro.try_get_ref::<i32>(), Some(&42))
    }

    #[test]
    fn basic_mutable_singleton() {
        let mut collection = ServiceCollection::default();
        collection.reg_mutable_singleton(42_i32);
        let pro = collection.build_service_provider();
        {
            let mut some = pro.try_get_mut::<i32>().unwrap();
            *some += 1;
        }

        let val = pro.try_get_mut::<i32>().unwrap();
        assert_eq!(*val, 43)
    }

    #[test]
    #[should_panic]
    fn double_reg_test() {
        let mut collection = ServiceCollection::default();
        collection.reg_mutable_singleton(42_i32);
        collection.reg_mutable_singleton(42_i32);
    }
}