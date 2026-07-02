use std::ops::RangeInclusive;

use bevy::prelude::*;

use crate::{ComputeImmediates, egui_plugin::EguiCtxRes};

pub fn ui(egui_ctx: Res<EguiCtxRes>, mut compute_immediates: ResMut<ComputeImmediates>) {
    egui::Window::new("Hello").show(&egui_ctx.0, |ui| {
        ui.label("top color:");
        drag_vec4(ui, &mut compute_immediates.data1, 0.005, 0.0..=1.0);
        ui.label("bottom color:");
        drag_vec4(ui, &mut compute_immediates.data2, 0.005, 0.0..=1.0);
        //egui_ctx.settings_ui(ui);
    });
}
fn drag_vec4(ui: &mut egui::Ui, value: &mut Vec4, speed: f32, range: RangeInclusive<f32>) -> bool {
    let mut changed = false;
    ui.columns(4, |ui| {
        changed |= ui[0]
            .add_sized(
                [0.0, 0.0],
                egui::DragValue::new(&mut value.x)
                    .min_decimals(1)
                    .max_decimals(2)
                    .speed(speed)
                    .range(range.clone()),
            )
            .changed();
        changed |= ui[1]
            .add_sized(
                [0.0, 0.0],
                egui::DragValue::new(&mut value.y)
                    .min_decimals(1)
                    .max_decimals(2)
                    .speed(speed)
                    .range(range.clone()),
            )
            .changed();
        changed |= ui[2]
            .add_sized(
                [0.0, 0.0],
                egui::DragValue::new(&mut value.z)
                    .min_decimals(1)
                    .max_decimals(2)
                    .speed(speed)
                    .range(range.clone()),
            )
            .changed();
        changed |= ui[3]
            .add_sized(
                [0.0, 0.0],
                egui::DragValue::new(&mut value.w)
                    .min_decimals(1)
                    .max_decimals(2)
                    .speed(speed)
                    .range(range.clone()),
            )
            .changed();
    });
    changed
}
