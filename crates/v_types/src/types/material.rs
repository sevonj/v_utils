// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: sevonj
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::io::Write;

use crate::VolitionError;
use crate::util::*;

pub const MAX_MATERIALS: u32 = 200;
pub const MAX_CONSTANTS: u32 = 10000;
pub const MAX_UNKNOWN3S: u32 = 2000;
pub const MAX_UNKNOWN4S: u16 = 50;
pub const MAX_UKNOWN4_VALUE: usize = 0xffff;

#[derive(Debug, Clone)]
pub struct MaterialsDeserialized {
    pub materials: Vec<MaterialDeserialized>,
    pub mat_unknown3s: Vec<MaterialUnknown3>,
    pub mat_unknown4s: Vec<i32>,
}

impl MaterialsDeserialized {
    pub fn from_data(buf: &[u8], data_offset: &mut usize) -> Result<Self, VolitionError> {
        let material_block = MaterialsHeader::from_le_unsized(&buf[*data_offset..])?;
        *data_offset += size_of::<MaterialsHeader>();

        let num_materials = material_block.num_materials as usize;
        let num_mat_consts = material_block.num_const_floats as usize;
        let num_mat_unknown3 = material_block.num_mat_unknown3 as usize;

        let mut material_headers = Vec::with_capacity(num_materials);
        for _ in 0..num_materials {
            material_headers.push(Material::from_le_unsized(&buf[*data_offset..])?);
            *data_offset += size_of::<Material>();
        }
        let mut materials: Vec<MaterialDeserialized> = material_headers
            .iter()
            .map(MaterialDeserialized::new)
            .collect();

        for (material, header) in materials.iter_mut().zip(&material_headers) {
            align(data_offset, 4);
            for _ in 0..header.num_unknown {
                material
                    .unknown1s
                    .push(MaterialUnknown1::from_le_unsized(&buf[*data_offset..])?);
                *data_offset += size_of::<MaterialUnknown1>();
            }
        }

        align(data_offset, 4);

        let mut mat_const_headers = Vec::with_capacity(num_materials);
        for _ in 0..num_materials {
            let a = MaterialConstHeader::from_le_unsized(&buf[*data_offset..])?;
            if !a.num_floats.is_multiple_of(4) {
                return Err(VolitionError::UnexpectedValue {
                    desc: "MaterialConstHeader::num_floats isn't a multiple of 4",
                    got: a.num_floats as i32,
                });
            }
            if !a.first_float.is_multiple_of(4) {
                return Err(VolitionError::UnexpectedValue {
                    desc: "MaterialConstHeader::first_float isn't a multiple of 4",
                    got: a.first_float as i32,
                });
            }
            *data_offset += size_of::<MaterialConstHeader>();

            let b = MaterialConstHeader::from_le_unsized(&buf[*data_offset..])?;
            if !b.num_floats.is_multiple_of(4) {
                return Err(VolitionError::UnexpectedValue {
                    desc: "MaterialConstHeader::num_floats isn't a multiple of 4",
                    got: b.num_floats as i32,
                });
            }
            if !b.first_float.is_multiple_of(4) {
                return Err(VolitionError::UnexpectedValue {
                    desc: "MaterialConstHeader::first_float isn't a multiple of 4",
                    got: b.first_float as i32,
                });
            }
            *data_offset += size_of::<MaterialConstHeader>();
            mat_const_headers.push((a, b));
        }

        align(data_offset, 16);

        let off_consts = *data_offset;
        let end_consts = off_consts + num_mat_consts * 4;

        for (material, (a, b)) in materials.iter_mut().zip(mat_const_headers.clone()) {
            let num_vecs_a = a.num_floats as usize / 4;
            let num_vecs_b = b.num_floats as usize / 4;
            let start_a = off_consts + a.first_float as usize * 4;
            let start_b = off_consts + b.first_float as usize * 4;
            let end_a = start_a + a.num_floats as usize * 4;
            let end_b = start_b + b.num_floats as usize * 4;

            if end_a > end_consts {
                return Err(VolitionError::UnexpectedValue {
                    desc: "Material consts end_a exceeds buffer",
                    got: end_a as i32,
                });
            }
            if end_b > end_consts {
                return Err(VolitionError::UnexpectedValue {
                    desc: "Material consts end_b exceeds buffer",
                    got: end_b as i32,
                });
            }

            material.consts_a_first = a.first_float;
            material.consts_b_first = b.first_float;

            for i in 0..num_vecs_a {
                let off = start_a + i * 16;
                material.const_vecs_a.push([
                    read_f32_le(buf, off),
                    read_f32_le(buf, off + 4),
                    read_f32_le(buf, off + 8),
                    read_f32_le(buf, off + 12),
                ]);
            }

            for i in 0..num_vecs_b {
                let off = start_b + i * 16;
                material.const_vecs_b.push([
                    read_f32_le(buf, off),
                    read_f32_le(buf, off + 4),
                    read_f32_le(buf, off + 8),
                    read_f32_le(buf, off + 12),
                ]);
            }
        }

        *data_offset = end_consts;

        for (material, header) in materials.iter_mut().zip(&material_headers) {
            for i in 0..16 {
                let entry = MaterialTextureEntry::from_le_unsized(&buf[*data_offset..])?;
                if i < header.num_textures && !entry.is_valid() {
                    let got = read_i32_le(buf, *data_offset);
                    return Err(VolitionError::UnexpectedValue {
                        desc: "Found invalid MaterialTextureEntry in used range",
                        got,
                    });
                } else if i >= header.num_textures && !entry.is_placeholder() {
                    let got = read_i32_le(buf, *data_offset);
                    return Err(VolitionError::UnexpectedValue {
                        desc: "Found non-placeholder MaterialTextureEntry in unused range",
                        got,
                    });
                }
                if i < header.num_textures {
                    material.textures.push(entry);
                }
                *data_offset += size_of::<MaterialTextureEntry>();
            }
        }

        let mut mat_unknown3s = Vec::with_capacity(num_mat_unknown3);
        for _ in 0..num_mat_unknown3 {
            mat_unknown3s.push(MaterialUnknown3::from_le_unsized(&buf[*data_offset..])?);
            *data_offset += size_of::<MaterialUnknown3>();
        }

        let mut mat_unknown4s = vec![];
        for unk3 in &mat_unknown3s {
            for _ in 0..unk3.num_mat_unk4 {
                let value = read_i32_le(buf, *data_offset);
                if value as usize > MAX_UKNOWN4_VALUE {
                    return Err(VolitionError::ValueTooHigh {
                        field: "Material unk4",
                        max: MAX_UKNOWN4_VALUE,
                        got: value as usize,
                    });
                }
                mat_unknown4s.push(value);
                *data_offset += 4;
            }
        }

        Ok(Self {
            materials,
            mat_unknown3s,
            mat_unknown4s,
        })
    }

