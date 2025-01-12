use bevy_app::Plugin;
use bevy_ecs::{
    query::{QueryData, ROQueryItem},
    world::{FromWorld, World},
};
use bevy_render::render_resource::{
    CachedComputePipelineId, CachedRenderPipelineId, ComputePipelineDescriptor, PipelineCache,
    RenderPipelineDescriptor,
};
use bevy_utils::hashbrown::HashMap;
use std::hash::Hash;
use variadics_please::all_tuples;

pub type RenderPipelineKey<S> = <S as PartialSpecialize<RenderPipelineDescriptor>>::Key;
pub type ComputePipelineKey<S> = <S as PartialSpecialize<ComputePipelineDescriptor>>::Key;

pub trait PartialSpecialize<T>: Send + Sync + 'static {
    type Key: Clone + Hash + Eq;
    fn specialize(&self, key: Self::Key, item: &mut T);
}

impl<K: Clone + Hash + Eq, T, F: Fn(K, &mut T)> PartialSpecialize<T> for F {
    type Key = K;

    fn specialize(&self, key: Self::Key, item: &mut T) {
        (self)(key, item)
    }
}

pub trait Specialize<T>: PartialSpecialize<T> {
    type Data: QueryData;
    fn get_key(data: ROQueryItem<Self::Data>, world: &World) -> Self::Key;

    // a bit jank to duplicate this but it lets us have the tuple impl
    fn from_world(world: &mut World) -> Self;
    fn compute_key_plugin(&self) -> impl Plugin;
}

macro_rules! impl_partial_specialize {
    ($(#[$meta:meta])* $(($S: ident, $s: ident, $k: ident)),*) => {
        $(#[$meta])*
        impl<T, $($S: PartialSpecialize<T>),*> PartialSpecialize<T> for ($($S,)*) {
            type Key = ($($S,)*);

            fn specialize(&self, key: Self::Key, item: &mut T) {
                let ($($s,)*) = self;
                let ($($k,)*) = key;
                $($s.specialize($k, item);)*
            }
        }
    }
}

all_tuples!(
    #[doc(fake_variadic)]
    impl_partial_specialize,
    0,
    15,
    S,
    s,
    k
);

macro_rules! impl_specialize {
    ($(#[$meta:meta])* $(($S: ident, $s: ident, $d: ident)),*) => {
        $(#[$meta])*
        impl<T, $($S: Specialize<T>),*> Specialize<T> for ($($S,)*) {
            type Data = ($($S::Data,)*);

            #[allow(clippy::unused_unit)]
            fn get_key(($($d,)*): ROQueryItem<Self::Data>, world: &World) -> Self::Key {
                ($(<$S as Specialize<T>>::get_key($d, world),)*)
            }

            // a bit jank to duplicate this but it lets us have the tuple impl
            #[allow(clippy::unused_unit)]
            fn from_world(world: &mut World) -> Self {
                ($(<$S as Specialize<T>>::from_world(world),)*)
            }

            fn compute_key_plugin(&self) -> impl Plugin {
                let ($($s,)*) = self;
                |app: &mut App| {
                    app.add_plugins(($(<$S as Specialize<T>>::compute_key_plugin($s),)*));
                }
            }
        }
    }
}

all_tuples!(
    #[doc(fake_variadic)]
    impl_specialize,
    0,
    15,
    S,
    s,
    d
);

#[derive(Resource)]
pub struct SpecializedRenderPipelines<S: PartialSpecialize<RenderPipelineDescriptor>> {
    specializer: S,
    user_specializer: Option<Box<dyn PartialSpecialize<RenderPipelineDescriptor, Key = S::Key>>>,
    base_descriptor: RenderPipelineDescriptor,
    pipelines: HashMap<S::Key, CachedRenderPipelineId>,
}

impl<S: PartialSpecialize<RenderPipelineDescriptor>> SpecializedRenderPipelines<S> {
    pub fn new(
        specializer: S,
        user_specializer: Option<
            Box<dyn PartialSpecialize<RenderPipelineDescriptor, Key = S::Key>>,
        >,
        base_descriptor: RenderPipelineDescriptor,
    ) -> Self {
        Self {
            specializer,
            user_specializer,
            base_descriptor,
            pipelines: Default::default(),
        }
    }

    pub fn specialize(
        &self,
        pipeline_cache: &PipelineCache,
        key: S::Key,
    ) -> CachedRenderPipelineId {
        *self.pipelines.entry(key.clone()).or_insert_with(|| {
            let mut descriptor = self.base_descriptor.clone();
            self.specializer.specialize(key.clone(), &mut descriptor);
            if let Some(user_specializer) = self.user_specializer {
                (user_specializer)(key, &mut descriptor);
            }
            pipeline_cache.queue_render_pipeline(descriptor)
        })
    }
}

#[derive(Resource)]
pub struct SpecializedComputePipelines<S: PartialSpecialize<ComputePipelineDescriptor>> {
    specializer: S,
    user_specializer: Option<Box<dyn PartialSpecialize<ComputePipelineDescriptor, Key = S::Key>>>,
    base_descriptor: ComputePipelineDescriptor,
    pipelines: HashMap<S::Key, CachedComputePipelineId>,
}

impl<S: PartialSpecialize<ComputePipelineDescriptor>> SpecializedComputePipelines<S> {
    pub fn new(
        specializer: S,
        user_specializer: Option<
            Box<dyn PartialSpecialize<ComputePipelineDescriptor, Key = S::Key>>,
        >,
        base_descriptor: ComputePipelineDescriptor,
    ) -> Self {
        Self {
            specializer,
            user_specializer,
            base_descriptor,
            pipelines: Default::default(),
        }
    }

    pub fn specialize(
        &self,
        pipeline_cache: &PipelineCache,
        key: S::Key,
    ) -> CachedComputePipelineId {
        *self.pipelines.entry(key.clone()).or_insert_with(|| {
            let mut descriptor = self.base_descriptor.clone();
            self.specializer.specialize(key.clone(), &mut descriptor);
            if let Some(user_specializer) = self.user_specializer {
                (user_specializer)(key, &mut descriptor);
            }
            pipeline_cache.queue_compute_pipeline(descriptor)
        })
    }
}
