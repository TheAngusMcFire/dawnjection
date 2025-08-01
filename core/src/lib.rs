use std::any::{Any, TypeId};
use std::clone::Clone;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, MutexGuard};

#[cfg(feature = "axum")]
pub mod axum;

#[cfg(feature = "rocket")]
pub mod rocket;

#[cfg(feature = "dawnflow")]
pub mod dawnflow;

pub use dawnjection_codegen::*;

pub trait FromDi {
    fn from_di(sp: &ServiceProvider) -> Option<Self>
    where
        Self: Sized;
}

pub struct I<T>(pub T);

impl<T> I<T> {
    pub fn get(self) -> T {
        self.0
    }
}

#[derive(Clone)]
pub struct ServiceProviderContainer(pub Arc<ServiceProvider>);

pub trait ServiceProviderAccess {
    fn get_sp_arc(&self) -> &Arc<ServiceProvider>;
}

impl ServiceProviderAccess for ServiceProviderContainer {
    fn get_sp_arc(&self) -> &Arc<ServiceProvider> {
        &self.0
    }
}

pub struct R<T: 'static> {
    provider: ServiceProviderContainer,
    phantom: PhantomData<T>,
}

impl<T> R<T> {
    pub fn new(provider: ServiceProviderContainer) -> Self {
        Self {
            provider,
            phantom: PhantomData,
        }
    }

    pub fn get(&self) -> &T {
        self.provider.0.try_get_ref::<T>().unwrap_or_else(|| {
            panic!(
                "Expected registered type in Dependency Injection: {}",
                std::any::type_name::<T>()
            )
        })
    }

    pub fn try_get(&self) -> Option<&T> {
        self.provider.0.try_get_ref()
    }
}

pub type Report = eyre::Report;
pub type BoxFuture<'r, T = Result<(), Report>> = futures::future::BoxFuture<'r, T>;
#[allow(dead_code)]
pub struct HandlerEntry {
    pub handler: fn(sp: Arc<ServiceProvider>) -> BoxFuture<'static>,
    pub name: String,
}

struct ServiceFactory<T> {
    pub factory: fn(&ServiceProvider) -> Option<T>,
}

struct CloneServiceFactory<T> {
    pub factory: fn(&CloneServiceFactory<T>) -> T,
    pub obj: T,
}

pub enum ServiceDescriptor {
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

impl std::fmt::Debug for ServiceDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ServiceDescriptor::Singleton(_) => "singleton",
                ServiceDescriptor::MutableSingleton(_) => "mutable-singleton",
                ServiceDescriptor::Clone(_) => "clone",
                ServiceDescriptor::Factory(_) => "factory",
                ServiceDescriptor::Take(_) => "take",
            }
        )
    }
}

#[derive(Default)]
pub struct ServiceCollection {
    map: HashMap<std::any::TypeId, ServiceDescriptor>,
}

