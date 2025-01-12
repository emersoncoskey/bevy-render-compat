use std::marker::PhantomData;

use bevy_app::Plugin;
use bevy_asset::AssetPath;
use bevy_ecs::{
    query::{QueryData, QueryItem, ROQueryItem, ReadOnlyQueryData, WorldQuery},
    system::{ReadOnlySystemParam, SystemParamItem},
    world::{FromWorld, World},
};
use bevy_reflect::TypePath;
use bevy_render::{
    render_resource::{
        CachedRenderPipelineId, ComputePipelineDescriptor, PipelineCache,
        RawComputePipelineDescriptor, RawRenderPipelineDescriptor, RenderPipelineDescriptor,
    },
    renderer::RenderDevice,
    Render,
};
use bevy_utils::HashMap;
use variadics_please::all_tuples;

use crate::{
    material::Material,
    specialize::{PartialSpecialize, RenderPipelineKey, Specialize},
};

pub trait MaterialPipeline: TypePath + Sized + 'static {
    type Properties: Send + Sync;
    type Pipelines<M: Material<Self>>: Pipelines;

    fn material_plugin<M: Material<Self>>() -> impl Plugin;
}

pub trait Pipelines {
    type Cached: Send + Sync;
    type Data: QueryData;

    fn into_plugin(self) -> impl Plugin;
    fn get_cached(data: ROQueryItem<Self::Data>, world: &World) -> Self::Cached;
}

// The goal here is something like:
// ```rust
// impl Material<Mesh3d> for StandardMaterial {
//     fn metadata(&self) -> Mesh3dMetadata {
//         Mesh3dMetadata {
//             ...
//         }
//     }
//
//     fn pipelines() -> Mesh3dPipelines {
//         Mesh3dPipelines {
//             prepass: MaterialRenderPipeline::new(
//                 "my_vertex_path".into(),
//                 "my_fragment_path".into(),
//             ).specialize(my_specialization_fn)
//             deferred: ...
//             main_pass: ...
//         }
//     }
// }
// ```

pub trait SpecializedRenderPipeline {
    type Specializer: Specialize<RenderPipelineDescriptor>;
}

pub trait SpecializedComputePipeline {
    type Specializer: Specialize<ComputePipelineDescriptor>;
}

pub trait DefaultVertex {
    fn default_vertex() -> AssetPath<'static>;
}

pub trait DefaultFragment {
    fn default_fragment() -> AssetPath<'static>;
}

pub trait DefaultCompute {
    fn default_compute() -> AssetPath<'static>;
}

pub struct MaterialRenderPipeline<S: SpecializedRenderPipeline> {
    vertex: AssetPath<'static>,
    fragment: AssetPath<'static>,
    user_specializer: Option<
        Box<
            dyn PartialSpecialize<
                RenderPipelineDescriptor,
                Key = RenderPipelineKey<S::Specializer>,
            >,
        >,
    >,
}

impl<S> Default for MaterialRenderPipeline<S>
where
    S: SpecializedRenderPipeline + DefaultVertex + DefaultFragment,
{
    fn default() -> Self {
        Self {
            vertex: S::default_vertex(),
            fragment: S::default_fragment(),
            user_specializer: None,
        }
    }
}

impl<S: SpecializedRenderPipeline> MaterialRenderPipeline<S> {
    pub fn new(vertex: AssetPath<'static>, fragment: AssetPath<'static>) -> Self {
        Self {
            vertex,
            fragment,
            user_specializer: None,
        }
    }

    pub fn with_vertex(self, vertex: AssetPath<'static>) -> Self {
        Self { vertex, ..self }
    }

    pub fn with_fragment(self, fragment: AssetPath<'static>) -> Self {
        Self { fragment, ..self }
    }

    pub fn specialize(
        self,
        specializer: impl PartialSpecialize<
            RenderPipelineDescriptor,
            Key = RenderPipelineKey<S::Specializer>,
        >,
    ) -> Self {
        Self {
            user_specializer: Some(Box::new(specializer)),
            ..self
        }
    }
}

impl<S: SpecializedRenderPipeline> Pipelines for MaterialRenderPipeline<S> {
    type Cached = CachedRenderPipelineId;

    fn into_plugin(self) -> impl Plugin {
        todo!()
    }

    type Data = ();
    fn get_cached(data: ROQueryItem<Self::Data>, world: &World) -> Self::Cached {}
}