    pub fn write<W: Write>(
        &self,
        w: &mut W,
        data_offset: &mut usize,
    ) -> Result<(), std::io::Error> {
        align_pad(w, data_offset, 4)?;

        let num_const_vectors: usize = self
            .materials
            .iter()
            .map(|mat| mat.const_vecs_a.len() + mat.const_vecs_b.len())
            .sum();
        let num_const_floats: usize = num_const_vectors * 4;
        let len_const_block = num_const_vectors * size_of::<MaterialConst>();

        w.write_all(
            &MaterialsHeader::new(
                self.materials.len() as u32,
                num_const_floats as u32,
                self.mat_unknown3s.len() as u32,
            )
            .to_le_bytes(),
        )?;
        *data_offset += size_of::<MaterialsHeader>();

        for material in &self.materials {
            w.write_all(&material.to_disk().to_le_bytes())?;
            *data_offset += size_of::<Material>();
        }

        for material in &self.materials {
            align_pad(w, data_offset, 4)?;
            for data in &material.unknown1s {
                w.write_all(&data.to_le_bytes())?;
                *data_offset += size_of::<MaterialUnknown1>();
            }
        }

        for material in &self.materials {
            align_pad(w, data_offset, 4)?;
            let num_floats_a = material.const_vecs_a.len() as u32 * 4;
            let num_floats_b = material.const_vecs_b.len() as u32 * 4;

            let a = MaterialConstHeader {
                num_floats: num_floats_a,
                first_float: material.consts_a_first,
            };
            let b = MaterialConstHeader {
                num_floats: num_floats_b,
                first_float: material.consts_b_first,
            };

            w.write_all(&a.to_le_bytes())?;
            *data_offset += size_of::<MaterialConstHeader>();
            w.write_all(&b.to_le_bytes())?;
            *data_offset += size_of::<MaterialConstHeader>();
        }

        align_pad(w, data_offset, 16)?;

        let mut consts_buf = vec![0xff; len_const_block];
        for material in &self.materials {
            let a_off = material.consts_a_first as usize * 4;
            for (i, c) in material.const_vecs_a.iter().enumerate() {
                let off = a_off + i * size_of::<MaterialConst>();
                consts_buf[off..(off + 4)].copy_from_slice(&c[0].to_le_bytes());
                consts_buf[(off + 4)..(off + 8)].copy_from_slice(&c[1].to_le_bytes());
                consts_buf[(off + 8)..(off + 12)].copy_from_slice(&c[2].to_le_bytes());
                consts_buf[(off + 12)..(off + 16)].copy_from_slice(&c[3].to_le_bytes());
            }

            let b_off = material.consts_b_first as usize * 4;
            for (i, c) in material.const_vecs_b.iter().enumerate() {
                let off = b_off + i * size_of::<MaterialConst>();
                consts_buf[off..(off + 4)].copy_from_slice(&c[0].to_le_bytes());
                consts_buf[(off + 4)..(off + 8)].copy_from_slice(&c[1].to_le_bytes());
                consts_buf[(off + 8)..(off + 12)].copy_from_slice(&c[2].to_le_bytes());
                consts_buf[(off + 12)..(off + 16)].copy_from_slice(&c[3].to_le_bytes());
            }
        }

        w.write_all(&consts_buf)?;
        *data_offset += consts_buf.len();

        for material in &self.materials {
            for i in 0..16 {
                w.write_all(
                    &material
                        .textures
                        .get(i)
                        .unwrap_or(&MaterialTextureEntry::placeholder())
                        .to_le_bytes(),
                )?;
                *data_offset += size_of::<MaterialTextureEntry>();
            }
        }

        for unk3 in &self.mat_unknown3s {
            w.write_all(&unk3.to_le_bytes())?;
            *data_offset += size_of::<MaterialUnknown3>();
        }

        for unk4 in &self.mat_unknown4s {
            w.write_all(&unk4.to_le_bytes())?;
            *data_offset += 4;
        }

        Ok(())
    }
}

