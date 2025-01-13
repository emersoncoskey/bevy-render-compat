use bevy_app::{App, Plugin};
use bevy_asset::AssetPath;
use bevy_ecs::{
    query::{QueryData, ROQueryItem},
    system::Resource,
    world::{FromWorld, World},
};
use bevy_render::{
    render_resource::{
        BindGroupLayout, CachedComputePipelineId, CachedRenderPipelineId,
        ComputePipelineDescriptor, PipelineCache, RenderPipelineDescriptor,
    },
    renderer::RenderDevice,
};
use bevy_utils::hashbrown::HashMap;
use std::{hash::Hash, iter};

use crate::material::{BaseMaterial, MaterialLayout};

pub trait SpecializedRenderPipeline: FromWorld + GetKey<RenderPipelineDescriptor> {}
impl<T: FromWorld + GetKey<RenderPipelineDescriptor>> SpecializedRenderPipeline for T {}

pub trait SpecializedComputePipeline: FromWorld + GetKey<ComputePipelineDescriptor> {}
impl<T: FromWorld + GetKey<ComputePipelineDescriptor>> SpecializedComputePipeline for T {}

pub trait DefaultVertex {
    fn default_vertex() -> AssetPath<'static>;
}

pub trait DefaultFragment {
    fn default_fragment() -> AssetPath<'static>;
}

pub trait DefaultCompute {
    fn default_compute() -> AssetPath<'static>;
}

pub trait Specialize<T>: Send + Sync + 'static {
    type Key: Clone + Hash + Eq;
    fn specialize(&self, key: Self::Key, item: &mut T);
}

pub trait GetKey<T>: Specialize<T> {
    type Data: QueryData;
    fn get_key(data: ROQueryItem<Self::Data>, world: &World) -> Self::Key;

    fn compute_key_plugin(&self) -> impl Plugin;
}

#[derive(Resource)]
pub struct SpecializedRenderPipelines<S: Specialize<RenderPipelineDescriptor>> {
    specializer: S,
    user_specializer: Option<fn(S::Key, &mut RenderPipelineDescriptor)>,
    base_descriptor: RenderPipelineDescriptor,
    pipelines: HashMap<S::Key, CachedRenderPipelineId>,
}

impl<S: Specialize<RenderPipelineDescriptor>> SpecializedRenderPipelines<S> {
    pub fn new(
        specializer: S,
        user_specializer: Option<fn(S::Key, &mut RenderPipelineDescriptor)>,
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
        &mut self,
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
pub struct SpecializedComputePipelines<S: Specialize<ComputePipelineDescriptor>> {
    specializer: S,
    user_specializer: Option<fn(S::Key, &mut ComputePipelineDescriptor)>,
    base_descriptor: ComputePipelineDescriptor,
    pipelines: HashMap<S::Key, CachedComputePipelineId>,
}

impl<S: Specialize<ComputePipelineDescriptor>> SpecializedComputePipelines<S> {
    pub fn new(
        specializer: S,
        user_specializer: Option<fn(S::Key, &mut ComputePipelineDescriptor)>,
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
        &mut self,
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

#[derive(Resource, Clone)]
pub struct DummyBindGroupLayout(BindGroupLayout);

impl FromWorld for DummyBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let layout = render_device.create_bind_group_layout("dummy_bind_group_layout", &[]);
        Self(layout)
    }
}

pub struct SpecializeMaterial<M: BaseMaterial, const I: usize> {
    dummy_layout: DummyBindGroupLayout,
    material_layout: MaterialLayout<M>,
}

impl<M: BaseMaterial, const I: usize> FromWorld for SpecializeMaterial<M, I> {
    fn from_world(world: &mut World) -> Self {
        let dummy_layout = world.resource::<DummyBindGroupLayout>().clone();
        let layout = world.resource::<MaterialLayout<M>>().clone();
        Self {
            dummy_layout,
            material_layout: layout,
        }
    }
}

impl<M: BaseMaterial, const I: usize> Specialize<RenderPipelineDescriptor>
    for SpecializeMaterial<M, I>
{
    type Key = ();

    fn specialize(&self, (): Self::Key, item: &mut RenderPipelineDescriptor) {
        //TODO: fill previous layouts with a dummy value if not long enough

        let len = item.layout.len();
        if len <= I {
            item.layout.insert(I, self.material_layout.layout.clone());
        } else {
            item.layout
                .extend(iter::repeat_n(self.dummy_layout.0.clone(), I - len));
            item.layout.push(self.material_layout.layout.clone());
        }
    }
}

impl<M: BaseMaterial, const I: usize> GetKey<RenderPipelineDescriptor>
    for SpecializeMaterial<M, I>
{
    type Data = ();

    fn get_key((): ROQueryItem<Self::Data>, _world: &World) -> Self::Key {}

    fn compute_key_plugin(&self) -> impl Plugin {
        |_app: &mut App| {}
    }
}

impl<M: BaseMaterial, const I: usize> Specialize<ComputePipelineDescriptor>
    for SpecializeMaterial<M, I>
{
    type Key = ();

    fn specialize(&self, (): Self::Key, item: &mut ComputePipelineDescriptor) {
        //TODO: fill previous layouts with a dummy value if not long enough
        if item.layout.len() <= I {
            item.layout.insert(I, self.0.layout.clone());
        } else {
            item.layout.push(self.0.layout.clone());
        }
    }
}

impl<M: BaseMaterial, const I: usize> GetKey<ComputePipelineDescriptor>
    for SpecializeMaterial<M, I>
{
    type Data = ();

    fn get_key((): ROQueryItem<Self::Data>, _world: &World) -> Self::Key {}

    fn compute_key_plugin(&self) -> impl Plugin {
        |_app: &mut App| {}
    }
}
