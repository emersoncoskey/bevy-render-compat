use super::{
    resource::{
        bind_group::{
            RenderGraphBindGroupDescriptor, RenderGraphBindGroupEntry, RenderGraphBindingResource,
        },
        pipeline::RenderGraphComputePipelineDescriptor,
        texture::RenderGraphTextureView,
        RenderDependencies, RenderHandle,
    },
    RenderGraphBuilder,
};
use crate::{prelude::Shader, render_resource::Texture};
use bevy_asset::Handle;
use wgpu::{TextureUsages, TextureViewDescriptor};

pub struct ComputePass<'g> {
    label: &'static str,
    shader: Handle<Shader>,
    bindings: Vec<RenderGraphBindGroupEntry<'g>>,
    dispatch_x: u32,
    dispatch_y: u32,
    dispatch_z: u32,
    graph: &'g mut RenderGraphBuilder<'g>,
}

impl<'g> ComputePass<'g> {
    pub fn new(
        label: &'static str,
        shader: Handle<Shader>,
        graph: &'g mut RenderGraphBuilder<'g>,
    ) -> Self {
        Self {
            label,
            shader,
            bindings: Vec::new(),
            dispatch_x: 1,
            dispatch_y: 1,
            dispatch_z: 1,
            graph,
        }
    }

    pub fn texture(&mut self, texture: RenderHandle<'g, Texture>) -> &mut Self {
        let texture_view = self
            .graph
            .new_texture_view_descriptor(RenderGraphTextureView {
                texture,
                descriptor: TextureViewDescriptor::default(),
            });

        self.bindings.push(RenderGraphBindGroupEntry {
            binding: self.bindings.len() as u32,
            resource: RenderGraphBindingResource::TextureView(texture_view),
        });

        self.graph
            .add_usages(texture, TextureUsages::TEXTURE_BINDING);

        self
    }

    // TODO: Other binding types

    pub fn dispatch_1d(&mut self, x: u32) -> &mut Self {
        self.dispatch_x = x;
        self
    }

    pub fn dispatch_2d(&mut self, x: u32, y: u32) -> &mut Self {
        self.dispatch_x = x;
        self.dispatch_y = y;
        self
    }

    pub fn dispatch_3d(&mut self, x: u32, y: u32, z: u32) -> &mut Self {
        self.dispatch_x = x;
        self.dispatch_y = y;
        self.dispatch_z = z;
        self
    }

    pub fn build(self) {
        let bind_group_layout = self.graph.new_bind_group_layout_descriptor(todo!());

        let bind_group = self
            .graph
            .new_bind_group_descriptor(RenderGraphBindGroupDescriptor {
                label: Some(&self.label),
                layout: bind_group_layout,
                dependencies: todo!(),
                bindings: self.bindings,
            });

        let pipeline =
            self.graph
                .new_compute_pipeline_descriptor(RenderGraphComputePipelineDescriptor {
                    label: Some(self.label.into()),
                    layout: vec![bind_group_layout],
                    push_constant_ranges: vec![], // TODO
                    shader: self.shader,
                    shader_defs: vec![], // TODO
                    entry_point: self.label.into(),
                });

        self.graph.add_compute_node(
            Some(&self.label),
            RenderDependencies::of((&bind_group, &pipeline)),
            |context, _, _, pass| {
                pass.set_bind_group(0, context.get_bind_group(bind_group).expect("TODO"), &[]);
                pass.set_pipeline(context.get_compute_pipeline(pipeline).expect("TODO"));
                pass.dispatch_workgroups(self.dispatch_x, self.dispatch_y, self.dispatch_z);
            },
        );
    }
}
