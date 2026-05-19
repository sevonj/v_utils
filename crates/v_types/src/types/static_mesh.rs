// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: sevonj
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::io::Write;

use crate::IndexBuffer;
use crate::LodMeshData;
use crate::LodMeshHeader;
use crate::MaterialsDeserialized;
use crate::MeshHeader;
use crate::Quaternion;
use crate::Surface;
use crate::Vector;
use crate::VertexBuffer;
use crate::VolitionError;
use crate::util::*;

pub const MAX_TEXTURES: u16 = 100;
pub const MAX_NAVPOINTS: u16 = 100;
pub const MAX_BONES: u32 = 500;

/// Deserialized cmesh/smesh
#[derive(Debug, Clone)]
pub struct StaticMesh {
    pub header: StaticMeshHeader,
    pub texture_addr_maybe: Vec<i32>,
    pub texture_names: Vec<String>,
    pub navpoints: Vec<StaticMeshNavpoint>,
    /// Maybe.
    pub bone_indices: Vec<i32>,
    pub matlib: MaterialsDeserialized,
    pub mesh_header: LodMeshHeader,
    pub unk_20b: Option<[u8; 20]>,
    pub lod_meshes: Vec<LodMeshData>,
}

impl StaticMesh {
    pub fn from_data(buf: &[u8], data_offset: &mut usize) -> Result<Self, VolitionError> {
        let header = StaticMeshHeader::from_le_unsized(buf)?;

        let num_textures = header.num_textures as usize;
        let num_navpoints = header.num_navpoints as usize;
        let num_bones = header.num_bones as usize;

        *data_offset += 0x40;

        let mut texture_flags = Vec::with_capacity(num_textures);
        for _ in 0..num_textures {
            texture_flags.push(read_i32_le(buf, *data_offset));
            *data_offset += 4;
        }

        align(data_offset, 16);
        *data_offset += 1; // for some there's an extra null byte at start 
        let mut texture_names = Vec::with_capacity(num_textures);
        for _ in 0..num_textures {
            let name = read_cstr(buf, *data_offset)?;
            *data_offset += name.len();
            *data_offset += 1; // nullterm
            texture_names.push(name.to_string());
        }

        let mut navpoints = Vec::with_capacity(num_navpoints);
        if num_navpoints > 0 {
            align(data_offset, 16);
            for _ in 0..num_navpoints {
                navpoints.push(StaticMeshNavpoint::from_le_unsized(&buf[*data_offset..])?);
                *data_offset += size_of::<StaticMeshNavpoint>();
            }
        }

        let mut bone_indices = Vec::with_capacity(num_bones);
        if num_bones > 0 {
            align(data_offset, 16);
            for _ in 0..num_bones {
                bone_indices.push(read_i32_le(buf, *data_offset));
                *data_offset += 4;
            }
        }

        align(data_offset, 4);
        let matlib = MaterialsDeserialized::from_data(buf, data_offset)?;

        let mesh_header = LodMeshHeader::from_le_unsized(&buf[*data_offset..])?;
        *data_offset += size_of::<LodMeshHeader>();

        let unk_20b = if header.unk_2c != 0 {
            let u = read_bytes(buf, *data_offset);
            *data_offset += 20;
            Some(u)
        } else {
            None
        };

        let ret = mesh_header.read_meshes(buf, data_offset)?;
        let lod_meshes = mesh_header.read_data(buf, data_offset, ret)?;

        Ok(Self {
            header,
            texture_addr_maybe: texture_flags,
            texture_names,
            navpoints,
            bone_indices,
            matlib,
            mesh_header,
            unk_20b,
            lod_meshes,
        })
    }

