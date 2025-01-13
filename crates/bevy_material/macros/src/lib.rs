use bevy_macro_utils::BevyManifest;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, Data, DataStruct, DeriveInput};

fn bevy_ecs_path() -> syn::Path {
    BevyManifest::shared().get_path("bevy_ecs")
}

fn bevy_render_path() -> syn::Path {
    BevyManifest::shared().get_path("bevy_material")
}

fn bevy_material_path() -> syn::Path {
    BevyManifest::shared().get_path("bevy_material")
}

#[proc_macro_derive(Pipelines)]
pub fn derive_pipelines(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    let bevy_material_path: syn::Path = bevy_material_path();
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    // TokenStream::from(quote! {
    //     impl #impl_generics #bevy_material_path::material_pipeline::Pipelines for #struct_name #type_generics #where_clause {
    //     }
    // })
    //
    TokenStream::from(quote! {})
}

fn impl_from_world(ast: &DeriveInput) -> proc_macro2::TokenStream {
    let bevy_ecs_path: syn::Path = bevy_ecs_path();
    let struct_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    quote! {
        impl #impl_generics #bevy_ecs_path::world::FromWorld for #struct_name #type_generics #where_clause {
            fn from_world(world: &mut #bevy_ecs_path::world::World) -> Self {
                Self {

                }
            }
        }
    }
}

fn impl_specialize(
    ast: &DeriveInput,
    target: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    todo!()
}

fn impl_get_key(ast: &DeriveInput, target: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    todo!()
}

#[proc_macro_derive(SpecializedRenderPipeline, attributes(exclude_key))]
pub fn derive_specialized_render_pipeline(input: TokenStream) -> TokenStream {
    let bevy_render_path: syn::Path = bevy_render_path();
    let mut ast = parse_macro_input!(input as DeriveInput);

    let Data::Struct(DataStruct { fields, .. }) = &ast.data else {
        return syn::Error::new(
            Span::call_site(),
            "#[derive(SpecializedRenderPipeline)]` only supports structs",
        )
        .into_compile_error()
        .into();
    };

    let from_world_impl = impl_from_world(&ast);

    let render_pipeline_descriptor_path =
        quote! { #bevy_render_path::render_resource::RenderPipelineDescriptor };
    let specialize_impl = impl_specialize(&ast, &render_pipeline_descriptor_path);
    let get_key_impl = impl_get_key(&ast, &render_pipeline_descriptor_path);

    TokenStream::from(quote! {
        #from_world_impl
        #specialize_impl
        #get_key_impl
    })
}

#[proc_macro_derive(SpecializedComputePipeline, attributes(exclude_key))]
pub fn derive_specialized_compute_pipeline(input: TokenStream) -> TokenStream {
    let bevy_material_path: syn::Path = bevy_material_path();
    let bevy_render_path: syn::Path = bevy_render_path();
    let mut ast = parse_macro_input!(input as DeriveInput);

    let Data::Struct(DataStruct { fields, .. }) = &ast.data else {
        return syn::Error::new(
            Span::call_site(),
            "#[derive(SpecializedComputePipeline)]` only supports structs",
        )
        .into_compile_error()
        .into();
    };

    let from_world_impl = impl_from_world(&ast);

    let render_pipeline_descriptor_path =
        quote! { #bevy_render_path::render_resource::ComputePipelineDescriptor };
    let specialize_impl = impl_specialize(&ast, &render_pipeline_descriptor_path);
    let get_key_impl = impl_get_key(&ast, &render_pipeline_descriptor_path);

    TokenStream::from(quote! {
        #from_world_impl
        #specialize_impl
        #get_key_impl
    })
}