impl ServiceCollection {
    fn check_if_already_registered<T: 'static>(&self) {
        if self.map.contains_key(&TypeId::of::<T>()) {
            panic!()
        }
    }
    pub fn reg_cloneable<T>(mut self, instance: T) -> Self
    where
        T: Clone + 'static,
        T: Sync + Send,
    {
        self.check_if_already_registered::<T>();

        let csf = CloneServiceFactory {
            obj: instance,
            factory: |x| x.obj.clone(),
        };

        self.map.insert(
            std::any::TypeId::of::<T>(),
            ServiceDescriptor::Clone(Box::new(csf)),
        );
        self
    }

    pub fn reg_singleton<T>(mut self, instance: T) -> Self
    where
        T: 'static,
        T: Sync + Send,
    {
        self.check_if_already_registered::<T>();
        self.map.insert(
            std::any::TypeId::of::<T>(),
            ServiceDescriptor::Singleton(Box::new(instance)),
        );
        self
    }

    pub fn reg_mutable_singleton<T>(mut self, instance: T) -> Self
    where
        T: 'static,
        T: Sync + Send,
    {
        self.check_if_already_registered::<T>();
        self.map.insert(
            std::any::TypeId::of::<T>(),
            ServiceDescriptor::MutableSingleton(Box::new(Arc::new(Mutex::new(instance)))),
        );
        self
    }

    pub fn reg_factory<T>(mut self, factory: fn(&ServiceProvider) -> Option<T>) -> Self
    where
        T: 'static + Sync + Send,
    {
        self.check_if_already_registered::<T>();
        self.map.insert(
            std::any::TypeId::of::<T>(),
            ServiceDescriptor::Factory(Box::new(ServiceFactory { factory })),
        );
        self
    }

    pub fn reg_takeable<T>(mut self, instance: T) -> Self
    where
        T: 'static,
        T: Sync + Send,
    {
        self.check_if_already_registered::<T>();
        self.map.insert(
            std::any::TypeId::of::<T>(),
            ServiceDescriptor::Take(Box::new(instance)),
        );
        self
    }

    pub fn register_takeable<T>(mut self, instance: T) -> Self
    where
        T: 'static,
        T: Sync + Send,
    {
        self.check_if_already_registered::<T>();
        self.map.insert(
            std::any::TypeId::of::<T>(),
            ServiceDescriptor::Take(Box::new(instance)),
        );
        self
    }

    pub fn register_service_descriptor(mut self, id: TypeId, sd: ServiceDescriptor) -> Self {
        self.map.insert(id, sd);
        self
    }

    pub fn build_service_provider(self) -> ServiceProvider {
        ServiceProvider {
            map: Arc::new(self.map),
            /* root provider does not have a scope */
            scope_context_mut: None,
            scope_context: None,
        }
    }

    /// build service provider which only contains a scope, no body
    pub fn build_scoped_service_provider(self) -> ServiceProvider {
        let mut map_mut: HashMap<std::any::TypeId, ServiceDescriptor> = Default::default();
        let mut map: HashMap<std::any::TypeId, ServiceDescriptor> = Default::default();

        for (ty, sd) in self.map {
            if matches!(sd, ServiceDescriptor::Take(_)) {
                map_mut.insert(ty, sd);
            } else {
                map.insert(ty, sd);
            }
        }

        ServiceProvider {
            map: Default::default(),
            scope_context_mut: Some(Arc::new(Mutex::new(map_mut))),
            scope_context: Some(Arc::new(map)),
        }
    }

    pub fn build_service_provider_arc(self) -> Arc<ServiceProvider> {
        Arc::new(self.build_service_provider())
    }

    pub(crate) fn get_service_map(self) -> HashMap<std::any::TypeId, ServiceDescriptor> {
        self.map
    }
}

pub trait IServiceProvider {
    /// one shot function to move entry from di scope
    fn try_take<T: 'static>(&self) -> Option<T>;
    fn try_get<T: 'static>(&self) -> Option<T>;
    fn try_get_ref<T: 'static>(&self) -> Option<&T>;
    fn try_get_mut<T: 'static>(&self) -> Option<MutexGuard<T>>;
}

#[derive(Clone)]
pub struct ServiceProvider {
    map: Arc<HashMap<std::any::TypeId, ServiceDescriptor>>,
    scope_context_mut: Option<Arc<Mutex<HashMap<std::any::TypeId, ServiceDescriptor>>>>,
    scope_context: Option<Arc<HashMap<std::any::TypeId, ServiceDescriptor>>>,
}

impl std::fmt::Debug for ServiceProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "\nroot:")?;
        for m in self.map.iter() {
            writeln!(f, "    TypeId: {:?} Desc: {:?}", &m.0, m.1)?;
        }

        writeln!(f, "scope:")?;

        if let Some(sc) = &self.scope_context_mut {
            let lock = sc.lock().unwrap();
            for m in lock.iter() {
                writeln!(f, "    TypeId: {:?} Desc: {:?}", &m.0, m.1)?;
            }
        }

        Ok(())
    }
}