    pub fn write<W: Write>(
        &self,
        w: &mut W,
        data_offset: &mut usize,
    ) -> Result<(), std::io::Error> {
        align_pad(w, data_offset, 16)?;

        w.write_all(&self.header.to_le_bytes())?;
        for _ in size_of::<StaticMeshHeader>()..0x40 {
            w.write_all(&[0])?;
        }
        *data_offset += 0x40;

        for flag in &self.texture_addr_maybe {
            w.write_all(&flag.to_le_bytes())?;
            *data_offset += 4;
        }

        align_pad(w, data_offset, 16)?;
        *data_offset += 1;
        w.write_all(&[0])?;
        for name in &self.texture_names {
            w.write_all(name.as_bytes())?;
            w.write_all(&[0])?;
            *data_offset += name.len() + 1;
        }

        if !self.navpoints.is_empty() {
            align_pad(w, data_offset, 16)?;
            for navpoint in &self.navpoints {
                w.write_all(&navpoint.to_le_bytes())?;
                *data_offset += size_of::<StaticMeshNavpoint>();
            }
        }

        if !self.bone_indices.is_empty() {
            align_pad(w, data_offset, 16)?;
            for boneidx in &self.bone_indices {
                w.write_all(&boneidx.to_le_bytes())?;
                *data_offset += 4;
            }
        }

        self.matlib.write(w, data_offset)?;

        w.write_all(&self.mesh_header.to_le_bytes())?;
        *data_offset += size_of::<LodMeshHeader>();
        if let Some(unk20b) = &self.unk_20b {
            w.write_all(unk20b)?;
            *data_offset += 20;
        }

        align_pad(w, data_offset, 16)?;
        for lod in &self.lod_meshes {
            w.write_all(&lod.mesh.header.to_le_bytes())?;
            *data_offset += size_of::<MeshHeader>();
        }
        for lod in &self.lod_meshes {
            if let Some(shadow_mesh) = &lod.shadow_mesh {
                w.write_all(&shadow_mesh.header.to_le_bytes())?;
                *data_offset += size_of::<MeshHeader>();
            }
        }
        for lod in &self.lod_meshes {
            let mesh = &lod.mesh;
            for surf in &mesh.surfaces {
                w.write_all(&surf.to_le_bytes())?;
                *data_offset += size_of::<Surface>();
            }

            if let Some(shadow_mesh) = &lod.shadow_mesh {
                for surf in &shadow_mesh.surfaces {
                    w.write_all(&surf.to_le_bytes())?;
                    *data_offset += size_of::<Surface>();
                }
            }
        }
        for lod in &self.lod_meshes {
            let mesh = &lod.mesh;
            align_pad(w, data_offset, 4)?;
            w.write_all(&mesh.index_header.to_le_bytes())?;
            *data_offset += size_of::<IndexBuffer>();

            for vertex_header in &mesh.vertex_headers {
                w.write_all(&vertex_header.to_le_bytes())?;
                *data_offset += size_of::<VertexBuffer>();
            }

            if let Some(shadow_mesh) = &lod.shadow_mesh {
                align_pad(w, data_offset, 4)?;
                w.write_all(&shadow_mesh.index_header.to_le_bytes())?;
                *data_offset += size_of::<IndexBuffer>();

                for vertex_header in &shadow_mesh.vertex_headers {
                    w.write_all(&vertex_header.to_le_bytes())?;
                    *data_offset += size_of::<VertexBuffer>();
                }
            }

            if lod.shadow_mesh.is_some() {
                align_pad(w, data_offset, 16)?;
                w.write_all(&lod.shadow_vbuf)?;
                *data_offset += lod.shadow_vbuf.len();

                align_pad(w, data_offset, 16)?;
                w.write_all(&lod.shadow_ibuf)?;
                *data_offset += lod.shadow_ibuf.len();
            }
        }

        align_pad(w, data_offset, 16)?;

        Ok(())
    }

