use core::marker::PhantomData;

use bevy_asset::{AssetId, Handle};
use bevy_ecs::{component::Component, reflect::ReflectComponent};
use bevy_reflect::Reflect;

use crate::{material::Material, material_pipeline::MaterialPipeline};

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct MaterialComponent<M: Material<P>, P: MaterialPipeline> {
    pub handle: Handle<M>,
    #[reflect(ignore)]
    _data: PhantomData<fn(P)>,
}

impl<M: Material<P>, P: MaterialPipeline> MaterialComponent<M, P> {
    pub fn new(handle: Handle<M>) -> Self {
        Self {
            handle,
            _data: PhantomData,
        }
    }

    #[inline]
    pub fn id(&self) -> AssetId<M> {
        self.handle.id()
    }
}

impl<M: Material<P>, P: MaterialPipeline> From<MaterialComponent<M, P>> for AssetId<M> {
    fn from(value: MaterialComponent<M, P>) -> Self {
        value.id()
    }
}

impl<M: Material<P>, P: MaterialPipeline> From<&MaterialComponent<M, P>> for AssetId<M> {
    fn from(value: &MaterialComponent<M, P>) -> Self {
        value.id()
    }
}