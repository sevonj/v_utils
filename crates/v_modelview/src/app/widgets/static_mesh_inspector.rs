// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: sevonj
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use egui::CollapsingHeader;
use egui::DragValue;
use egui::Label;
use egui::TextWrapMode;
use egui::Widget;
use egui_extras::Column;
use egui_extras::TableBuilder;
use v_types::StaticMesh;
use v_types::StaticMeshHeader;
use v_types::StaticMeshNavpoint;

use crate::app::widgets::FloatDisplay;
use crate::app::widgets::LodMeshInspector;
use crate::app::widgets::MaterialsInspector;
use crate::app::widgets::QuatDisplay;
use crate::app::widgets::VectorDisplay;

const ROW_H: f32 = 16.0;
const IDX_W: f32 = 20.0;
const SPACE: f32 = 8.0;

pub struct StaticMeshInspector<'a> {
    smesh: &'a mut StaticMesh,
}

impl<'a> StaticMeshInspector<'a> {
    pub fn new(smesh: &'a mut StaticMesh) -> Self {
        Self { smesh }
    }
}

impl Widget for StaticMeshInspector<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.scope(|ui| {
            let smesh = self.smesh;

            let has_textures = smesh.header.num_textures > 0;
            let has_navpoints = smesh.header.num_navpoints > 0;
            let has_bones = smesh.header.num_bones > 0;

            header_ui(ui, &smesh.header);

            ui.add_space(SPACE);

            CollapsingHeader::new("Textures")
                .enabled(has_textures)
                .show(ui, |ui| {
                    textures_ui(ui, &mut smesh.texture_addr_maybe, &mut smesh.texture_names);
                })
                .header_response
                .on_disabled_hover_text("No textures");

            ui.add_space(SPACE);

            CollapsingHeader::new("Navpoints")
                .enabled(has_navpoints)
                .show(ui, |ui| {
                    navpoints_ui(ui, &smesh.navpoints);
                })
                .header_response
                .on_disabled_hover_text("No navpoints");

            ui.add_space(SPACE);

            CollapsingHeader::new("Bone Indices(?)")
                .enabled(has_bones)
                .show(ui, |ui| {
                    bone_indices_ui(ui, &smesh.bone_indices);
                })
                .header_response
                .on_disabled_hover_text("No bones");

            ui.add_space(SPACE);

            CollapsingHeader::new("Materials").show(ui, |ui| {
                ui.add(MaterialsInspector::new(smesh));
            });

            ui.add_space(SPACE);

            ui.add(LodMeshInspector::new(smesh))
        })
        .response
    }
}

fn header_ui(ui: &mut egui::Ui, header: &StaticMeshHeader) {
    let table_builder = TableBuilder::new(ui)
        .striped(true)
        .vscroll(false)
        .column(Column::auto())
        .column(Column::remainder())
        .header(ROW_H, |mut row| {
            row.col(|ui| {
                ui.weak("Field");
            });
            row.col(|ui| {
                ui.weak("Value");
            });
        });

    table_builder.body(|mut body| {
        body.row(ROW_H, |mut row| {
            row.col(|ui| {
                ui.label("Flags:");
            });
            row.col(|ui| {
                ui.monospace(format!("{:08X?}", header.mesh_flags));
            });
        });

        body.row(ROW_H, |mut row| {
            row.col(|ui| {
                ui.label("unk_08:");
            });
            row.col(|ui| {
                ui.monospace(format!("{:08X?}", header.unk_08));
            });
        });

        body.row(ROW_H, |mut row| {
            row.col(|ui| {
                ui.label("Textures:");
            });
            row.col(|ui| {
                ui.label(format!("{}", header.num_textures));
            });
        });

        body.row(ROW_H, |mut row| {
            row.col(|ui| {
                ui.label("Navpoints:");
            });
            row.col(|ui| {
                ui.label(format!("{}", header.num_navpoints));
            });
        });

        body.row(ROW_H, |mut row| {
            row.col(|ui| {
                ui.label("unk_10:");
            });
            row.col(|ui| {
                ui.monospace(format!("{:08X?}", header.unk_10));
            });
        });

        body.row(ROW_H, |mut row| {
            row.col(|ui| {
                ui.label("Bounding Center:");
            });
            row.col(|ui| {
                ui.add(VectorDisplay::new(header.bounding_center));
            });
        });

        body.row(ROW_H, |mut row| {
            row.col(|ui| {
                ui.label("Bounding Radius:");
            });
            row.col(|ui| {
                ui.add(FloatDisplay::new(header.bounding_radius));
            });
        });

        body.row(ROW_H, |mut row| {
            row.col(|ui| {
                ui.label("Bones(?):");
            });
            row.col(|ui| {
                ui.label(format!("{}", header.num_bones));
            });
        });

        body.row(ROW_H, |mut row| {
            row.col(|ui| {
                ui.label("unk_28:");
            });
            row.col(|ui| {
                ui.monospace(format!("{:08X?}", header.unk_28));
            });
        });

        body.row(ROW_H, |mut row| {
            row.col(|ui| {
                ui.label("unk_2c:");
            });
            row.col(|ui| {
                ui.monospace(format!("{:08X?}", header.unk_2c));
            });
        });
    });
}