    pub fn dump_wavefront(&self, g_smesh: Option<&[u8]>, separate_surfaces: bool) -> String {
        let mut out = String::new();

        out += "# v_modelview StaticMesh dump\n";
        if self.mesh_header.has_shadow_meshes() {
            out += "# INFO: Model has shadow mesh.\n";
        } else {
            out += "# INFO: Model doesn't have shadow mesh.\n";
        }
        if separate_surfaces {
            out += "# INFO: separate_surfaces enabled. Every surface is a separate object.\n";
        }
        if g_smesh.is_none() {
            out += "# WARNING: GPU file not loaded. Dumping only shadow meshes.\n";
        }

        fn write_vertices(out: &mut String, vhead: &super::VertexBuffer, vbuf: &[u8]) {
            let stride = vhead.stride as usize;
            for i in 0..vhead.num_vertices as usize {
                let v_off = i * stride;
                let pos = Vector::from_le_unsized(&vbuf[v_off..]).unwrap();
                let (u, v) = if vhead.num_uv_channels > 0 {
                    let uv_offset = v_off + vhead.off_uv();
                    (
                        read_i16_le(vbuf, uv_offset) as f32 / 1024.0,
                        read_i16_le(vbuf, uv_offset + 2) as f32 / 1024.0,
                    )
                } else {
                    (0.0, 0.0)
                };
                let normal_offset = v_off + vhead.off_normal();
                let (nx, ny, nz) = if vhead.has_normals() {
                    (
                        vbuf[normal_offset] as f32 / 128.0 - 0.5,
                        vbuf[normal_offset + 1] as f32 / 128.0 - 0.5,
                        vbuf[normal_offset + 2] as f32 / 128.0 - 0.5,
                    )
                } else {
                    (0.0, 0.0, 0.0)
                };
                *out += &format!("v {} {} {}\n", pos.x, pos.y, pos.z);
                *out += &format!("vt {u} {v}\n");
                *out += &format!("vn {nx} {ny} {nz}\n");
            }
        }

        fn write_indices(out: &mut String, base_index: usize, surf: &Surface, ibuf: &[u8]) {
            let start = surf.start_index as usize;
            let end = start + surf.num_indices as usize;

            for i in start..end - 2 {
                let a = base_index + read_u16_le(ibuf, i * 2) as usize;
                let b = base_index + read_u16_le(ibuf, i * 2 + 2) as usize;
                let c = base_index + read_u16_le(ibuf, i * 2 + 4) as usize;
                if (i - start).is_multiple_of(2) {
                    *out += &format!("f {a}/{a}/{a} {b}/{b}/{b} {c}/{c}/{c}\n");
                } else {
                    *out += &format!("f {a}/{a}/{a} {c}/{c}/{c} {b}/{b}/{b}\n");
                }
            }
        }

        let buffers = g_smesh.map(|g_smesh| self.render_buffers(g_smesh).unwrap());
        if let Some(buffers) = &buffers {
            assert_eq!(buffers.len(), self.lod_meshes.len());
        }

        let mut base_index = 1;
        for (lod, mesh) in self.lod_meshes.iter().enumerate() {
            if let Some(buffers) = &buffers {
                let (vbufs, ibuf) = &buffers[lod];
                let mesh = &mesh.mesh;

                if !separate_surfaces {
                    out += &format!("o lod_{lod}\n");
                }

                for (i, surf) in mesh.surfaces.iter().enumerate() {
                    if separate_surfaces {
                        out += &format!("o lod_{lod}_surf{i}\n");
                    }
                    let mat_name = self.matlib.materials[surf.material as usize].id;
                    out += &format!("usemtl mat_{mat_name:08X}\n");

                    let vhead = &mesh.vertex_headers[surf.vbuf as usize];
                    let vbuf = vbufs[surf.vbuf as usize];
                    write_vertices(&mut out, vhead, vbuf);
                    write_indices(&mut out, base_index, surf, ibuf);
                    base_index += vhead.num_vertices as usize;
                }
            }

            if let Some(shadow_mesh) = &mesh.shadow_mesh {
                if !separate_surfaces {
                    out += &format!("o lod_{lod}_shadow\n");
                }

                let vbuf = &mesh.shadow_vbuf;
                let ibuf = &mesh.shadow_ibuf;
                for (i, surf) in shadow_mesh.surfaces.iter().enumerate() {
                    if separate_surfaces {
                        out += &format!("o lod_{lod}_shadow_surf{i}\n");
                    }
                    let mat_name = self.matlib.materials[surf.material as usize].id;
                    out += &format!("usemtl mat_{mat_name:08X}\n");

                    let vhead = &shadow_mesh.vertex_headers[surf.vbuf as usize];
                    write_vertices(&mut out, vhead, vbuf);
                    write_indices(&mut out, base_index, surf, ibuf);
                    base_index += vhead.num_vertices as usize;
                }
            }
        }

        out
    }

