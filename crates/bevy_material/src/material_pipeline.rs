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
        CachedRenderPipelineId, ComputePipelineDescriptor, RawComputePipelineDescriptor,
        RawRenderPipelineDescriptor, RenderPipelineDescriptor,
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

pub struct MaterialRenderPipeline<S: SpecializedRenderPipeline> {
    vertex: AssetPath<'static>,
    fragment: AssetPath<'static>,
    user_specializer: Option<fn(RenderPipelineKey<S::Specializer>, &mut RenderPipelineDescriptor)>,
    _data: PhantomData<S>,
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
            _data: PhantomData,
        }
    }
}

impl<S: SpecializedRenderPipeline> MaterialRenderPipeline<S> {
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

type RenderPipelineKey<T> = <T as Specialize<RenderPipelineDescriptor>>::Key;
type ComputePipelineKey<T> = <T as Specialize<ComputePipelineDescriptor>>::Key;

//TODO: BAD NAME
pub trait SpecializedRenderPipeline {
    type Specializer: Specialize<RenderPipelineDescriptor>;
    type Data: QueryData;

    fn get_key(
        data: ROQueryItem<Self::Data>,
        world: &World,
        last_key: Option<RenderPipelineKey<Self::Specializer>>,
    ) -> RenderPipelineKey<Self::Specializer>;
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

    fn chain<S: Specialize>(self, next: S) -> impl Specialize<T> {
        (self, next)
    }
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

pub struct RenderPipelineSpecializer<S: Specialize<RenderPipelineDescriptor>> {
    specializer: S,
    pipelines: HashMap<S::Key, CachedRenderPipelineId>,
    base: RenderPipelineDescriptor,
}

impl<S: Specialize<RenderPipelineDescriptor>> RenderPipelineSpecializer<S> {
    fn specialize(&self, render_device: &RenderDevice, key: S::Key) -> CachedRenderPipelineId {
        todo!()
    }
}
