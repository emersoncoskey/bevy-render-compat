use bevy_ecs::world::World;
use wgpu::ImageDataLayout;

use crate::{
    render_graph_v2::{RenderGraph, RenderGraphBuilder},
    render_resource::{Texture, TextureDescriptor},
    renderer::RenderDevice,
};

use super::{
    render_deps, IntoRenderResource, RenderHandle, RenderResource, RenderResourceInit,
    RenderResourceMeta, RetainedRenderResource, SimpleResourceStore, WriteRenderResource,
};

impl RenderResource for Texture {
    type Descriptor = TextureDescriptor<'static>;
    type Data = Self;
    type Store = SimpleResourceStore<Self>;

    fn get_store(graph: &RenderGraph) -> &Self::Store {
        &graph.textures
    }

    fn get_store_mut(graph: &mut RenderGraph) -> &mut Self::Store {
        &mut graph.textures
    }

    fn from_data<'a>(data: &'a Self::Data, _world: &'a World) -> Option<&'a Self> {
        Some(data)
    }

    fn from_descriptor(
        descriptor: &Self::Descriptor,
        world: &World,
        render_device: &RenderDevice,
    ) -> Self::Data {
        render_device.create_texture(descriptor)
    }
}

impl WriteRenderResource for Texture {}

impl RetainedRenderResource for Texture {}

impl IntoRenderResource for TextureDescriptor<'static> {
    type Resource = Texture;

    fn into_render_resource(
        self,
        _world: &World,
        render_device: &RenderDevice,
    ) -> RenderResourceInit<Self::Resource> {
        let tex = render_device.create_texture(&self);
        let meta = RenderResourceMeta {
            descriptor: Some(self),
            resource: tex,
        };
        RenderResourceInit::Eager(meta)
    }
}

pub fn new_texture_with_data(
    graph: &mut RenderGraphBuilder,
    descriptor: TextureDescriptor<'static>,
    data_layout: ImageDataLayout,
    data: &'static [u8],
) -> RenderHandle<Texture> {
    let size = descriptor.size;
    let mut tex = graph.new_resource(descriptor);
    let t1 = tex.clone();
    graph.add_node(render_deps(&mut tex), move |ctx, _, queue| {
        queue.write_texture(ctx.get(&t1).as_image_copy(), data, data_layout, size);
    });
    tex
}