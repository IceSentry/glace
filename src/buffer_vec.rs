#![allow(unused)]

use std::{iter, marker::PhantomData};

use bevy::render::render_resource::{
    encase::internal::{WriteInto, Writer},
    Buffer, ShaderType,
};
use wgpu::{BindingResource, BufferAddress, BufferUsages, Device, Queue};

/// Like [`RawBufferVec`], but doesn't require that the data type `T` be
/// [`NoUninit`].
///
/// This is a high-performance data structure that you should use whenever
/// possible if your data is more complex than is suitable for [`RawBufferVec`].
/// The [`ShaderType`] trait from the `encase` library is used to ensure that
/// the data is correctly aligned for use by the GPU.
///
/// For performance reasons, unlike [`RawBufferVec`], this type doesn't allow
/// CPU access to the data after it's been added via [`BufferVec::push`]. If you
/// need CPU access to the data, consider another type, such as
/// [`StorageBuffer`][super::StorageBuffer].
pub struct BufferVec<T>
where
    T: ShaderType + WriteInto,
{
    data: Vec<u8>,
    buffer: Option<Buffer>,
    capacity: usize,
    buffer_usage: BufferUsages,
    label: Option<String>,
    label_changed: bool,
    phantom: PhantomData<T>,
}

impl<T> BufferVec<T>
where
    T: ShaderType + WriteInto,
{
    /// Creates a new [`BufferVec`] with the given [`BufferUsages`].
    pub const fn new(buffer_usage: BufferUsages) -> Self {
        Self {
            data: vec![],
            buffer: None,
            capacity: 0,
            buffer_usage,
            label: None,
            label_changed: false,
            phantom: PhantomData,
        }
    }

    /// Returns a handle to the buffer, if the data has been uploaded.
    #[inline]
    pub fn buffer(&self) -> Option<&Buffer> {
        self.buffer.as_ref()
    }

    /// Returns the binding for the buffer if the data has been uploaded.
    #[inline]
    pub fn binding(&self) -> Option<BindingResource> {
        Some(BindingResource::Buffer(
            self.buffer()?.as_entire_buffer_binding(),
        ))
    }

    /// Returns the amount of space that the GPU will use before reallocating.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of items that have been pushed to this buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len() / u64::from(T::min_size()) as usize
    }

    /// Returns true if the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Adds a new value and returns its index.
    pub fn push(&mut self, value: T) -> usize {
        let element_size = u64::from(T::min_size()) as usize;
        let offset = self.data.len();

        // TODO: Consider using unsafe code to push uninitialized, to prevent
        // the zeroing. It shows up in profiles.
        self.data.extend(iter::repeat(0).take(element_size));

        // Take a slice of the new data for `write_into` to use. This is
        // important: it hoists the bounds check up here so that the compiler
        // can eliminate all the bounds checks that `write_into` will emit.
        let mut dest = &mut self.data[offset..(offset + element_size)];
        value.write_into(&mut Writer::new(&value, &mut dest, 0).unwrap());

        offset / u64::from(T::min_size()) as usize
    }

    /// Changes the debugging label of the buffer.
    ///
    /// The next time the buffer is updated (via [`Self::reserve`]), Bevy will inform
    /// the driver of the new label.
    pub fn set_label(&mut self, label: Option<&str>) {
        let label = label.map(str::to_string);

        if label != self.label {
            self.label_changed = true;
        }

        self.label = label;
    }

    /// Returns the label
    pub fn get_label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Creates a [`Buffer`] on the [`RenderDevice`] with size
    /// at least `size_of::<T>() * capacity`, unless such a buffer already exists.
    ///
    /// If a [`Buffer`] exists, but is too small, references to it will be discarded,
    /// and a new [`Buffer`] will be created. Any previously created [`Buffer`]s
    /// that are no longer referenced will be deleted by the [`Device`]
    /// once it is done using them (typically 1-2 frames).
    ///
    /// In addition to any [`BufferUsages`] provided when
    /// the `BufferVec` was created, the buffer on the [`RenderDevice`]
    /// is marked as [`BufferUsages::COPY_DST`](BufferUsages).
    pub fn reserve(&mut self, capacity: usize, device: &Device) {
        if capacity <= self.capacity && !self.label_changed {
            return;
        }

        self.capacity = capacity;
        let size = u64::from(T::min_size()) as usize * capacity;

        let wgpu_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: self.label.as_deref(),
            size: size as BufferAddress,
            usage: BufferUsages::COPY_DST | self.buffer_usage,
            mapped_at_creation: false,
        });
        self.buffer = Some(Buffer::from(wgpu_buffer));
        self.label_changed = false;
    }

    /// Queues writing of data from system RAM to VRAM using the [`RenderDevice`]
    /// and the provided [`RenderQueue`].
    ///
    /// Before queuing the write, a [`reserve`](BufferVec::reserve) operation is
    /// executed.
    pub fn write_buffer(&mut self, device: &Device, queue: &Queue) {
        if self.data.is_empty() {
            return;
        }

        self.reserve(self.data.len() / u64::from(T::min_size()) as usize, device);

        let Some(buffer) = &self.buffer else { return };
        queue.write_buffer(buffer, 0, &self.data);
    }

    /// Reduces the length of the buffer.
    pub fn truncate(&mut self, len: usize) {
        self.data.truncate(u64::from(T::min_size()) as usize * len);
    }

    /// Removes all elements from the buffer.
    pub fn clear(&mut self) {
        self.data.clear();
    }
}
