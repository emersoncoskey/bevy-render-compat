pub mod handle;
pub mod material;
pub mod material_data;
pub mod material_pipeline;
pub mod specialize;

#[cfg(test)]
mod tests {
    use bevy_app::{App, Plugin};
    use bevy_reflect::Reflect;
    use bevy_render::render_resource::RenderPipelineDescriptor;

    use crate::handle::MaterialHandle;
    use crate::material::Material;
    use crate::material_pipeline::{MaterialPipeline, MaterialRenderPipeline};
    use crate::specialize::Specialize;

    #[derive(Reflect)]
    pub struct TestPipeline;

    type TestMaterial<M> = MaterialHandle<M, TestPipeline>;
}
