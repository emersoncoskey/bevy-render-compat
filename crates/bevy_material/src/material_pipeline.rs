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

use crate::material::Material;

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

pub struct MaterialRenderPipeline<S: SpecializedRenderPipeline> {
    vertex: AssetPath<'static>,
    fragment: AssetPath<'static>,
    user_specializer: Option<fn(S::Key, &mut RenderPipelineDescriptor)>,
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
        specializer: fn(RenderPipelineKey<S::Specializer>, &mut RenderPipelineDescriptor),
    ) -> Self {
        Self {
            user_specializer: Some(specializer),
            ..self
        }
    }
}

impl<S: SpecializedRenderPipeline> Pipelines for MaterialRenderPipeline<S> {
    type Cached = CachedRenderPipelineId;

    fn into_plugin(self) -> impl Plugin {}
}

type RenderPipelineKey<T> = <T as Specialize<RenderPipelineDescriptor>>::Key;
type ComputePipelineKey<T> = <T as Specialize<ComputePipelineDescriptor>>::Key;

pub trait SpecializedRenderPipeline: Specialize<RenderPipelineDescriptor> {
    type Data: QueryData;

    fn get_key<'w>(data: ROQueryItem<Self::Data>, world: &World) -> Self::Key;

    fn into_plugin(self) -> impl Plugin;
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

pub trait Specialize<T>: FromWorld + Send + Sync + 'static {
    type Key: Clone + Hash + Eq;
    fn specialize(&self, key: Self::Key, item: &mut T);
}

impl<K: Clone + Hash + Eq, T, F: Fn(&self, K, &mut T)> Specialize<T> for F {
    type Key = K;

    fn specialize(&self, key: Self::Key, item: &mut T) {
        (self)(key, item)
    }
}

macro_rules! impl_specialize {
    ($(#[$meta:meta])* $(($S: ident, $s: ident, $k: ident)),*) => {
        $(#[$meta])*
         impl<T, $($S: Specialize<T>),*> Specialize<T> for ($($T,)*) {
            type Key = ($($T,)*);

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
    impl_specialize,
    0,
    15,
    S,
    s,
    k
);

pub struct SpecializedRenderPipelines<S: Specialize<RenderPipelineDescriptor>> {
    specializer: S,
    user_specializer: Option<fn(S::Key, &mut RenderPipelineDescriptor)>,
    pipelines: HashMap<S::Key, CachedRenderPipelineId>,
    base_descriptor: RenderPipelineDescriptor,
}

impl<S: Specialize<RenderPipelineDescriptor>> SpecializedRenderPipelines<S> {
    fn new(
        specializer: S,
        user_specializer: Option<fn(S::Key, &mut RenderPipelineDescriptor)>,
        base_descriptor: RenderPipelineDescriptor,
    ) -> Self {
        Self {
            specializer,
            user_specializer,
            pipelines: Default::default(),
            base_descriptor,
        }
    }

    fn specialize(&self, pipeline_cache: &PipelineCache, key: S::Key) -> CachedRenderPipelineId {
        self.pipelines.entry(key.clone()).or_insert_with(|| {
            let mut descriptor = self.base.clone();
            self.specializer.specialize(key.clone(), &mut descriptor);
            if let Some(user_specializer) = self.user_specializer {
                (user_specializer)(key, &mut descriptor);
            }
            pipeline_cache.queue_render_pipeline(descriptor);
        })
    }
}
