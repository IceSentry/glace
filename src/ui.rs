use bevy::prelude::*;

use crate::{egui_plugin::EguiCtxRes, ComputePushConstants};

pub fn ui(egui_ctx: Res<EguiCtxRes>, mut compute_push_constants: ResMut<ComputePushConstants>) {
    egui::Window::new("Hello").show(&egui_ctx.0, |ui| {
        ui.label("top color:");
        drag_vec4(ui, &mut compute_push_constants.data1, 0.05);
        ui.label("bottom color:");
        drag_vec4(ui, &mut compute_push_constants.data2, 0.05);
        drag_vec4(ui, &mut compute_push_constants.data3, 0.05);
        drag_vec4(ui, &mut compute_push_constants.data4, 0.05);
        //egui_ctx.settings_ui(ui);
    });
}
fn drag_vec4(ui: &mut egui::Ui, value: &mut Vec4, speed: f32) -> bool {
    let mut changed = false;
    ui.columns(4, |ui| {
        changed |= ui[0]
            .add_sized([0.0, 0.0], egui::DragValue::new(&mut value.x).speed(speed))
            .changed();
        changed |= ui[1]
            .add_sized([0.0, 0.0], egui::DragValue::new(&mut value.y).speed(speed))
            .changed();
        changed |= ui[2]
            .add_sized([0.0, 0.0], egui::DragValue::new(&mut value.z).speed(speed))
            .changed();
        changed |= ui[3]
            .add_sized([0.0, 0.0], egui::DragValue::new(&mut value.w).speed(speed))
            .changed();
    });
    changed
}