    /// Return: vec<(vbufs, ibuf)>
    #[allow(clippy::type_complexity)]
    pub fn render_buffers<'a>(
        &self,
        buf: &'a [u8],
    ) -> Result<Vec<(Vec<&'a [u8]>, &'a [u8])>, VolitionError> {
        let mut offset = 0;
        let mut datas = vec![];
        for lod_mesh in &self.lod_meshes {
            align(&mut offset, 16);
            let mut vbufs = vec![];
            for vertex_head in &lod_mesh.mesh.vertex_headers {
                let len = vertex_head.num_vertices as usize * vertex_head.stride as usize;
                let end = offset + len;
                vbufs.push(&buf[offset..end]);
                offset += len;
            }

            align(&mut offset, 16);
            let len = lod_mesh.mesh.index_header.num_indices as usize * 2;
            let end = offset + len;
            datas.push((vbufs, &buf[offset..end]));
            offset += len;
        }
        Ok(datas)
    }
}

/// 1:1 from disk
#[derive(Debug, Clone)]
#[repr(C)]
pub struct StaticMeshHeader {
    pub magic: i32,
    pub version: i16,
    pub mesh_flags: i16,
    pub unk_08: i32,
    pub num_textures: u16,
    pub num_navpoints: u16,
    pub unk_10: i32,
    pub bounding_center: Vector,
    pub bounding_radius: f32,
    /// maybe?
    pub num_bones: u32,
    pub unk_28: i32,
    pub unk_2c: i32,
}

impl StaticMeshHeader {
    pub const SIGNATURE: i32 = 0x424BD00D;
    pub const VERSION: i16 = 33;

    pub fn from_le_unsized(buf: &[u8]) -> Result<Self, VolitionError> {
        check_fits_buf::<Self>(buf)?;
        Self::from_le_bytes(buf[..size_of::<Self>()].try_into().unwrap())
    }

    pub fn from_le_bytes(buf: &[u8; size_of::<Self>()]) -> Result<Self, VolitionError> {
        let magic = read_i32_le(buf, 0);
        if magic != Self::SIGNATURE {
            return Err(VolitionError::ExpectedExactValue {
                field: "StaticMesh::magic",
                expected: Self::SIGNATURE,
                got: magic,
            });
        }

        let version = read_i16_le(buf, 0x4);
        if version != Self::VERSION {
            return Err(VolitionError::UnknownStaticMeshVersion(version));
        }

        let num_textures = read_u16_le(buf, 0xc);
        if num_textures > MAX_TEXTURES {
            return Err(VolitionError::ValueTooHigh {
                field: "StaticMeshHeader::num_textures",
                max: MAX_TEXTURES as usize,
                got: num_textures as usize,
            });
        }

        let num_navpoints = read_u16_le(buf, 0xe);
        if num_navpoints > MAX_NAVPOINTS {
            return Err(VolitionError::ValueTooHigh {
                field: "StaticMeshHeader::num_navpoints",
                max: MAX_NAVPOINTS as usize,
                got: num_navpoints as usize,
            });
        }

        let num_bones = read_u32_le(buf, 0x24);
        if num_bones > MAX_BONES {
            return Err(VolitionError::ValueTooHigh {
                field: "StaticMeshHeader::num_bones",
                max: MAX_BONES as usize,
                got: num_bones as usize,
            });
        }

        let unk_2c = read_i32_le(buf, 0x2c);
        if ![0, 2].contains(&unk_2c) {
            return Err(VolitionError::UnexpectedValue {
                desc: "StaticMesh::unk_2c expected 0 or 2",
                got: unk_2c,
            });
        }

        Ok(Self {
            magic,
            version,
            mesh_flags: read_i16_le(buf, 0x6),
            unk_08: read_i32_le(buf, 0x8),
            num_textures,
            num_navpoints,
            unk_10: read_i32_le(buf, 0x10),
            bounding_center: Vector::from_le_unsized(&buf[0x14..])?,
            bounding_radius: read_f32_le(buf, 0x20),
            num_bones,
            unk_28: read_i32_le(buf, 0x28),
            unk_2c,
        })
    }

