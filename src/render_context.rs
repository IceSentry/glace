use bevy::{
    ecs::{
        resource::Resource,
        system::{Deferred, Res, SystemBuffer, SystemMeta, SystemParam},
        world::{DeferredWorld, World},
    },
    log::info_span,
};
use wgpu::{CommandBuffer, CommandEncoder};

use crate::Device;

#[derive(Default)]
pub struct RenderContextState {
    command_encoder: Option<CommandEncoder>,
    command_buffers: Vec<CommandBuffer>,
    render_device: Option<wgpu::Device>,
}

#[expect(unused)]
impl RenderContextState {
    fn flush_encoder(&mut self) {
        if let Some(encoder) = self.command_encoder.take() {
            self.command_buffers.push(encoder.finish());
        }
    }

    fn command_encoder(&mut self) -> &mut CommandEncoder {
        let render_device = self
            .render_device
            .as_ref()
            .expect("RenderDevice must be set before accessing command_encoder");

        self.command_encoder.get_or_insert_with(|| {
            render_device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default())
        })
    }

    pub fn finish(&mut self) -> Vec<CommandBuffer> {
        self.flush_encoder();
        core::mem::take(&mut self.command_buffers)
    }
}

impl SystemBuffer for RenderContextState {
    fn apply(&mut self, system_meta: &SystemMeta, world: &mut World) {
        let _span = info_span!("RenderContextState::apply", system = %system_meta.name()).entered();

        let has_buffers = !self.command_buffers.is_empty();
        let has_encoder = self.command_encoder.is_some();

        if has_buffers || has_encoder {
            let mut pending = world.resource_mut::<PendingCommandBuffers>();

            if has_buffers {
                pending.push(core::mem::take(&mut self.command_buffers));
            }

            if let Some(encoder) = self.command_encoder.take() {
                pending.push_encoder(encoder);
            }
        }

        self.render_device = None;
    }

    fn queue(&mut self, _system_meta: &SystemMeta, _world: DeferredWorld) {}
}

#[derive(SystemParam)]
pub struct RenderContext<'w, 's> {
    state: Deferred<'s, RenderContextState>,
    render_device: Res<'w, Device>,
    // diagnostics_recorder: Option<Res<'w, DiagnosticsRecorder>>,
}

impl<'w, 's> RenderContext<'w, 's> {
    fn ensure_device(&mut self) {
        if self.state.render_device.is_none() {
            self.state.render_device = Some(self.render_device.clone());
        }
    }

    pub fn command_encoder(&mut self) -> &mut CommandEncoder {
        self.ensure_device();
        self.state.command_encoder()
    }
}

#[derive(Resource, Default)]
pub struct PendingCommandBuffers {
    buffers: Vec<CommandBuffer>,
    encoders: Vec<CommandEncoder>,
}

impl PendingCommandBuffers {
    pub fn push(&mut self, buffers: impl IntoIterator<Item = CommandBuffer>) {
        self.buffers.extend(buffers);
    }

    pub fn push_encoder(&mut self, encoder: CommandEncoder) {
        self.encoders.push(encoder);
    }

    pub fn take(&mut self) -> Vec<CommandBuffer> {
        for encoder in self.encoders.drain(..) {
            self.buffers.push(encoder.finish());
        }
        core::mem::take(&mut self.buffers)
    }

    // pub fn is_empty(&self) -> bool {
    //     self.buffers.is_empty() && self.encoders.is_empty()
    // }
    //
    // pub fn len(&self) -> usize {
    //     self.buffers.len() + self.encoders.len()
    // }
}
