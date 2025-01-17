use std::marker::PhantomData;

use bevy_app::Plugin;
use bevy_asset::AssetPath;
use bevy_ecs::system::{ReadOnlySystemParam, SystemParamItem};
use bevy_reflect::TypePath;
use bevy_render::{
    render_resource::{
        CachedRenderPipelineId, ComputePipeline, RenderPipeline, RenderPipelineDescriptor,
        Specialize, SpecializeFn,
    },
    renderer::RenderDevice,
    Render,
};
use bevy_utils::HashMap;
use variadics_please::all_tuples;

use crate::material::Material;

pub trait MaterialPipeline: TypePath + Sized + 'static {
    type Properties: Send + Sync;
    type Pipelines<M: Material<Self>>: Pipelines;

    fn material_plugin<M: Material<Self>>() -> impl Plugin;
}

pub trait Pipelines {
    type CachedIds: Send + Sync;
    type Specializers: Send + Sync;

    //TODO
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

pub trait DefaultVertex: Specialize<RenderPipeline> {
    fn default_vertex() -> AssetPath<'static>;
}

pub trait DefaultFragment: Specialize<RenderPipeline> {
    fn default_fragment() -> AssetPath<'static>;
}

pub trait DefaultCompute: Specialize<ComputePipeline> {
    fn default_compute() -> AssetPath<'static>;
}

pub struct MaterialRenderPipeline<S: Specialize<RenderPipeline>> {
    vertex: AssetPath<'static>,
    fragment: AssetPath<'static>,
    user_specializer: Option<SpecializeFn<RenderPipeline, S>>,
    _data: PhantomData<S>, // wire up the specializer somewhere in here
}

impl<S: Specialize<RenderPipeline> + DefaultVertex + DefaultFragment> Default
    for MaterialRenderPipeline<S>
{
    fn default() -> Self {
        Self {
            vertex: S::default_vertex(),
            fragment: S::default_fragment(),
            user_specializer: None,
            _data: PhantomData,
        }
    }
}

impl<S: Specialize<RenderPipeline>> MaterialRenderPipeline<S> {
    pub fn new(vertex: AssetPath<'static>, fragment: AssetPath<'static>) -> Self {
        Self {
            vertex,
            fragment,
            user_specializer: None,
            _data: PhantomData,
        }
    }

    pub fn with_vertex(self, vertex: AssetPath<'static>) -> Self {
        Self { vertex, ..self }
    }

    pub fn with_fragment(self, fragment: AssetPath<'static>) -> Self {
        Self { fragment, ..self }
    }

    pub fn specialize(self, specialize_fn: SpecializeFn<RenderPipeline, S>) -> Self {
        Self {
            user_specializer: Some(specialize_fn),
            ..self
        }
    }
}

pub struct MaterialComputePipeline<S: Specialize<ComputePipeline>> {
    compute: AssetPath<'static>,
    user_specializer: Option<SpecializeFn<ComputePipeline, S>>,
    _data: PhantomData<S>, // wire up the specializer somewhere in here
}

impl<S: Specialize<ComputePipeline> + DefaultCompute> Default for MaterialComputePipeline<S> {
    fn default() -> Self {
        Self {
            compute: S::default_compute(),
            user_specializer: None,
            _data: PhantomData,
        }
    }
}

impl<S: Specialize<ComputePipeline>> MaterialComputePipeline<S> {
    pub fn new(compute: AssetPath<'static>) -> Self {
        Self {
            compute,
            user_specializer: None,
            _data: PhantomData,
        }
    }

    pub fn specialize(self, specialize_fn: SpecializeFn<ComputePipeline, S>) -> Self {
        Self {
            user_specializer: Some(specialize_fn),
            ..self
        }
    }
}