    pub fn to_le_bytes(&self) -> [u8; size_of::<Self>()] {
        let mut bytes = [0; size_of::<Self>()];

        bytes[0x00..0x04].copy_from_slice(&Self::SIGNATURE.to_le_bytes());
        bytes[0x04..0x06].copy_from_slice(&Self::VERSION.to_le_bytes());
        bytes[0x06..0x08].copy_from_slice(&self.mesh_flags.to_le_bytes());
        bytes[0x08..0x0c].copy_from_slice(&self.unk_08.to_le_bytes());
        bytes[0x0c..0x0e].copy_from_slice(&self.num_textures.to_le_bytes());
        bytes[0x0e..0x10].copy_from_slice(&self.num_navpoints.to_le_bytes());
        bytes[0x10..0x14].copy_from_slice(&self.unk_10.to_le_bytes());
        bytes[0x14..0x20].copy_from_slice(&self.bounding_center.to_le_bytes());
        bytes[0x20..0x24].copy_from_slice(&self.bounding_radius.to_le_bytes());
        bytes[0x24..0x28].copy_from_slice(&self.num_bones.to_le_bytes());
        bytes[0x28..0x2c].copy_from_slice(&self.unk_28.to_le_bytes());
        bytes[0x2c..0x30].copy_from_slice(&self.unk_2c.to_le_bytes());

        bytes
    }
}

/// 1:1 from disk
/// Navigation reference point
/// Used for IK?
#[derive(Debug, Clone)]
#[repr(C)]
pub struct StaticMeshNavpoint {
    /// name to reference nav point by
    pub name: [u8; 64],
    /// vid this navp is attached to.
    pub vid: i32,
    /// position of navpoint in object coords
    pub pos: Vector,
    /// quaternion representation of navpoint
    pub orient: Quaternion,
}

impl StaticMeshNavpoint {
    pub fn from_le_unsized(buf: &[u8]) -> Result<Self, VolitionError> {
        check_fits_buf::<Self>(buf)?;
        Self::from_le_bytes(buf[..size_of::<Self>()].try_into().unwrap())
    }

    pub fn from_le_bytes(buf: &[u8; size_of::<Self>()]) -> Result<Self, VolitionError> {
        Ok(Self {
            name: read_bytes(buf, 0x0),
            vid: read_i32_le(buf, 0x40),
            pos: Vector::from_le_unsized(&buf[0x44..])?,
            orient: Quaternion::from_le_unsized(&buf[0x50..])?,
        })
    }

    pub fn to_le_bytes(&self) -> [u8; size_of::<Self>()] {
        let mut bytes = [0; size_of::<Self>()];
        bytes[0x00..0x40].copy_from_slice(&self.name);
        bytes[0x40..0x44].copy_from_slice(&self.vid.to_le_bytes());
        bytes[0x44..0x50].copy_from_slice(&self.pos.to_le_bytes());
        bytes[0x50..].copy_from_slice(&self.orient.to_le_bytes());
        bytes
    }

    pub fn name(&self) -> Result<&str, VolitionError> {
        read_cstr(&self.name, 0)
    }
}

#[cfg(test)]
mod tests {

