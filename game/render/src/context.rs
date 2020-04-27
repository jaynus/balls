#[cfg(target_os = "macos")]
use cocoa::appkit::{NSView, NSWindow};
#[cfg(target_os = "macos")]
use cocoa::base::id as cocoa_id;
#[cfg(target_os = "macos")]
use metal::CoreAnimationLayer;
#[cfg(target_os = "macos")]
use objc::runtime::YES;
#[cfg(target_os = "macos")]
use std::mem;

#[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
pub use ash::extensions::khr::XlibSurface;

#[cfg(target_os = "windows")]
use ash::extensions::khr::Win32Surface;
#[cfg(target_os = "macos")]
use ash::extensions::mvk::MacOSSurface;

use crate::Destroyable;

pub struct Context {
    pub device: ash::Device,
    pub physical_device: ash::vk::PhysicalDevice,
    pub memory_properties: ash::vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,

    pub allocator: crate::alloc::AllocatorPtr,

    pub surface_capabilities: ash::vk::SurfaceCapabilitiesKHR,
    pub surface_formats: Vec<ash::vk::SurfaceFormatKHR>,

    pub present_queue: ash::vk::Queue,
    pub present_modes: Vec<ash::vk::PresentModeKHR>,

    pub surface_loader: ash::extensions::khr::Surface,

    pub surface: Surface,
    pub swapchain: ash::vk::SwapchainKHR,

    pub present_images: PresentationState,

    pub debug_report_loader: ash::extensions::ext::DebugReport,
    pub debug_call_back: ash::vk::DebugReportCallbackEXT,
}
impl std::ops::Deref for Context {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}
impl AsRef<ash::Device> for Context {
    fn as_ref(&self) -> &ash::Device {
        &self.device
    }
}

pub struct Surface {
    pub surface: ash::vk::SurfaceKHR,
    pub format: ash::vk::SurfaceFormatKHR,
    pub resolution: ash::vk::Extent2D,
}
impl std::ops::Deref for Surface {
    type Target = ash::vk::SurfaceKHR;

    fn deref(&self) -> &Self::Target {
        &self.surface
    }
}
impl AsRef<ash::vk::SurfaceKHR> for Surface {
    fn as_ref(&self) -> &ash::vk::SurfaceKHR {
        &self.surface
    }
}

pub struct PresentationState {
    pub images: Vec<ash::vk::Image>,
    pub views: Vec<ash::vk::ImageView>,
    // pub frame_buffers: Vec<Framebuffer>,
}
impl Destroyable for PresentationState {
    unsafe fn destroy(&mut self, device: &Context) -> Result<(), crate::Error> {
        use ash::version::DeviceV1_0;

        // self.frame_buffers.drain(..).for_each(|v| v.destroy(device));

        self.views
            .drain(..)
            .for_each(|v| device.destroy_image_view(v, None));

        self.images
            .drain(..)
            .for_each(|v| device.destroy_image(v, None));

        Ok(())
    }
}

impl Destroyable for Vec<ash::vk::Semaphore> {
    unsafe fn destroy(&mut self, device: &Context) -> Result<(), crate::Error> {
        use ash::version::DeviceV1_0;

        self.drain(..)
            .for_each(|s| device.destroy_semaphore(s, None));

        Ok(())
    }
}
