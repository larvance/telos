//! Memory allocation wrapper around `gpu-allocator`.

use ash::vk;
use gpu_allocator::{
    AllocationSizes, AllocatorDebugSettings,
    vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator, AllocatorCreateDesc},
};
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::error::GpuError;

/// Manages dedicated and sub-allocated GPU memory.
pub struct GpuAllocator {
    allocator: Arc<Mutex<Allocator>>,
}

impl GpuAllocator {
    /// Creates a new GPU memory allocator.
    pub fn new(
        instance: ash::Instance,
        device: ash::Device,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Self, GpuError> {
        let desc = AllocatorCreateDesc {
            instance,
            device,
            physical_device,
            debug_settings: AllocatorDebugSettings::default(),
            buffer_device_address: true,
            allocation_sizes: AllocationSizes::new(32 * 1024 * 1024, 32 * 1024 * 1024),
        };

        let allocator = Allocator::new(&desc).map_err(|e| {
            GpuError::Allocation(format!("Failed to initialize gpu-allocator: {e}"))
        })?;

        info!("GPU memory allocator initialized with buffer device address support");

        Ok(Self {
            allocator: Arc::new(Mutex::new(allocator)),
        })
    }

    /// Allocates device memory for a buffer or image.
    pub fn allocate(
        &self,
        name: &'static str,
        requirements: vk::MemoryRequirements,
        location: gpu_allocator::MemoryLocation,
        linear: bool,
    ) -> Result<Allocation, GpuError> {
        let desc = AllocationCreateDesc {
            name,
            requirements,
            location,
            linear,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };

        let mut alloc = self
            .allocator
            .lock()
            .map_err(|_| GpuError::Allocation("Allocator lock poisoned".into()))?;

        match alloc.allocate(&desc) {
            Ok(a) => Ok(a),
            Err(e) => {
                tracing::error!(
                    name = name,
                    size = desc.requirements.size,
                    align = desc.requirements.alignment,
                    type_bits = format!("{:#x}", desc.requirements.memory_type_bits),
                    location = ?desc.location,
                    linear = desc.linear,
                    "Allocation failed: {e}"
                );
                Err(GpuError::Allocation(format!(
                    "Allocation failed for '{name}': {e}"
                )))
            }
        }
    }

    /// Allocates device memory for an image, preferring a dedicated allocation if possible.
    pub fn allocate_image(
        &self,
        name: &'static str,
        image: vk::Image,
        requirements: vk::MemoryRequirements,
        location: gpu_allocator::MemoryLocation,
    ) -> Result<Allocation, GpuError> {
        let dedicated_desc = AllocationCreateDesc {
            name,
            requirements,
            location,
            linear: false,
            allocation_scheme: AllocationScheme::DedicatedImage(image),
        };

        let mut alloc = self
            .allocator
            .lock()
            .map_err(|_| GpuError::Allocation("Allocator lock poisoned".into()))?;

        alloc
            .allocate(&dedicated_desc)
            .or_else(|_| {
                let fallback_desc = AllocationCreateDesc {
                    name,
                    requirements,
                    location,
                    linear: false,
                    allocation_scheme: AllocationScheme::GpuAllocatorManaged,
                };
                alloc.allocate(&fallback_desc)
            })
            .map_err(|e| GpuError::Allocation(format!("Allocation failed for '{name}': {e}")))
    }

    /// Frees an existing GPU memory allocation.
    pub fn free(&self, allocation: Allocation) -> Result<(), GpuError> {
        let mut alloc = self
            .allocator
            .lock()
            .map_err(|_| GpuError::Allocation("Allocator lock poisoned".into()))?;

        alloc
            .free(allocation)
            .map_err(|e| GpuError::Allocation(format!("Free allocation failed: {e}")))
    }
}