impl ServiceProvider {
    pub fn destroy(
        self,
    ) -> (
        Arc<HashMap<std::any::TypeId, ServiceDescriptor>>,
        Option<Arc<Mutex<HashMap<std::any::TypeId, ServiceDescriptor>>>>,
        Option<Arc<HashMap<std::any::TypeId, ServiceDescriptor>>>,
    ) {
        (self.map, self.scope_context_mut, self.scope_context)
    }

    pub fn create_scope(&self, scope_seed: Option<ServiceCollection>) -> Self {
        if let Some(service_map) = scope_seed.map(|x| x.get_service_map()) {
            let mut map_mut: HashMap<std::any::TypeId, ServiceDescriptor> = Default::default();
            let mut map: HashMap<std::any::TypeId, ServiceDescriptor> = Default::default();

            for (ty, sd) in service_map {
                if matches!(sd, ServiceDescriptor::Take(_)) {
                    map_mut.insert(ty, sd);
                } else {
                    map.insert(ty, sd);
                }
            }
            ServiceProvider {
                map: self.map.clone(),
                scope_context_mut: Some(Arc::new(Mutex::new(map_mut))),
                scope_context: Some(Arc::new(map)),
            }
        } else {
            ServiceProvider {
                map: self.map.clone(),
                scope_context_mut: None,
                scope_context: None,
            }
        }
    }

    pub fn create_scope_arc(&self, scope_seed: Option<ServiceCollection>) -> Arc<Self> {
        self.create_scope(scope_seed).into()
    }

    pub fn try_take<T: 'static>(&self) -> Option<T> {
        self.scope_context_mut.as_ref()?;
        let type_id = TypeId::of::<T>();

        let mut scope_map = self.scope_context_mut.as_ref().unwrap().lock().unwrap();

        match scope_map.remove(&type_id) {
            Some(ServiceDescriptor::Take(taken)) => match taken.downcast::<T>() {
                Ok(x) => Some(*x),
                _ => None,
            },
            Some(x) => {
                scope_map.insert(type_id, x);
                None
            }
            _ => None,
        }
    }

    pub fn try_get<T: 'static>(&self) -> Option<T> {
        let req_type = &TypeId::of::<T>();
        let def = match self.map.get(req_type) {
            Some(ServiceDescriptor::Factory(x)) => x
                .downcast_ref::<ServiceFactory<T>>()
                .and_then(|fun| (fun.factory)(self)),
            Some(ServiceDescriptor::Clone(x)) => x
                .downcast_ref::<CloneServiceFactory<T>>()
                .map(|fun| (fun.factory)(fun)),
            _ => None,
        };

        if def.is_some() {
            return def;
        }

        if let Some(scope_map) = &self.scope_context {
            let svc = match scope_map.get(req_type) {
                Some(ServiceDescriptor::Factory(x)) => x
                    .downcast_ref::<ServiceFactory<T>>()
                    .and_then(|fun| (fun.factory)(self)),
                Some(ServiceDescriptor::Clone(x)) => x
                    .downcast_ref::<CloneServiceFactory<T>>()
                    .map(|fun| (fun.factory)(fun)),
                _ => None,
            };

            if svc.is_some() {
                return svc;
            }
        }

        self.try_take()
    }

    pub fn try_get_ref<T: 'static>(&self) -> Option<&T> {
        match self.map.get(&TypeId::of::<T>()) {
            Some(ServiceDescriptor::Clone(x)) => x.downcast_ref::<T>(),
            Some(ServiceDescriptor::Singleton(x)) => x.downcast_ref::<T>(),
            _ => None,
        }
    }

    pub fn try_get_mut<T: 'static>(&self) -> Option<MutexGuard<T>> {
        match self.map.get(&TypeId::of::<T>()) {
            Some(ServiceDescriptor::MutableSingleton(x)) => {
                if let Some(x) = x.downcast_ref::<Arc<Mutex<T>>>() {
                    // not sure what the correct handling of this is
                    match x.lock() {
                        Ok(x) => Some(x),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn take<T: 'static>(&self) -> eyre::Result<T> {
        let type_name = std::any::type_name::<T>();
        match self.try_take() {
            Some(x) => Ok(x),
            None => eyre::bail!(
                "cannot take type \"{}\" out of sevice provider because it could not be found",
                type_name
            ),
        }
    }

    pub fn get<T: 'static>(&self) -> eyre::Result<T> {
        let type_name = std::any::type_name::<T>();
        match self.try_get() {
            Some(x) => Ok(x),
            None => eyre::bail!(
                "cannot get type \"{}\" out of sevice provider because it could not be found",
                type_name
            ),
        }
    }

    pub fn get_ref<T: 'static>(&self) -> eyre::Result<&T> {
        let type_name = std::any::type_name::<T>();
        match self.try_get_ref() {
            Some(x) => Ok(x),
            None => eyre::bail!(
                "cannot get_ref type \"{}\" out of sevice provider because it could not be found",
                type_name
            ),
        }
    }

    pub fn get_mut<T: 'static>(&self) -> eyre::Result<MutexGuard<T>> {
        let type_name = std::any::type_name::<T>();
        match self.try_get_mut() {
            Some(x) => Ok(x),
            None => eyre::bail!(
                "cannot get_mut type \"{}\" out of sevice provider because it could not be found",
                type_name
            ),
        }
    }
}

