use super::component::{Component, ComponentManager, TypeNotFoundError};
use crate::snowflake::Snowflake;
use crate::util::Result;

use std::any;
use std::any::TypeId;
use std::collections::HashSet;
use std::fmt;
use std::result;

use failure::{Error, Fail};

pub trait Entity: Sized + 'static {
    fn id(&self) -> Snowflake;
    fn component_manager(&self) -> &ComponentManager<Self>;
    fn components_attached(&self) -> &HashSet<TypeId>;
    fn components_attached_mut(&mut self) -> &mut HashSet<TypeId>;
    //fn component_cache(&self) -> HashMap<TypeId, Box<Component<Self>>>;

    fn get_component<T: Component<Self> + 'static>(&self) -> Result<Option<T>> {
        if !self.components_attached().contains(&TypeId::of::<T>()) {
            if !self.component_manager().is_registered::<T>() {
                Err(TypeNotFoundError::new(any::type_name::<T>().to_owned()).into())
            } else {
                Ok(None)
            }
        } else {
            self.component_manager().get_component::<T>(&self)
        }
    }

    fn set_component<T: Component<Self> + 'static>(&mut self, component: T) -> Result<()> {
        self.component_manager()
            .set_component::<T>(&self, component)
            .map(|_v| {
                self.components_attached_mut().insert(TypeId::of::<T>());
            })
    }

    fn has_component<T: Component<Self> + 'static>(&self) -> bool {
        self.components_attached().contains(&TypeId::of::<T>())
    }

    fn delete_component<T: Component<Self> + 'static>(&mut self) -> Result<()> {
        self.components_attached_mut().remove(&TypeId::of::<T>());
        self.component_manager().delete_component::<T>(&self)
    }

    fn clear_components(&mut self) -> result::Result<(), ClearComponentsError> {
        let mut err = ClearComponentsError::new();
        for type_id in self.components_attached().iter() {
            if let Err(e) = self
                .component_manager()
                .delete_component_by_id(&self, type_id)
            {
                err.push(e);
            }
        }

        self.components_attached_mut().clear();

        if err.len() > 0 {
            Err(err)
        } else {
            Ok(())
        }
    }
}

#[derive(Fail, Debug)]
pub struct ClearComponentsError {
    errors: Vec<Error>,
}

impl fmt::Display for ClearComponentsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "failed to clear components due to errors:\n")?;
        for err in self.errors.iter() {
            err.fmt(f)?;
        }

        Ok(())
    }
}

impl ClearComponentsError {
    pub fn new() -> ClearComponentsError {
        ClearComponentsError { errors: Vec::new() }
    }

    pub fn push(&mut self, err: Error) {
        self.errors.push(err);
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }
}