    use std::io::BufWriter;
    use std::path::PathBuf;

    use super::*;

    const NUM_SMESH: usize = 659;
    const NUM_CMESH: usize = 3028;

    #[test]
    fn test_header_cycle_bytes() {
        let mut buf = vec![];

        buf.extend_from_slice(&StaticMeshHeader::SIGNATURE.to_le_bytes());
        buf.extend_from_slice(&StaticMeshHeader::VERSION.to_le_bytes());
        buf.extend_from_slice(&4_i16.to_le_bytes()); // mesh_flags
        buf.extend_from_slice(&5_i32.to_le_bytes()); // unk_08
        buf.extend_from_slice(&6_i16.to_le_bytes()); // num_textures
        buf.extend_from_slice(&7_i16.to_le_bytes()); // num_navpoints
        buf.extend_from_slice(&7_i32.to_le_bytes()); // unk_10
        buf.extend_from_slice(&3.0_f32.to_le_bytes()); // pos
        buf.extend_from_slice(&4.0_f32.to_le_bytes());
        buf.extend_from_slice(&5.0_f32.to_le_bytes());
        buf.extend_from_slice(&6.0_f32.to_le_bytes()); // radius
        buf.extend_from_slice(&7_u32.to_le_bytes()); // num_bones
        buf.extend_from_slice(&8_i32.to_le_bytes()); // unk_28
        buf.extend_from_slice(&2_i32.to_le_bytes()); // unk_2c

        let hed = StaticMeshHeader::from_le_unsized(&buf).unwrap();
        assert_eq!(buf, hed.to_le_bytes());
    }

    #[test]
    fn test_navpoint_cycle_bytes() {
        let mut buf = vec![];

        buf.extend_from_slice(&[67_u8; 64]);
        buf.extend_from_slice(&0_i32.to_le_bytes());
        buf.extend_from_slice(&3.0_f32.to_le_bytes());
        buf.extend_from_slice(&4.0_f32.to_le_bytes());
        buf.extend_from_slice(&5.0_f32.to_le_bytes());
        buf.extend_from_slice(&(-5.0_f32).to_le_bytes());
        buf.extend_from_slice(&(-5.0_f32).to_le_bytes());
        buf.extend_from_slice(&(-5.0_f32).to_le_bytes());
        buf.extend_from_slice(&(-5.0_f32).to_le_bytes());

        let hed = StaticMeshNavpoint::from_le_unsized(&buf).unwrap();
        assert_eq!(buf, hed.to_le_bytes());
    }

    #[cfg(not(feature = "no_gamedata"))]
    #[test]
    fn test_parse_every_smesh() {
        // Unpacked meshes.vpp_pc
        let samples_path = PathBuf::from("../../samples/meshes_extracted");

        let mut num_failed = 0;
        let mut num_success = 0;
        for entry in std::fs::read_dir(samples_path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if !entry.metadata().unwrap().is_file() || path.extension().unwrap() != "smesh_pc" {
                continue;
            }

            let buf = std::fs::read(&path).unwrap();
            let mut offset = 0;
            if let Err(e) = StaticMesh::from_data(&buf, &mut offset) {
                num_failed += 1;
                println!("ERR: {path:?} {offset:#X?} {e}");
            } else {
                num_success += 1
            }
        }
        println!("num_failed: {num_failed:?} / {NUM_SMESH:?}");
        assert_eq!(num_failed, 0);
        assert_eq!(num_success, NUM_SMESH);
    }

