use bevy_ecs::{
    schedule::IntoSystemConfigs,
    system::{lifetimeless::SRes, SystemParamItem},
    world::{FromWorld, World},
};
use core::marker::PhantomData;

use bevy_app::{App, Plugin};
use bevy_asset::{Asset, AssetApp, AssetId, AssetServer};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::system::{Query, ResMut, Resource};
use bevy_render::{
    render_asset::{PrepareAssetError, RenderAsset, RenderAssetPlugin},
    render_resource::{AsBindGroup, AsBindGroupError, BindGroupLayout, PreparedBindGroup},
    renderer::RenderDevice,
    sync_world::{MainEntity, MainEntityHashMap},
    view::ViewVisibility,
    Extract, ExtractSchedule, RenderApp,
};

use crate::material_pipeline::MaterialPipeline;
use crate::{
    component::MaterialComponent,
    shaders::{LoadedShaders, Shaders},
};

pub enum SpecializeMaterialError {}

pub trait BaseMaterial: Asset + AsBindGroup + Clone + Sized {}

impl<T: Asset + AsBindGroup + Clone + Sized> BaseMaterial for T {}

pub trait Material<P: MaterialPipeline>: BaseMaterial {
    fn properties(&self) -> P::MaterialProperties;
    fn shaders() -> Shaders<P>;
    fn specialize(ctx: P::PipelineContext<'_, Self>) -> Result<(), SpecializeMaterialError>;
}

pub struct MaterialPlugin<M: Material<P>, P: MaterialPipeline>(PhantomData<fn(M, P)>);

impl<M: Material<P>, P: MaterialPipeline> Default for MaterialPlugin<M, P> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<M: Material<P>, P: MaterialPipeline> Plugin for MaterialPlugin<M, P> {
    fn build(&self, app: &mut App) {
        app.register_type::<MaterialComponent<M, P>>()
            .init_asset::<M>()
            .add_plugins((
                RenderAssetPlugin::<MaterialBindGroup<M>>::default(),
                RenderAssetPlugin::<MaterialProperties<M, P>>::default(),
            ))
            .init_resource::<MaterialShaders<M, P>>()
            .add_systems(
                ExtractSchedule,
                (clear_material_instances::<M, P>, extract_materials::<M, P>).chain(),
            )
            .add_plugins(P::material_plugin::<M>());
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<MaterialLayout<M>>();
        }
    }
}

fn clear_material_instances<M: Material<P>, P: MaterialPipeline>(
    mut material_instances: ResMut<MaterialInstances<M, P>>,
) {
    material_instances.clear();
}

fn extract_materials<M: Material<P>, P: MaterialPipeline>(
    mut material_instances: ResMut<MaterialInstances<M, P>>,
    materials: Extract<Query<(&MainEntity, &ViewVisibility, &MaterialComponent<M, P>)>>,
) {
    for (main_entity, view_visibility, material) in &materials {
        if view_visibility.get() {
            material_instances.insert(*main_entity, material.id());
        }
    }
}

/// Stores all extracted instances of a [`Material`] in the render world.
#[derive(Resource, Deref, DerefMut)]
pub struct MaterialInstances<M: Material<P>, P: MaterialPipeline> {
    #[deref]
    instances: MainEntityHashMap<AssetId<M>>,
    _data: PhantomData<fn(P)>,
}

impl<M: Material<P>, P: MaterialPipeline> Default for MaterialInstances<M, P> {
    fn default() -> Self {
        Self {
            instances: Default::default(),
            _data: PhantomData,
        }
    }
}

/// Data prepared for a [`Material`] instance.
#[derive(Deref)]
pub struct MaterialBindGroup<M: BaseMaterial> {
    bind_group: PreparedBindGroup<M::Data>,
}

impl<M: BaseMaterial> RenderAsset for MaterialBindGroup<M> {
    type SourceAsset = M;

    type Param = (SRes<RenderDevice>, SRes<MaterialLayout<M>>, M::Param);

    fn prepare_asset(
        material: Self::SourceAsset,
        (render_device, layout, ref mut material_param): &mut SystemParamItem<Self::Param>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        match material.as_bind_group(layout, render_device, material_param) {
            Ok(bind_group) => Ok(MaterialBindGroup { bind_group }),
            Err(AsBindGroupError::RetryNextUpdate) => {
                Err(PrepareAssetError::RetryNextUpdate(material))
            }
            Err(other) => Err(PrepareAssetError::AsBindGroupError(other)),
        }
    }
}

#[derive(Deref)]
pub struct MaterialProperties<M: Material<R>, R: MaterialPipeline> {
    #[deref]
    properties: R::MaterialProperties,
    _data: PhantomData<fn(M)>,
}

impl<M: Material<R>, R: MaterialPipeline> MaterialProperties<M, R> {
    pub fn new(material: &M) -> Self {
        Self {
            properties: material.properties(),
            _data: PhantomData,
        }
    }
}

impl<M: Material<P>, P: MaterialPipeline> RenderAsset for MaterialProperties<M, P> {
    type SourceAsset = M;

    type Param = ();

    fn prepare_asset(
        material: Self::SourceAsset,
        (): &mut SystemParamItem<Self::Param>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        Ok(MaterialProperties::new(&material))
    }
}

#[derive(Resource, Deref)]
pub struct MaterialLayout<M: BaseMaterial> {
    #[deref]
    layout: BindGroupLayout,
    _data: PhantomData<M>,
}

impl<M: BaseMaterial> FromWorld for MaterialLayout<M> {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        Self {
            layout: M::bind_group_layout(render_device),
            _data: PhantomData,
        }
    }
}

#[derive(Deref, Resource)]
pub struct MaterialShaders<M: Material<P>, P: MaterialPipeline> {
    #[deref]
    shaders: LoadedShaders<P>,
    _data: PhantomData<fn(M)>,
}

impl<M: Material<P>, P: MaterialPipeline> FromWorld for MaterialShaders<M, P> {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        let mut shaders = P::default_shaders();
        shaders.extend(M::shaders());

        Self {
            shaders: shaders.load(asset_server),
            _data: PhantomData,
        }
    }
}