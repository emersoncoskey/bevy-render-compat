use std::borrow::Cow;

use bevy_asset::Handle;
use bevy_math::UVec3;
use bevy_render::render_resource::{
    BindGroup, BindGroupLayout, BindGroupLayoutEntry, BindingType, Buffer, BufferUsages, Sampler,
    Shader, ShaderDefVal, ShaderStages, TextureDimension, TextureFormat, TextureSampleType,
    TextureUsages, TextureView, TextureViewDimension,
};

use crate::core::{
    resource::{
        bind_group::{
            RenderGraphBindGroupDescriptor, RenderGraphBindGroupEntry, RenderGraphBindingResource,
        },
        texture::RenderGraphTextureViewDescriptor,
        RenderHandle,
    },
    Label, RenderGraphBuilder,
};

//NOTE: these utilities are in an extremely experimental state, and at least BindGroupBuilder will
//require a rework of how resource metadata is stored in order to function. At present, these
//mainly serve as an example of the kind of tools this rewrite enables.

pub struct BindGroupBuilder<'a, 'b: 'a, 'g: 'b> {
    label: Label<'g>,
    graph: &'a mut RenderGraphBuilder<'b, 'g>,
    shader_stages: ShaderStages,
    layout: Vec<BindGroupLayoutEntry>,
    entries: Vec<RenderGraphBindGroupEntry<'g>>,
}

impl<'a, 'b: 'a, 'g: 'b> BindGroupBuilder<'a, 'b, 'g> {
    pub fn new(
        label: Label<'g>,
        graph: &'a mut RenderGraphBuilder<'b, 'g>,
        shader_stages: ShaderStages,
    ) -> Self {
        Self {
            label,
            graph,
            shader_stages,
            layout: Vec::new(),
            entries: Vec::new(),
        }
    }

    pub fn set_shader_stages(&mut self, shader_stages: ShaderStages) -> &mut Self {
        self.shader_stages = shader_stages;
        self
    }

    pub fn sampler(&mut self, sampler: RenderHandle<'g, Sampler>) -> &mut Self {
        let descriptor = self.graph.meta(sampler);
        self.layout.push(BindGroupLayoutEntry {
            binding: self.layout.len() as u32,
            visibility: self.shader_stages,
            ty: BindingType::Sampler(descriptor.binding_type()),
            count: None,
        });
        self.entries.push(RenderGraphBindGroupEntry {
            binding: self.entries.len() as u32,
            resource: RenderGraphBindingResource::Sampler(sampler),
        });
        self
    }

    pub fn texture(&mut self, texture_view: RenderHandle<'g, TextureView>) -> &mut Self {
        let RenderGraphTextureViewDescriptor {
            texture,
            descriptor,
        } = self.graph.meta(texture_view).clone();
        self.graph
            .add_usages(texture, TextureUsages::TEXTURE_BINDING);
        let features = self.graph.features();
        let texture_descriptor = self.graph.meta(texture);
        self.layout.push(BindGroupLayoutEntry {
            binding: self.layout.len() as u32,
            visibility: self.shader_stages,
            ty: BindingType::Texture {
                sample_type: descriptor
                    .format
                    .unwrap_or(texture_descriptor.format)
                    .sample_type(Some(descriptor.aspect), Some(features))
                    .expect("Unable to determine texture sample type from format"),
                view_dimension: descriptor.dimension.unwrap_or({
                    match texture_descriptor.dimension {
                        TextureDimension::D1 => TextureViewDimension::D1,
                        TextureDimension::D2 => TextureViewDimension::D2,
                        TextureDimension::D3 => TextureViewDimension::D3,
                    }
                }),
                multisampled: texture_descriptor.sample_count != 1,
            },
            count: None,
        });
        self.entries.push(RenderGraphBindGroupEntry {
            binding: self.entries.len() as u32,
            resource: RenderGraphBindingResource::TextureView(texture_view),
        });
        self
    }

    pub fn read_storage_texture(&mut self, texture: RenderHandle<'g, TextureView>) -> &mut Self {
        self
    }

    //would infer texture format and dimension from metadata stored in graph.
    pub fn write_storage_texture(&mut self, texture: RenderHandle<'g, TextureView>) -> &mut Self {
        // self.graph
        //     .add_usages(texture, TextureUsages::STORAGE_BINDING);
        // let descriptor = self.graph.descriptor_of(texture);
        // self.layout.push(BindGroupLayoutEntry {
        //     binding: self.layout.len(),
        //     visibility: self.shader_stages,
        //     ty: BindingType::StorageTexture {
        //         access: StorageTextureAccess::ReadWrite,
        //         format: (),
        //         view_dimension: (),
        //     },
        //     count: todo!(),
        // });
        self
    }

    pub fn uniform_buffer(&mut self, buffer: RenderHandle<'g, Buffer>) -> &mut Self {
        self.graph.add_usages(buffer, BufferUsages::UNIFORM);
        self
    }

    pub fn read_storage_buffer(&mut self, buffer: RenderHandle<'g, Buffer>) -> &mut Self {
        self.graph.add_usages(buffer, BufferUsages::STORAGE);
        self
    }

