// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: sevonj
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

mod beautiful;
mod solid;
mod wireframe;

use std::collections::HashMap;

use egui_wgpu::CallbackResources;
use egui_wgpu::CallbackTrait;
use egui_wgpu::RenderState;
use egui_wgpu::wgpu;
use glam::Mat4;
use glam::Vec3;

use crate::app::ui::model_view::resources::solid::SolidUniforms;

pub struct StaticMeshResource {
    // Beautiful stuff
    tex_uvcheck_bind_group: wgpu::BindGroup,
    render_pipelines: HashMap<u16, wgpu::RenderPipeline>,
    render_lods: Vec<beautiful::RenderMesh>,

    // Solid stuff
    solid_uniform_buf: wgpu::Buffer,
    solid_bind_group: wgpu::BindGroup,
    solid_pipeline_none: wgpu::RenderPipeline,
    solid_pipeline_bone: wgpu::RenderPipeline,
    shadow_lods: Vec<solid::ShadowMesh>,

    // Wireframe stuff
    wframe_pipeline: wgpu::RenderPipeline,
    bbox_vbuf: wgpu::Buffer,
    axis_vbuf: wgpu::Buffer,
}

impl StaticMeshResource {
    pub fn new(
        render_state: &RenderState,
        smesh: &v_types::StaticMesh,
        g_smesh: Option<&[u8]>,
    ) -> Self {
        let device = &render_state.device;

        let common_bgl = solid::solid_bgl(device);

        let tex_bgl = beautiful::texture_bgl(device);
        let tex_uvcheck_bind_group =
            beautiful::load_tex_uvcheck(device, &render_state.queue, &tex_bgl);

        let (render_pipelines, render_lods) = if let Some(g_smesh) = g_smesh {
            let buffers = smesh.render_buffers(g_smesh).unwrap();
            let pipelines = beautiful::render_pipelines(render_state, smesh, &common_bgl, &tex_bgl);
            let lods = beautiful::render_geom_lods(device, smesh, &buffers);
            (pipelines, lods)
        } else {
            (HashMap::new(), Vec::new())
        };

        let solid_uniform_buf: wgpu::Buffer = solid::solid_uniform_buf(device);
        let solid_bind_group = solid::solid_bind_group(&solid_uniform_buf, &common_bgl, device);
        let (solid_pipeline_none, solid_pipeline_bone) =
            solid::shadow_pipelines(render_state, &common_bgl);
        let shadow_lods = solid::shadow_lods(device, smesh);

        let wframe_pipeline = wireframe::wframe_pipeline(render_state);
        let bbox_vbuf = wireframe::bbox_vbuf(&smesh.mesh_header.bbox, device);
        let axis_vbuf = wireframe::axis_vbuf(device);

        Self {
            tex_uvcheck_bind_group,
            render_pipelines,
            render_lods,
            solid_uniform_buf,
            solid_bind_group,
            solid_pipeline_none,
            solid_pipeline_bone,
            shadow_lods,
            wframe_pipeline,
            bbox_vbuf,
            axis_vbuf,
        }
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, cb: &StaticMeshCallback) {
        let view = cb.view.to_cols_array_2d();
        let light = cb.view.inverse().transform_vector3(cb.light).to_array();

        queue.write_buffer(
            &self.solid_uniform_buf,
            0,
            bytemuck::bytes_of(&SolidUniforms {
                view,
                light,
                _pad: 0.0,
            }),
        );
    }
}

pub struct StaticMeshCallback {
    pub view: Mat4,
    pub light: Vec3,
    pub show_render: bool,
    pub show_shadow: bool,
    pub show_bbox: bool,
    pub show_origin: bool,
    pub visible_lod: usize,
}

impl CallbackTrait for StaticMeshCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(res) = resources.get::<StaticMeshResource>() {
            res.update_uniforms(queue, self);
        }
        vec![]
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        rpass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(res) = resources.get::<StaticMeshResource>() else {
            return;
        };

        if self.show_render
            && let Some(lod) = res.render_lods.get(self.visible_lod)
        {
            rpass.set_bind_group(0, &res.solid_bind_group, &[]);
            rpass.set_bind_group(1, &res.tex_uvcheck_bind_group, &[]);
            rpass.set_index_buffer(lod.ibuf.slice(..), wgpu::IndexFormat::Uint16);

            for surf in &lod.surfaces {
                let vbuf = &lod.vbufs[surf.vbuf as usize];
                let Some(pipeline) = &res.render_pipelines.get(&surf.material) else {
                    continue;
                };
                rpass.set_pipeline(pipeline);
                rpass.set_vertex_buffer(0, vbuf.slice(..));
                let indices = surf.start_index..(surf.start_index + surf.num_indices as u32);
                let base_vertex = surf.start_vertex as i32;
                rpass.draw_indexed(indices, base_vertex, 0..1);
            }
        }

        if self.show_shadow
            && let Some(sub) = res.shadow_lods.get(self.visible_lod)
        {
            rpass.set_bind_group(0, &res.solid_bind_group, &[]);
            rpass.set_index_buffer(sub.ibuf.slice(..), wgpu::IndexFormat::Uint16);
            if sub.has_bones {
                rpass.set_pipeline(&res.solid_pipeline_bone);
            } else {
                rpass.set_pipeline(&res.solid_pipeline_none);
            }

            for surf in &sub.surfaces {
                rpass.set_vertex_buffer(0, sub.vbuf.slice(..));
                let indices = surf.start_index..(surf.start_index + surf.num_indices as u32);
                let base_vertex = surf.start_vertex as i32;
                rpass.draw_indexed(indices, base_vertex, 0..1);
            }
        }

        if self.show_bbox {
            rpass.set_pipeline(&res.wframe_pipeline);
            rpass.set_bind_group(0, &res.solid_bind_group, &[]);
            rpass.set_vertex_buffer(0, res.bbox_vbuf.slice(..));
            rpass.draw(0..24, 0..1);
        }

        if self.show_origin {
            rpass.set_pipeline(&res.wframe_pipeline);
            rpass.set_bind_group(0, &res.solid_bind_group, &[]);
            rpass.set_vertex_buffer(0, res.axis_vbuf.slice(..));
            rpass.draw(0..6, 0..1);
        }
    }
}
