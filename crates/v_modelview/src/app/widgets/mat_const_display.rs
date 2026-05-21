// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: sevonj
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use egui::DragValue;
use egui::Widget;

pub struct MatConstDisplay<'a> {
    value: &'a mut [f32; 4],
}

impl<'a> MatConstDisplay<'a> {
    pub fn new(value: &'a mut [f32; 4]) -> Self {
        Self { value }
    }
}

impl Widget for MatConstDisplay<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut self.value[0]));
            ui.add(DragValue::new(&mut self.value[1]));
            ui.add(DragValue::new(&mut self.value[2]));
            ui.add(DragValue::new(&mut self.value[3]));

            let mut color = *self.value;
            if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                *self.value = color;
            }
        })
        .response
    }
}
