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
    // one shot consumable, e.g. some scoped context object
    Take(Box<dyn Any + Sync + Send>),
}


#[derive(Default)]
pub struct ServiceCollection {
    map: HashMap<std::any::TypeId, ServiceDescriptor>,
}

impl ServiceCollection {

    fn check_if_already_registered<T: 'static>(&self) {
        if self.map.contains_key(&TypeId::of::<T>()) { panic!() }
    }
    pub fn reg_cloneable<T: Sync + Send>(mut self, instance: T) -> Self where T: Clone + 'static {
        self.check_if_already_registered::<T>();

        let csf = CloneServiceFactory {
            obj: instance,
            factory: |x| {
                x.obj.clone()
            }
        };

        self.map.insert(std::any::TypeId::of::<T>(), ServiceDescriptor::Clone(Box::new(csf)));
        self
    }

    pub fn reg_singleton<T:  Sync + Send>(mut self, instance: T) -> Self where T: 'static {
        self.check_if_already_registered::<T>();
        self.map.insert(std::any::TypeId::of::<T>(), ServiceDescriptor::Singleton(Box::new(instance)));
        self
    }

    pub fn reg_mutable_singleton<T:  Sync + Send>(mut self, instance: T) -> Self where T: 'static {
        self.check_if_already_registered::<T>();
        self.map.insert(std::any::TypeId::of::<T>(), ServiceDescriptor::MutableSingleton(Box::new(Arc::new(Mutex::new(instance)))));
        self
    }

    pub fn reg_factory<T: 'static + Sync + Send>(mut self, factory: fn(&ServiceProvider) -> Option<T>) -> Self {
        self.check_if_already_registered::<T>();
        self.map.insert(std::any::TypeId::of::<T>(), ServiceDescriptor::Factory(Box::new(ServiceFactory { factory })));
        self
    }

    pub fn reg_takeable<T:  Sync + Send>(mut self, instance: T) -> Self where T: 'static {
        self.check_if_already_registered::<T>();
        self.map.insert(std::any::TypeId::of::<T>(), ServiceDescriptor::Take(Box::new(instance)));
        self
    }

    pub fn build_service_provider(self) -> ServiceProvider {
        ServiceProvider {
            map: Arc::new(self.map),
            /* root provider does not have a scope */
            scope_context: None
        }
    }

    pub fn build_service_provider_arc(self) -> Arc<ServiceProvider> {
        Arc::new(self.build_service_provider())
    }


    pub (crate) fn get_service_map(self) -> HashMap<std::any::TypeId, ServiceDescriptor> {
        self.map
    }

}


pub trait IServiceProvider {
    /* one shot function to move entry from di scope */
    fn try_take<T: 'static>(&self) -> Option<T>;
    fn try_get<T: 'static>(&self) -> Option<T>;
    fn try_get_ref<T: 'static>(&self) -> Option<&T>;
    fn try_get_mut<T: 'static>(&self) -> Option<MutexGuard<T>>;
}

pub struct ServiceProvider {
    map: Arc<HashMap<std::any::TypeId, ServiceDescriptor>>,
    scope_context: Option<Arc<Mutex<HashMap<std::any::TypeId, ServiceDescriptor>>>>
}

impl ServiceProvider {
    pub fn create_scope(&self, scope_seed: Option<ServiceCollection>) -> Self {
        let scope_ctx = match scope_seed {
            Some(x) => x.get_service_map(),
            None => HashMap::new()
        };

        ServiceProvider {
            map: self.map.clone(),
            scope_context: Some(Arc::new(Mutex::new(scope_ctx)))
        }
    }

    pub fn create_scope_arc(&self, scope_seed: Option<ServiceCollection>) -> Arc<Self> {
        self.create_scope(scope_seed).into()
    }
}

impl IServiceProvider for ServiceProvider {

    fn try_take<T: 'static>(&self) -> Option<T> {

        self.scope_context.as_ref()?;

        let mut scope_map = self.scope_context.as_ref().unwrap().lock().unwrap();
        
        match scope_map.get(&TypeId::of::<T>()) {
            Some(ServiceDescriptor::Take(_)) => {},
            _ => return None
        }
        
        match scope_map.remove(&TypeId::of::<T>()) {
            Some(ServiceDescriptor::Take(taken)) => {
                match taken.downcast::<T>() {
                    Ok(x) => Some(*x),
                    _ => None
                }
            }
            _ => None
        }
    }

    fn try_get<T: 'static>(&self) -> Option<T> {
        let def = match self.map.get(&TypeId::of::<T>()) {
            Some(ServiceDescriptor::Factory(x)) => 
            x.downcast_ref::<ServiceFactory<T>>()
                .and_then(|fun| (fun.factory)(self)),
            Some(ServiceDescriptor::Clone(x)) => 
            x.downcast_ref::<CloneServiceFactory<T>>()
                .map(|fun| (fun.factory)(fun)),
            _ => None
        };

        if def.is_some() {
            return def;
        }

        self.scope_context.as_ref()?;

        if let Some(x) = self.try_take() {
            return Some(x);
        }

        {
            let scope_map = self.scope_context.as_ref().unwrap().lock().unwrap();
            match scope_map.get(&TypeId::of::<T>()) {
                Some(ServiceDescriptor::Factory(x)) => 
                x.downcast_ref::<ServiceFactory<T>>()
                    .and_then(|fun| (fun.factory)(self)),
                Some(ServiceDescriptor::Clone(x)) => 
                x.downcast_ref::<CloneServiceFactory<T>>()
                    .map(|fun| (fun.factory)(fun)),
                _ => None
            }
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
        let collection = ServiceCollection::default();
        let pro = collection
            .reg_cloneable(42_i32)
            .build_service_provider();
        assert_eq!(pro.try_get::<i32>(), Some(42));
    }

    #[test]
    fn basic_factory() {
        let pro = ServiceCollection::default()
            .reg_mutable_singleton(42_u32)
            .reg_factory(|x| {
            let int = x.try_get_mut::<u32>().unwrap();
            Some((*int) as i32)
        }).build_service_provider();
        assert_eq!(pro.try_get::<i32>(), Some(42));
    }

    #[test]
    fn basic_singleton() {
        let pro = ServiceCollection::default()
            .reg_singleton(42_i32
            ).build_service_provider();
        assert_eq!(pro.try_get_ref::<i32>(), Some(&42))
    }

    #[test]
    fn basic_mutable_singleton() {
        let pro = ServiceCollection::default()
            .reg_mutable_singleton(42_i32)
            .build_service_provider();

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
        let _ = ServiceCollection::default()
            .reg_mutable_singleton(42_i32)
            .reg_mutable_singleton(42_i32);
    }

    #[test]
    fn scope_test() {
        let test_string = "some string takeable";
        let root_sc = ServiceCollection::default()
            .reg_mutable_singleton(42_i32)
            .build_service_provider();
        let scope_collection = ServiceCollection::default()
            .reg_takeable(test_string.to_string())
            .reg_takeable(1234u64);
        let scope_sp = root_sc.create_scope(Some(scope_collection));
        let some_string = scope_sp.try_get::<String>();
        assert_eq!(some_string, Some(test_string.to_string()));
        assert_eq!(scope_sp.try_take(), Some(1234u64));
    }
}