/// 1:1 from disk
#[derive(Debug, Clone)]
#[repr(C)]
pub struct MaterialsHeader {
    /// Number of [MaterialData] immediately after this header.
    pub num_materials: u32,
    /// Always Zero.
    pub unknown_04: i32,
    /// Always Zero.
    pub unknown_08: i32,
    /// Always Zero.
    pub unknown_0c: i32,
    /// Colors, etc.
    pub num_const_floats: u32,
    /// Always Zero.
    pub unknown_14: i32,
    /// Always Zero.
    pub unknown_18: i32,
    pub num_mat_unknown3: u32,
    /// Always Zero.
    pub unknown_20: i32,
}

impl MaterialsHeader {
    pub fn new(num_materials: u32, num_const_floats: u32, num_mat_unknown3: u32) -> Self {
        Self {
            num_materials,
            unknown_04: 0,
            unknown_08: 0,
            unknown_0c: 0,
            num_const_floats,
            unknown_14: 0,
            unknown_18: 0,
            num_mat_unknown3,
            unknown_20: 0,
        }
    }

    pub fn from_le_unsized(buf: &[u8]) -> Result<Self, VolitionError> {
        check_fits_buf::<Self>(buf)?;
        Self::from_le_bytes(buf[..size_of::<Self>()].try_into().unwrap())
    }

