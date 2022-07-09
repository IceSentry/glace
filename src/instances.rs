use bevy::prelude::{Added, Changed, Commands, Component, Entity, Or, Query, Res, With, Without};
use wgpu::util::DeviceExt;

use crate::{model::Model, renderer::WgpuRenderer, transform::Transform};

#[derive(Component)]
pub struct InstanceBuffer(pub wgpu::Buffer);

/// If you want to spawn multiple instances of the same mesh you need to
/// specify the Transform of each instance in this component.
/// If the renderer sees this component it will draw it using draw_instanced
#[derive(Component)]
pub struct Instances(pub Vec<Transform>);

/// Creates the necessary IntanceBuffer on any Model created with a Model and a Transform or Instances
pub fn create_instance_buffer(
    mut commands: Commands,
    renderer: Res<WgpuRenderer>,
    query: Query<
        (Entity, Option<&Transform>, Option<&Instances>),
        (
            Or<(
                (Added<Model>, With<Transform>),
                (Added<Model>, With<Instances>),
                (With<Model>, Added<Transform>),
                (With<Model>, Added<Instances>),
            )>,
            Without<InstanceBuffer>,
        ),
    >,
) {
    for (entity, transform, instances) in query.iter() {
        let instance_data = if let Some(transform) = transform {
            vec![transform.to_raw()]
        } else if let Some(instances) = instances {
            instances.0.iter().map(Transform::to_raw).collect()
        } else {
            log::warn!("Trying to create instance buffer without Transform or Instances");
            continue;
        };

        log::info!("creating instance buffer");

        let instance_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&instance_data),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        commands
            .entity(entity)
            .insert(InstanceBuffer(instance_buffer));
    }
}

#[allow(clippy::type_complexity)]
pub fn update_instance_buffer(
    renderer: Res<WgpuRenderer>,
    query: Query<
        (&InstanceBuffer, Option<&Transform>, Option<&Instances>),
        Or<(Changed<Transform>, Changed<Instances>)>,
    >,
) {
    for (buffer, transform, instances) in query.iter() {
        let data: Vec<_> = if let Some(t) = transform {
            vec![Transform::to_raw(t)]
        } else if let Some(instances) = instances {
            instances.0.iter().map(Transform::to_raw).collect()
        } else {
            unreachable!();
        };

        renderer
            .queue
            .write_buffer(&buffer.0, 0, bytemuck::cast_slice(&data[..]));
    }
}