    #[cfg(not(feature = "no_gamedata"))]
    #[test]
    fn test_parse_every_smesh_reaches_end() {
        // Unpacked meshes.vpp_pc
        let samples_path = PathBuf::from("../../samples/meshes_extracted");

        let mut num_failed = 0;
        for entry in std::fs::read_dir(samples_path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if !entry.metadata().unwrap().is_file() || path.extension().unwrap() != "smesh_pc" {
                continue;
            }

            let buf = std::fs::read(&path).unwrap();
            let mut offset = 0;
            StaticMesh::from_data(&buf, &mut offset).unwrap();

            align(&mut offset, 16);

            if offset != buf.len() {
                num_failed += 1;
                println!(
                    "Didn't reach end: {path:?} off: {offset:#X?}, end: {:#X?}",
                    buf.len()
                );
            }
        }
        println!("num_failed: {num_failed:?} / {NUM_SMESH:?}");
        assert_eq!(num_failed, 0);
    }

    #[cfg(not(feature = "no_gamedata"))]
    #[test]
    fn test_cycle_every_smesh() {
        // Unpacked meshes.vpp_pc
        let samples_path = PathBuf::from("../../samples/meshes_extracted");

        let mut num_failed = 0;
        for entry in std::fs::read_dir(samples_path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if !entry.metadata().unwrap().is_file() || path.extension().unwrap() != "smesh_pc" {
                continue;
            }

            let input_buf = std::fs::read(&path).unwrap();
            let smesh = StaticMesh::from_data(&input_buf, &mut 0).unwrap();

            let mut out_buf = Vec::with_capacity(input_buf.len());
            let mut w = BufWriter::new(&mut out_buf);

            smesh.write(&mut w, &mut 0).unwrap();
            drop(w);

            let input_slice = &input_buf[..out_buf.len()];
            if out_buf != input_slice {
                num_failed += 1;
                println!("ERR: {path:?}");
                // println!("exp: {input_slice:?}");
                // println!("got: {out_buf:?}");
            }
        }
        println!("num_failed: {num_failed:?} / {NUM_SMESH:?}");
        assert_eq!(num_failed, 0);
    }

    #[cfg(not(feature = "no_gamedata"))]
    #[test]
    fn test_cycle_every_smesh_to_end() {
        // Unpacked meshes.vpp_pc
        let samples_path = PathBuf::from("../../samples/meshes_extracted");

        let mut num_failed = 0;
        for entry in std::fs::read_dir(samples_path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if !entry.metadata().unwrap().is_file() || path.extension().unwrap() != "smesh_pc" {
                continue;
            }

            let input_buf = std::fs::read(&path).unwrap();
            let smesh = StaticMesh::from_data(&input_buf, &mut 0).unwrap();

            let mut out_buf = Vec::with_capacity(input_buf.len());
            let mut w = BufWriter::new(&mut out_buf);

            smesh.write(&mut w, &mut 0).unwrap();
            drop(w);

            if out_buf != input_buf {
                num_failed += 1;
                println!("ERR: {path:?}");
                println!(
                    "exp len: {:X}, got len: {:X}",
                    input_buf.len(),
                    out_buf.len()
                );
            }
        }
        println!("num_failed: {num_failed:?} / {NUM_SMESH:?}");
        assert_eq!(num_failed, 0);
    }

    #[cfg(not(feature = "no_gamedata"))]
    #[test]
    fn test_parse_every_cmesh() {
        // Unpacked meshes.vpp_pc
        let samples_path = PathBuf::from("../../samples/meshes_extracted");

        let mut num_failed = 0;
        let mut num_success = 0;
        for entry in std::fs::read_dir(samples_path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if !entry.metadata().unwrap().is_file() || path.extension().unwrap() != "cmesh_pc" {
                continue;
            }

            let buf = std::fs::read(&path).unwrap();
            let mut offset = 0;
            if let Err(e) = StaticMesh::from_data(&buf, &mut offset) {
                num_failed += 1;
                println!("ERR: {path:?} {offset:#X?} {e}");
            } else {
                num_success += 1
            }
        }
        println!("num_failed: {num_failed:?} / {NUM_CMESH:?}");
        assert_eq!(num_failed, 0);
        assert_eq!(num_success, NUM_CMESH);
    }

    // #[test]
    // fn test_static_mesh_size() {
    //     assert_eq!(size_of::<StaticMesh>(), 0x40);
    // }
}