fn textures_ui(ui: &mut egui::Ui, texture_addr_maybe: &mut [i32], texture_names: &mut [String]) {
    assert_eq!(texture_addr_maybe.len(), texture_names.len());

    let num_textures = texture_addr_maybe.len();

    let table_builder = TableBuilder::new(ui)
        .striped(true)
        .vscroll(false)
        .column(Column::exact(IDX_W))
        .column(Column::auto())
        .column(Column::remainder())
        .header(ROW_H, |mut row| {
            row.col(|_| {});
            row.col(|ui| {
                ui.weak("???");
            });
            row.col(|ui| {
                ui.weak("Name");
            });
        });

    table_builder.body(|body| {
        body.rows(ROW_H, num_textures, |mut row| {
            let idx = row.index();
            row.col(|ui| {
                ui.weak(format!("{idx}"));
            });
            row.col(|ui| {
                ui.add(DragValue::new(&mut texture_addr_maybe[idx]).hexadecimal(8, true, true));
            });
            row.col(|ui| {
                ui.text_edit_singleline(&mut texture_names[idx]);
            });
        });
    });
}

fn navpoints_ui(ui: &mut egui::Ui, navpoints: &[StaticMeshNavpoint]) {
    let num_navpoints = navpoints.len();

    let table_builder = TableBuilder::new(ui)
        .striped(true)
        .vscroll(false)
        .column(Column::remainder());

    table_builder.body(|body| {
        body.rows(ROW_H * 3.0, num_navpoints, |mut row| {
            let idx = row.index();
            let navpoint = &navpoints[idx];
            row.col(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let name = navpoint.name().unwrap_or("[INVALID NAME]");
                        ui.weak(format!("{idx}"));
                        ui.add(Label::new(name).wrap_mode(TextWrapMode::Truncate));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Pos:");
                        ui.add(VectorDisplay::new(navpoint.pos));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Orient:");
                        ui.add(QuatDisplay::new(navpoint.orient));
                    });
                });
            });
        });
    });
}

fn bone_indices_ui(ui: &mut egui::Ui, bone_indices: &[i32]) {
    let num_bone_indices = bone_indices.len();

    let table_builder = TableBuilder::new(ui)
        .striped(true)
        .vscroll(false)
        .column(Column::exact(IDX_W))
        .column(Column::remainder());

    table_builder.body(|body| {
        body.rows(ROW_H, num_bone_indices, |mut row| {
            let idx = row.index();
            let value = bone_indices[idx];

            row.col(|ui| {
                ui.weak(format!("{idx:<3}"));
            });
            row.col(|ui| {
                ui.label(value.to_string());
            });
        });
    });
}
