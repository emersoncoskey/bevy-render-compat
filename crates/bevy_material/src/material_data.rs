use bevy_asset::AssetId;
use bevy_ecs::system::{Res, SystemParam};
use bevy_render::{render_asset::RenderAssets, sync_world::MainEntity};

use crate::{
    material::{
        Material, MaterialBindGroup, MaterialInstances, MaterialLayout, MaterialProperties,
        MaterialShaders,
    },
    material_pipeline::MaterialPipeline,
};

#[derive(SystemParam)]
pub struct MaterialData<'w, M: Material<P>, P: MaterialPipeline> {
    pub layout: Res<'w, MaterialLayout<M>>,
    pub shaders: Res<'w, MaterialShaders<M, P>>,
    pub bind_groups: Res<'w, RenderAssets<MaterialBindGroup<M>>>,
    pub properties: Res<'w, RenderAssets<MaterialProperties<M, P>>>,
}

impl<'w, M: Material<P>, P: MaterialPipeline> MaterialData<'w, M, P> {
    pub fn get(&self, main_entity: MainEntity, id: AssetId<M>) -> Option<MaterialInstance<M, P>> {
        let bind_group = self.bind_groups.get(id)?;
        let properties = self.properties.get(id)?;
        Some(MaterialInstance {
            main_entity,
            layout: &self.layout,
            shaders: &self.shaders,
            bind_group,
            properties,
        })
    }

    pub fn iter<'a>(
        &'a self,
        instances: &'a MaterialInstances<M, P>,
    ) -> impl Iterator<Item = MaterialInstance<'a, M, P>> {
        instances
            .iter()
            .filter_map(|(main_entity, material_id)| self.get(*main_entity, *material_id))
    }
}

pub struct MaterialInstance<'a, M: Material<P>, P: MaterialPipeline> {
    pub main_entity: MainEntity,
    pub properties: &'a P::Properties,
    pub shaders: &'a MaterialShaders<M, P>,
    pub bind_group: &'a MaterialBindGroup<M>,
    pub properties: &'a MaterialProperties<M, P>,
}
