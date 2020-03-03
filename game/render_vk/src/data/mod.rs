use rl_core::{
    failure,
    shrinkwrap::Shrinkwrap,
    slotmap,
    winit::{self, window::Window},
    NamedSlotMap,
};

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
use ash::extensions::khr::XlibSurface;

#[cfg(target_os = "windows")]
use ash::extensions::khr::Win32Surface;
#[cfg(target_os = "macos")]
use ash::extensions::mvk::MacOSSurface;

pub use ash::{
    extensions::{
        ext::DebugReport,
        khr::{Surface, Swapchain},
    },
    version::{DeviceV1_0, EntryV1_0, InstanceV1_0},
    {vk, Device, Entry, Instance},
};
use std::{
    convert::AsRef,
    default::Default,
    ffi::{CStr, CString},
    ops::Deref,
    os::raw::{c_char, c_void},
};

pub mod buffer;
pub mod builders;
pub mod texture;

slotmap::new_key_type! { pub struct BufferHandle; }
slotmap::new_key_type! { pub struct TextureHandle; }

#[derive(Shrinkwrap, Default)]
#[shrinkwrap(mutable)]
pub struct BufferStorage(pub NamedSlotMap<BufferHandle, buffer::Buffer>);

#[derive(Shrinkwrap, Default)]
#[shrinkwrap(mutable)]
pub struct TextureStorage(pub NamedSlotMap<TextureHandle, texture::Texture>);

pub struct VulkanResources {
    buffers: BufferStorage,
    textures: TextureStorage,
}

pub struct VulkanDevice {
    pub device: ash::Device,
    pub physical_device: ash::vk::PhysicalDevice,
    pub memory_properties: ash::vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,

    pub surface_capabilities: vk::SurfaceCapabilitiesKHR,
    pub surface_formats: Vec<vk::SurfaceFormatKHR>,

    pub present_queue: ash::vk::Queue,
    pub present_modes: Vec<vk::PresentModeKHR>,

    pub surface_loader: ash::extensions::khr::Surface,
}
impl Deref for VulkanDevice {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}
impl AsRef<ash::Device> for VulkanDevice {
    fn as_ref(&self) -> &ash::Device {
        &self.device
    }
}

pub struct VulkanSurface {
    pub surface: ash::vk::SurfaceKHR,
    pub format: ash::vk::SurfaceFormatKHR,
    pub resolution: ash::vk::Extent2D,
}
impl Deref for VulkanSurface {
    type Target = ash::vk::SurfaceKHR;

    fn deref(&self) -> &Self::Target {
        &self.surface
    }
}
impl AsRef<ash::vk::SurfaceKHR> for VulkanSurface {
    fn as_ref(&self) -> &ash::vk::SurfaceKHR {
        &self.surface
    }
}

pub struct VulkanPresentImages {
    pub images: Vec<ash::vk::Image>,
    pub views: Vec<ash::vk::ImageView>,
}
impl VulkanDestroy for VulkanPresentImages {
    unsafe fn destroy(&mut self, device: &VulkanDevice) {
        self.views
            .drain(..)
            .for_each(|v| device.destroy_image_view(v, None));
        self.images
            .drain(..)
            .for_each(|v| device.destroy_image(v, None));
    }
}

pub struct VulkanDepthImage {
    pub image: ash::vk::Image,
    pub view: ash::vk::ImageView,
    pub memory: ash::vk::DeviceMemory,
}
impl VulkanDestroy for VulkanDepthImage {
    unsafe fn destroy(&mut self, device: &VulkanDevice) {
        device.destroy_image_view(self.view, None);
        device.destroy_image(self.image, None);
        device.free_memory(self.memory, None);
    }
}

pub struct VulkanContext {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub swapchain_loader: ash::extensions::khr::Swapchain,

    pub device: VulkanDevice,

    pub surface: VulkanSurface,

    pub swapchain: ash::vk::SwapchainKHR,

    pub command_buffer_pool: ash::vk::CommandPool,
    pub primary_command_buffers: Vec<ash::vk::CommandBuffer>,
    pub setup_command_buffer: ash::vk::CommandBuffer,

    pub present_images: VulkanPresentImages,
    pub depth_image: VulkanDepthImage,

    pub image_available_semaphores: Vec<ash::vk::Semaphore>,
    pub rendering_complete_semaphores: Vec<ash::vk::Semaphore>,

    pub debug_report_loader: ash::extensions::ext::DebugReport,
    pub debug_call_back: ash::vk::DebugReportCallbackEXT,
}

pub trait VulkanDestroy {
    unsafe fn destroy(&mut self, device: &VulkanDevice);
}
impl VulkanDestroy for Vec<ash::vk::Semaphore> {
    unsafe fn destroy(&mut self, device: &VulkanDevice) {
        self.drain(..)
            .for_each(|s| device.destroy_semaphore(s, None));
    }
}

impl VulkanDestroy for Vec<ash::vk::Framebuffer> {
    unsafe fn destroy(&mut self, device: &VulkanDevice) {
        self.drain(..)
            .for_each(|s| device.destroy_framebuffer(s, None));
    }
}