    pub fn write_storage_buffer(&mut self, buffer: RenderHandle<'g, Buffer>) -> &mut Self {
        self.graph.add_usages(buffer, BufferUsages::STORAGE);
        self
    }

    pub fn read_write_storage_buffer(&mut self, buffer: RenderHandle<'g, Buffer>) -> &mut Self {
        self.graph.add_usages(buffer, BufferUsages::STORAGE);
        self
    }

    pub fn build(
        self,
    ) -> (
        RenderHandle<'g, BindGroupLayout>,
        RenderHandle<'g, BindGroup>,
    ) {
        let layout = self.graph.new_resource(self.layout);
        let bind_group = self.graph.new_resource(RenderGraphBindGroupDescriptor {
            label: self.label,
            layout,
            entries: self.entries,
        });
        (layout, bind_group)
    }

    pub fn build_and_return_graph(
        self,
    ) -> (
        RenderHandle<'g, BindGroupLayout>,
        RenderHandle<'g, BindGroup>,
        &'a mut RenderGraphBuilder<'b, 'g>,
    ) {
        let layout = self.graph.new_resource(self.layout);
        let bind_group = self.graph.new_resource(RenderGraphBindGroupDescriptor {
            label: self.label,
            layout,
            entries: self.entries,
        });
        (layout, bind_group, self.graph)
    }
}

pub struct ComputePass<'a, 'b: 'a, 'g: 'b> {
    label: Label<'static>,
    entry_point: Cow<'static, str>,
    bind_group: BindGroupBuilder<'a, 'b, 'g>,
    shader: Handle<Shader>,
    shader_defs: Vec<ShaderDefVal>,
    dispatch_size: UVec3,
}

impl<'a, 'b: 'a, 'g: 'b> ComputePass<'a, 'b, 'g> {
    pub fn new(
        label: Label<'static>,
        entry_point: Cow<'static, str>,
        graph: &'a mut RenderGraphBuilder<'b, 'g>,
        shader: Handle<Shader>,
    ) -> Self {
        Self {
            label: label.clone(),
            entry_point,
            bind_group: BindGroupBuilder::new(label, graph, ShaderStages::COMPUTE),
            shader,
            dispatch_size: UVec3::ONE,
            shader_defs: Vec::new(),
        }
    }

    pub fn sampler(&mut self, sampler: RenderHandle<'g, Sampler>) -> &mut Self {
        self.bind_group.sampler(sampler);
        self
    }

    pub fn texture(&mut self, texture: RenderHandle<'g, TextureView>) -> &mut Self {
        self.bind_group.texture(texture);
        self
    }

    pub fn read_storage_texture(&mut self, texture: RenderHandle<'g, TextureView>) -> &mut Self {
        self.bind_group.read_storage_texture(texture);
        self
    }

    //would infer texture format and dimension from metadata stored in graph.
    pub fn write_storage_texture(&mut self, texture: RenderHandle<'g, TextureView>) -> &mut Self {
        self.bind_group.write_storage_texture(texture);
        self
    }

    // pub fn read_buffer(&mut self, buffer: RenderHandle<'g, Buffer>) -> &mut Self {
    //     self.bind_group.read_buffer(buffer);
    //     self
    // }
    //
    // pub fn write_buffer(&mut self, buffer: RenderHandle<'g, Buffer>) -> &mut Self {
    //     self.bind_group.write_buffer(buffer);
    //     self
    // }

    pub fn dispatch(&mut self, size: UVec3) -> &mut Self {
        self.dispatch_size = size;
        self
    }

    pub fn dispatch_1d(&mut self, x: u32) -> &mut Self {
        self.dispatch(UVec3 { x, y: 1, z: 1 })
    }

    pub fn dispatch_2d(&mut self, x: u32, y: u32) -> &mut Self {
        self.dispatch(UVec3 { x, y, z: 1 })
    }

    pub fn dispatch_3d(&mut self, x: u32, y: u32, z: u32) -> &mut Self {
        self.dispatch(UVec3 { x, y, z })
    }

    pub fn build(self) {
        // let (layout, mut bind_group, graph) = self.bind_group.build_and_return_graph();
        // let pipeline = graph.new_resource(RenderGraphComputePipelineDescriptor {
        //     label: self.label.clone(),
        //     layout: vec![layout],
        //     push_constant_ranges: Vec::new(),
        //     shader: self.shader,
        //     shader_defs: self.shader_defs,
        //     entry_point: self.entry_point,
        // });
        //
        // graph.add_compute_node(
        //     self.label,
        //     deps![&mut bind_group, &pipeline],
        //     move |ctx, pass| {
        //         pass.set_bind_group(0, ctx.get(bind_group).deref(), &[]);
        //         pass.set_pipeline(ctx.get(pipeline).deref());
        //         pass.dispatch_workgroups(
        //             self.dispatch_size.x,
        //             self.dispatch_size.y,
        //             self.dispatch_size.z,
        //         );
        //     },
        // );
    }
}