    pub fn from_le_bytes(buf: &[u8; size_of::<Self>()]) -> Result<Self, VolitionError> {
        let num_materials = read_u32_le(buf, 0x0);
        if num_materials > MAX_MATERIALS {
            return Err(VolitionError::ValueTooHigh {
                field: "MaterialBlock::num_materials",
                max: MAX_MATERIALS as usize,
                got: num_materials as usize,
            });
        }

        let unknown_04 = read_i32_le(buf, 0x4);
        if unknown_04 != 0 {
            return Err(VolitionError::ExpectedExactValue {
                field: "MaterialBlock::unknown_04",
                expected: 0,
                got: unknown_04,
            });
        }

        let unknown_08 = read_i32_le(buf, 0x8);
        if unknown_08 != 0 {
            return Err(VolitionError::ExpectedExactValue {
                field: "MaterialBlock::unknown_08",
                expected: 0,
                got: unknown_08,
            });
        }

        let unknown_0c = read_i32_le(buf, 0xc);
        if unknown_0c != 0 {
            return Err(VolitionError::ExpectedExactValue {
                field: "MaterialBlock::unknown_0c",
                expected: 0,
                got: unknown_0c,
            });
        }

        let num_const_floats = read_u32_le(buf, 0x10);
        if num_const_floats > MAX_CONSTANTS {
            return Err(VolitionError::ValueTooHigh {
                field: "MaterialBlock::num_const_floats",
                max: MAX_CONSTANTS as usize,
                got: num_const_floats as usize,
            });
        }

        let unknown_14 = read_i32_le(buf, 0x14);
        if unknown_14 != 0 {
            return Err(VolitionError::ExpectedExactValue {
                field: "MaterialBlock::unknown_14",
                expected: 0,
                got: unknown_14,
            });
        }

        let unknown_18 = read_i32_le(buf, 0x18);
        if unknown_18 != 0 {
            return Err(VolitionError::ExpectedExactValue {
                field: "MaterialBlock::unknown_18",
                expected: 0,
                got: unknown_18,
            });
        }
        let num_mat_unknown3 = read_u32_le(buf, 0x1c);
        if num_mat_unknown3 > MAX_UNKNOWN3S {
            return Err(VolitionError::ValueTooHigh {
                field: "MaterialBlock::num_mat_unknown3",
                max: MAX_UNKNOWN3S as usize,
                got: num_mat_unknown3 as usize,
            });
        }

        let unknown_20 = read_i32_le(buf, 0x20);
        if unknown_20 != 0 {
            return Err(VolitionError::ExpectedExactValue {
                field: "MaterialBlock::unknown_20",
                expected: 0,
                got: unknown_20,
            });
        }

        Ok(Self {
            num_materials,
            unknown_04,
            unknown_08,
            unknown_0c,
            num_const_floats,
            unknown_14,
            unknown_18,
            num_mat_unknown3,
            unknown_20,
        })
    }