// this is legacy
impl IServiceProvider for ServiceProvider {
    fn try_take<T: 'static>(&self) -> Option<T> {
        self.try_take()
    }

    fn try_get<T: 'static>(&self) -> Option<T> {
        self.try_get()
    }

    fn try_get_ref<T: 'static>(&self) -> Option<&T> {
        self.try_get_ref()
    }

    fn try_get_mut<T: 'static>(&self) -> Option<MutexGuard<T>> {
        self.try_get_mut()
    }
}

#[cfg(test)]
mod tests {
    use crate::ServiceCollection;

    #[test]
    fn basic_clone() {
        let collection = ServiceCollection::default();
        let pro = collection.reg_cloneable(42_i32).build_service_provider();
        assert_eq!(pro.try_get::<i32>(), Some(42));
    }

    #[test]
    fn basic_factory() {
        let pro = ServiceCollection::default()
            .reg_mutable_singleton(42_u32)
            .reg_factory(|x| {
                let int = x.try_get_mut::<u32>().unwrap();
                Some((*int) as i32)
            })
            .build_service_provider();
        assert_eq!(pro.try_get::<i32>(), Some(42));
    }

    #[test]
    fn basic_singleton() {
        let pro = ServiceCollection::default()
            .reg_singleton(42_i32)
            .build_service_provider();
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
    fn scope_clone_test() {
        let sp = ServiceCollection::default()
            .reg_cloneable(24)
            .build_service_provider();
        let ss = sp.create_scope(Some(
            ServiceCollection::default().reg_cloneable("test".to_string()),
        ));
        ss.try_get::<String>().unwrap();
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

    #[test]
    fn scoped_factory_test() {
        let sc = ServiceCollection::default()
            .reg_factory(|x| Some(x.try_get::<u32>().unwrap() as u64))
            .reg_cloneable(0u32);
        sc.build_scoped_service_provider().try_get::<u64>().unwrap();
    }

    #[test]
    fn scoped_take_in_factory_test() {
        let sc = ServiceCollection::default()
            .reg_factory(|x| Some(x.try_get::<u32>().unwrap() as u64))
            .reg_takeable(0u32);
        sc.build_scoped_service_provider().try_get::<u64>().unwrap();
    }
}
