// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: sevonj
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use bytemuck::Pod;
use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

const SHADER: &str = include_str!("shad_solid.wgsl");

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SolidUniforms {
    pub view: [[f32; 4]; 4],
    pub light: [f32; 3],
    pub _pad: f32,
}

pub struct ShadowMesh {
    pub vbuf: wgpu::Buffer,
    pub ibuf: wgpu::Buffer,
    pub surfaces: Vec<v_types::Surface>,
    pub has_bones: bool,
}

pub fn solid_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("smesh_shadow_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

pub fn solid_uniform_buf(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("smesh_shadow_uniforms"),
        size: std::mem::size_of::<SolidUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub fn solid_bind_group(
    uniform_buf: &wgpu::Buffer,
    bgl: &wgpu::BindGroupLayout,
    device: &wgpu::Device,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("smesh_shadow_bg"),
        layout: bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buf.as_entire_binding(),
        }],
    })
}

pub fn shadow_pipelines(
    render_state: &egui_wgpu::RenderState,
    bgl: &wgpu::BindGroupLayout,
) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
    let device = &render_state.device;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("smesh_shadow_shad"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("smesh_shadow_layout"),
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });

    let vertex_none = wgpu::VertexState {
        module: &shader,
        entry_point: Some("vs_main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        buffers: &[wgpu::VertexBufferLayout {
            array_stride: 12 as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3],
        }],
    };

    let vertex_bone = wgpu::VertexState {
        module: &shader,
        entry_point: Some("vs_main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        buffers: &[wgpu::VertexBufferLayout {
            array_stride: 20 as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3],
        }],
    };

    let fragment = Some(wgpu::FragmentState {
        module: &shader,
        entry_point: Some("fs_main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        targets: &[Some(wgpu::ColorTargetState {
            format: render_state.target_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })],
    });

    let primitive = wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleStrip,
        strip_index_format: Some(wgpu::IndexFormat::Uint16),
        cull_mode: Some(wgpu::Face::Back),
        ..Default::default()
    };

    let depth_stencil = Some(wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth32Float,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    });

    let pipeline_none = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("smesh_shadow_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: vertex_none,
        fragment: fragment.clone(),
        primitive,
        depth_stencil: depth_stencil.clone(),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let pipeline_bone = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("smesh_shadow_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: vertex_bone,
        fragment,
        primitive,
        depth_stencil,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    (pipeline_none, pipeline_bone)
}

pub fn shadow_lods(device: &wgpu::Device, smesh: &v_types::StaticMesh) -> Vec<ShadowMesh> {
    smesh
        .lod_meshes
        .iter()
        .filter(|s: &&v_types::LodMeshData| s.shadow_mesh.is_some())
        .map(|s| {
            let mesh: &v_types::Mesh = s.shadow_mesh.as_ref().unwrap();

            let vhead = &mesh.vertex_headers[0];

            let num_bytes = vhead.num_vertices as usize * vhead.stride as usize;
            let slice = &s.shadow_vbuf[0..num_bytes];
            let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("shadow_vbuf"),
                contents: slice,
                usage: wgpu::BufferUsages::VERTEX,
            });

            let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("shadow_ibuf"),
                contents: &s.shadow_ibuf,
                usage: wgpu::BufferUsages::INDEX,
            });

            ShadowMesh {
                vbuf,
                ibuf,
                surfaces: mesh.surfaces.clone(),
                has_bones: vhead.has_bones(),
            }
        })
        .collect()
}