    pub fn to_le_bytes(&self) -> [u8; size_of::<Self>()] {
        let mut bytes = [0; size_of::<Self>()];
        bytes[0x00..0x04].copy_from_slice(&self.num_materials.to_le_bytes());
        bytes[0x04..0x08].copy_from_slice(&0_u32.to_le_bytes());
        bytes[0x08..0x0c].copy_from_slice(&0_u32.to_le_bytes());
        bytes[0x0c..0x10].copy_from_slice(&0_u32.to_le_bytes());
        bytes[0x10..0x14].copy_from_slice(&self.num_const_floats.to_le_bytes());
        bytes[0x14..0x18].copy_from_slice(&0_u32.to_le_bytes());
        bytes[0x18..0x1c].copy_from_slice(&0_u32.to_le_bytes());
        bytes[0x1c..0x20].copy_from_slice(&self.num_mat_unknown3.to_le_bytes());
        bytes[0x20..0x24].copy_from_slice(&0_u32.to_le_bytes());
        bytes
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct MaterialDeserialized {
    /// name checksum?
    pub shader_hash: i32,
    /// name checksum?
    pub material_hash: i32,
    pub flags: i32,
    pub unknown1s: Vec<MaterialUnknown1>,
    pub consts_a_first: u32,
    pub const_vecs_a: Vec<MaterialConst>,
    pub consts_b_first: u32,
    pub const_vecs_b: Vec<MaterialConst>,
    pub textures: Vec<MaterialTextureEntry>,
    pub unk_10: i16,
    pub unk_12: i16,
    pub ptr_14: i32,
}

impl MaterialDeserialized {
    pub fn new(mat: &Material) -> Self {
        Self {
            shader_hash: mat.shader_hash,
            material_hash: mat.material_hash,
            flags: mat.flags,
            unknown1s: vec![],
            consts_a_first: 0,
            const_vecs_a: vec![],
            consts_b_first: 0,
            const_vecs_b: vec![],
            textures: vec![],
            unk_10: mat.unk_10,
            unk_12: mat.unk_12,
            ptr_14: mat.ptr_14,
        }
    }

    pub fn to_disk(&self) -> Material {
        Material {
            shader_hash: self.shader_hash,
            material_hash: self.material_hash,
            flags: self.flags,
            num_unknown: self.unknown1s.len() as u16,
            num_textures: self.textures.len() as u16,
            unk_10: self.unk_10,
            unk_12: self.unk_12,
            ptr_14: self.ptr_14,
        }
    }
}

/// 1:1 from disk
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Material {
    /// name checksum?
    pub shader_hash: i32,
    /// name checksum?
    pub material_hash: i32,
    pub flags: i32,
    pub num_unknown: u16,
    pub num_textures: u16,
    pub unk_10: i16,
    pub unk_12: i16,
    pub ptr_14: i32,
    /* Could be:
     * - num_constants u8
     * - max_constants u8
     * - off_texture i32
     * - off_constant_checksums i32
     * - off_constants i32
     */
}

impl Material {
    pub fn from_le_unsized(buf: &[u8]) -> Result<Self, VolitionError> {
        check_fits_buf::<Self>(buf)?;
        Self::from_le_bytes(buf[..size_of::<Self>()].try_into().unwrap())
    }

    pub fn from_le_bytes(buf: &[u8; size_of::<Self>()]) -> Result<Self, VolitionError> {
        let num_unknown = read_u16_le(buf, 0xc);
        let ptr_14 = read_i32_le(buf, 0x14);

        if ![0, -1].contains(&ptr_14) {
            return Err(VolitionError::UnexpectedValue {
                desc: "Material::ptr_14 should be either 0 or -1",
                got: ptr_14,
            });
        }

        if num_unknown != 0 && ptr_14 != -1 {
            return Err(VolitionError::UnexpectedValue {
                desc: "Material::ptr_14 should be either -1 ",
                got: ptr_14,
            });
        }
        /* else if num_unknown == 0 && ptr_14 != 0 {
            // Sometimes still -1, mostly cmesh
            // Is the relationship just a coincidence or
            // do some files get the stuff from elsewhere or what
            // Does this even matter
        } */

        Ok(Self {
            shader_hash: read_i32_le(buf, 0x0),
            material_hash: read_i32_le(buf, 0x4),
            flags: read_i32_le(buf, 0x8),
            num_unknown,
            num_textures: read_u16_le(buf, 0xe),
            unk_10: read_i16_le(buf, 0x10),
            unk_12: read_i16_le(buf, 0x12),
            ptr_14,
        })
    }

    pub fn to_le_bytes(&self) -> [u8; size_of::<Self>()] {
        let mut bytes = [0; size_of::<Self>()];
        bytes[0x00..0x04].copy_from_slice(&self.shader_hash.to_le_bytes());
        bytes[0x04..0x08].copy_from_slice(&self.material_hash.to_le_bytes());
        bytes[0x08..0x0c].copy_from_slice(&self.flags.to_le_bytes());
        bytes[0x0c..0x0e].copy_from_slice(&self.num_unknown.to_le_bytes());
        bytes[0x0e..0x10].copy_from_slice(&self.num_textures.to_le_bytes());
        bytes[0x10..0x12].copy_from_slice(&self.unk_10.to_le_bytes());
        bytes[0x12..0x14].copy_from_slice(&self.unk_12.to_le_bytes());
        bytes[0x14..0x18].copy_from_slice(&self.ptr_14.to_le_bytes());
        bytes
    }
}

/// 1:1 from disk
#[derive(Debug, Clone)]
#[repr(C)]
pub struct MaterialUnknown1 {
    pub unk_00: i16,
    pub unk_02: i16,
    pub unk_04: i16,
}

impl MaterialUnknown1 {
    pub fn from_le_unsized(buf: &[u8]) -> Result<Self, VolitionError> {
        check_fits_buf::<Self>(buf)?;
        Self::from_le_bytes(buf[..size_of::<Self>()].try_into().unwrap())
    }

    pub fn from_le_bytes(buf: &[u8; size_of::<Self>()]) -> Result<Self, VolitionError> {
        Ok(Self {
            unk_00: read_i16_le(buf, 0x0),
            unk_02: read_i16_le(buf, 0x2),
            unk_04: read_i16_le(buf, 0x4),
        })
    }

    pub fn to_le_bytes(&self) -> [u8; size_of::<Self>()] {
        let mut bytes = [0; size_of::<Self>()];
        bytes[0x00..0x02].copy_from_slice(&self.unk_00.to_le_bytes());
        bytes[0x02..0x04].copy_from_slice(&self.unk_02.to_le_bytes());
        bytes[0x04..0x06].copy_from_slice(&self.unk_04.to_le_bytes());
        bytes
    }
}

/// 1:1 from disk
pub type MaterialConst = [f32; 4];

/// 1:1 from disk
#[derive(Debug, Clone)]
#[repr(C)]
pub struct MaterialConstHeader {
    pub num_floats: u32,
    pub first_float: u32,
}

impl MaterialConstHeader {
    pub fn from_le_unsized(buf: &[u8]) -> Result<Self, VolitionError> {
        check_fits_buf::<Self>(buf)?;
        Self::from_le_bytes(buf[..size_of::<Self>()].try_into().unwrap())
    }

    pub fn from_le_bytes(buf: &[u8; size_of::<Self>()]) -> Result<Self, VolitionError> {
        Ok(Self {
            num_floats: read_u32_le(buf, 0x0),
            first_float: read_u32_le(buf, 0x4),
        })
    }

    pub fn to_le_bytes(&self) -> [u8; size_of::<Self>()] {
        let mut bytes = [0; size_of::<Self>()];
        bytes[0x00..0x04].copy_from_slice(&self.num_floats.to_le_bytes());
        bytes[0x04..0x08].copy_from_slice(&self.first_float.to_le_bytes());
        bytes
    }
}

/// 1:1 from disk
#[derive(Debug, Clone)]
#[repr(C)]
pub struct MaterialTextureEntry {
    /// Texture index. -1 if entry is unused
    pub index: i16,
    /// Texture flags? -1 if entry is unused
    pub flags: i16,
}

impl MaterialTextureEntry {
    pub const fn is_placeholder(&self) -> bool {
        self.index == -1 && self.flags == -1
    }

    pub const fn is_valid(&self) -> bool {
        self.index >= 0 && self.flags != -1
    }

    pub const fn placeholder() -> Self {
        Self {
            index: -1,
            flags: -1,
        }
    }

    pub fn from_le_unsized(buf: &[u8]) -> Result<Self, VolitionError> {
        check_fits_buf::<Self>(buf)?;
        Self::from_le_bytes(buf[..size_of::<Self>()].try_into().unwrap())
    }

    pub fn from_le_bytes(buf: &[u8; size_of::<Self>()]) -> Result<Self, VolitionError> {
        Ok(Self {
            index: read_i16_le(buf, 0x0),
            flags: read_i16_le(buf, 0x2),
        })
    }

    pub fn to_le_bytes(&self) -> [u8; size_of::<Self>()] {
        let mut bytes = [0; size_of::<Self>()];
        bytes[0x00..0x02].copy_from_slice(&self.index.to_le_bytes());
        bytes[0x02..0x04].copy_from_slice(&self.flags.to_le_bytes());
        bytes
    }
}

/// 1:1 from disk
#[derive(Debug, Clone)]
#[repr(C)]
pub struct MaterialUnknown3 {
    pub unk_00: i32,
    pub unk_04: i32,
    pub num_mat_unk4: u16,
    pub unk_06: i16,
    pub ptr_08: i32,
}

impl MaterialUnknown3 {
    pub fn from_le_unsized(buf: &[u8]) -> Result<Self, VolitionError> {
        check_fits_buf::<Self>(buf)?;
        Self::from_le_bytes(buf[..size_of::<Self>()].try_into().unwrap())
    }

    pub fn from_le_bytes(buf: &[u8; size_of::<Self>()]) -> Result<Self, VolitionError> {
        let num_mat_unk4 = read_u16_le(buf, 0x8);
        if num_mat_unk4 > MAX_UNKNOWN4S {
            return Err(VolitionError::ValueTooHigh {
                field: "MaterialBlock::num_mat_unk4",
                max: MAX_UNKNOWN4S as usize,
                got: num_mat_unk4 as usize,
            });
        }

        let ptr_08 = read_i32_le(buf, 0xc);
        if ptr_08 != -1 {
            return Err(VolitionError::ExpectedExactValue {
                field: "MaterialBlock::ptr_08",
                expected: -1,
                got: ptr_08,
            });
        }

        Ok(Self {
            unk_00: read_i32_le(buf, 0x0),
            unk_04: read_i32_le(buf, 0x4),
            num_mat_unk4,
            unk_06: read_i16_le(buf, 0xa),
            ptr_08,
        })
    }

    pub fn to_le_bytes(&self) -> [u8; size_of::<Self>()] {
        let mut bytes = [0; size_of::<Self>()];
        bytes[0x00..0x04].copy_from_slice(&self.unk_00.to_le_bytes());
        bytes[0x04..0x08].copy_from_slice(&self.unk_04.to_le_bytes());
        bytes[0x08..0x0a].copy_from_slice(&self.num_mat_unk4.to_le_bytes());
        bytes[0x0a..0x0c].copy_from_slice(&self.unk_06.to_le_bytes());
        bytes[0x0c..0x10].copy_from_slice(&(-1_i32).to_le_bytes());
        bytes
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_material_block_size() {
        assert_eq!(size_of::<MaterialsHeader>(), 0x24);
    }

    #[test]
    fn test_materials_header_cycle() {
        let mut buf: Vec<u8> = vec![];

        buf.extend_from_slice(&9_i32.to_le_bytes()); // num_materials
        buf.extend_from_slice(&0_i32.to_le_bytes());
        buf.extend_from_slice(&0_i32.to_le_bytes());
        buf.extend_from_slice(&0_i32.to_le_bytes());
        buf.extend_from_slice(&10_i32.to_le_bytes()); // num_const_floats
        buf.extend_from_slice(&0_i32.to_le_bytes());
        buf.extend_from_slice(&0_i32.to_le_bytes());
        buf.extend_from_slice(&11_i32.to_le_bytes()); // num_mat_unknown3
        buf.extend_from_slice(&0_i32.to_le_bytes());

        let hed = MaterialsHeader::from_le_unsized(&buf).unwrap();
        assert_eq!(buf, hed.to_le_bytes());
    }

    #[test]
    fn test_material_size() {
        assert_eq!(size_of::<Material>(), 0x18);
    }

    #[test]
    fn test_material_cycle() {
        let mut buf: Vec<u8> = vec![];

        buf.extend_from_slice(&0xba115101_u32.to_le_bytes());
        buf.extend_from_slice(&0xb00bfee7_u32.to_le_bytes());
        buf.extend_from_slice(&1_i32.to_le_bytes());
        buf.extend_from_slice(&3_i16.to_le_bytes());
        buf.extend_from_slice(&4_i16.to_le_bytes());
        buf.extend_from_slice(&5_i16.to_le_bytes());
        buf.extend_from_slice(&6_i16.to_le_bytes());
        buf.extend_from_slice(&(-1_i32).to_le_bytes());

        let hed = Material::from_le_unsized(&buf).unwrap();
        assert_eq!(buf, hed.to_le_bytes());
    }

    #[test]
    fn test_material_unk3() {
        assert_eq!(size_of::<MaterialUnknown3>(), 0x10);
    }

    #[test]
    fn test_material_unk3_cycle() {
        let mut buf: Vec<u8> = vec![];

        buf.extend_from_slice(&1_i32.to_le_bytes());
        buf.extend_from_slice(&2_i32.to_le_bytes());
        buf.extend_from_slice(&3_i16.to_le_bytes());
        buf.extend_from_slice(&4_i16.to_le_bytes());
        buf.extend_from_slice(&(-1_i32).to_le_bytes());

        let hed = MaterialUnknown3::from_le_unsized(&buf).unwrap();
        assert_eq!(buf, hed.to_le_bytes());
    }
}
