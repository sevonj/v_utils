// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: sevonj
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use egui::CollapsingHeader;
use egui::Widget;
use egui_extras::Column;
use egui_extras::TableBuilder;
use v_types::StaticMesh;

use crate::app::widgets::MatConstDisplay;

const ROW_H: f32 = 16.0;
const SPACE: f32 = 8.0;

pub struct MaterialsInspector<'a> {
    smesh: &'a mut StaticMesh,
}

impl<'a> MaterialsInspector<'a> {
    pub fn new(smesh: &'a mut StaticMesh) -> Self {
        Self { smesh }
    }
}

impl Widget for MaterialsInspector<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.scope(|ui| {
            ui.label(format!("Materials: {}", &self.smesh.matlib.materials.len()));

            for (i, mat) in self.smesh.matlib.materials.iter_mut().enumerate() {
                ui.monospace(format!("Shader id: {:08X}", mat.shader));
                ui.monospace(format!("Material id: {:08X}", mat.id));
                ui.monospace(format!("Flags:{:08X}", mat.flags));
                ui.monospace(format!("Unknown1s: {}", mat.unknown1s.len()));

                CollapsingHeader::new("Unknown1s")
                    .id_salt(("unk1", i))
                    .enabled(!mat.unknown1s.is_empty())
                    .show(ui, |ui| {
                        let table_builder = TableBuilder::new(ui)
                            .striped(true)
                            .vscroll(false)
                            .column(Column::remainder());

                        table_builder.body(|body| {
                            body.rows(ROW_H, mat.unknown1s.len(), |mut row| {
                                let index = row.index();
                                row.col(|ui| {
                                    let val = &mat.unknown1s[index];
                                    ui.monospace(format!(
                                        "?: {}, ?:, {}, ?: {}",
                                        val.unk_00, val.unk_02, val.unk_04
                                    ));
                                });
                            });
                        });
                    })
                    .header_response
                    .on_disabled_hover_text("Empty");

                CollapsingHeader::new("Constants A")
                    .id_salt(("consta", i))
                    .enabled(!mat.const_vecs_a.is_empty())
                    .show(ui, |ui| {
                        let table_builder = TableBuilder::new(ui)
                            .striped(true)
                            .vscroll(false)
                            .column(Column::auto())
                            .column(Column::remainder());

                        table_builder.body(|body| {
                            body.rows(ROW_H, mat.const_vecs_a.len(), |mut row| {
                                let index = row.index();
                                row.col(|ui| {
                                    ui.weak(index.to_string());
                                });
                                row.col(|ui| {
                                    ui.add(MatConstDisplay::new(&mut mat.const_vecs_a[index]));
                                });
                            });
                        });
                    })
                    .header_response
                    .on_disabled_hover_text("Empty");

                CollapsingHeader::new("Constants B")
                    .id_salt(("constb", i))
                    .enabled(!mat.const_vecs_b.is_empty())
                    .show(ui, |ui| {
                        let table_builder = TableBuilder::new(ui)
                            .striped(true)
                            .vscroll(false)
                            .column(Column::auto())
                            .column(Column::remainder());

                        table_builder.body(|body| {
                            body.rows(ROW_H, mat.const_vecs_b.len(), |mut row| {
                                let index = row.index();
                                row.col(|ui| {
                                    ui.weak(index.to_string());
                                });
                                row.col(|ui| {
                                    ui.add(MatConstDisplay::new(&mut mat.const_vecs_b[index]));
                                });
                            });
                        });
                    })
                    .header_response
                    .on_disabled_hover_text("Empty");
            }

            ui.add_space(SPACE);
        })
        .response
    }
}
