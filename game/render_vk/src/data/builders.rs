#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::wildcard_imports,
    clippy::missing_safety_doc,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    clippy::use_self,
    clippy::cast_possible_truncation
)]
use super::*;
const APP_NAME: &str = "RL";

impl VulkanContext {
    #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
    unsafe fn create_surface<E: EntryV1_0, I: InstanceV1_0>(
        entry: &E,
        instance: &I,
        window: &Window,
    ) -> Result<vk::SurfaceKHR, anyhow::Error> {
        use winit::platform::unix::WindowExtUnix;
        let x11_display = window
            .xlib_display()
            .ok_or_else(|| anyhow::anyhow!("Failed to aqcuire xlib display"))?;
        let x11_window = window
            .xlib_window()
            .ok_or_else(|| anyhow::anyhow!("Failed to aqcuire xlib window"))?;
        let x11_create_info = vk::XlibSurfaceCreateInfoKHR::builder()
            .window(x11_window)
            .dpy(x11_display as *mut vk::Display);

        let xlib_surface_loader = XlibSurface::new(entry, instance);
        xlib_surface_loader
            .create_xlib_surface(&x11_create_info, None)
            .map_err(vk::Result::into)
    }

    #[cfg(target_os = "macos")]
    unsafe fn create_surface<E: EntryV1_0, I: InstanceV1_0>(
        entry: &E,
        instance: &I,
        window: &winit::Window,
    ) -> Result<vk::SurfaceKHR, vk::Result> {
        use std::ptr;
        use winit::os::macos::WindowExt;

        let wnd: cocoa_id = mem::transmute(window.get_nswindow());

        let layer = CoreAnimationLayer::new();

        layer.set_edge_antialiasing_mask(0);
        layer.set_presents_with_transaction(false);
        layer.remove_all_animations();

        let view = wnd.contentView();

        layer.set_contents_scale(view.backingScaleFactor());
        view.setLayer(mem::transmute(layer.as_ref()));
        view.setWantsLayer(YES);

        let create_info = vk::MacOSSurfaceCreateInfoMVK {
            s_type: vk::StructureType::MACOS_SURFACE_CREATE_INFO_M,
            p_next: ptr::null(),
            flags: Default::default(),
            p_view: window.get_nsview() as *const c_void,
        };

        let macos_surface_loader = MacOSSurface::new(entry, instance);
        macos_surface_loader.create_mac_os_surface_mvk(&create_info, None)
    }

    #[cfg(target_os = "windows")]
    #[allow(clippy::similar_names)]
    unsafe fn create_surface<E: EntryV1_0, I: InstanceV1_0>(
        entry: &E,
        instance: &I,
        window: &winit::Window,
    ) -> Result<vk::SurfaceKHR, vk::Result> {
        use std::ptr;
        use winapi::shared::windef::HWND;
        use winapi::um::libloaderapi::GetModuleHandleW;
        use winit::os::windows::WindowExt;

        let hwnd = window.get_hwnd() as HWND;
        let hinstance = GetModuleHandleW(ptr::null()) as *const c_void;
        let win32_create_info = vk::Win32SurfaceCreateInfoKHR {
            s_type: vk::StructureType::WIN32_SURFACE_CREATE_INFO_KHR,
            p_next: ptr::null(),
            flags: Default::default(),
            hinstance,
            hwnd: hwnd as *const c_void,
        };
        let win32_surface_loader = Win32Surface::new(entry, instance);
        win32_surface_loader.create_win32_surface(&win32_create_info, None)
    }

    pub unsafe fn create_instance<E: EntryV1_0>(
        entry: &E,
    ) -> Result<<E as EntryV1_0>::Instance, anyhow::Error> {
        let layer_names = [CString::new("VK_LAYER_KHRONOS_validation")?];
        let layers_names_raw: Vec<*const i8> = layer_names
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();

        let extension_names_raw = extension_names();
        let app_name = CString::new(APP_NAME)?;

        let appinfo = vk::ApplicationInfo::builder()
            .application_name(&app_name)
            .application_version(0)
            .engine_name(&app_name)
            .engine_version(0)
            .api_version(vk::make_version(1, 0, 0));

        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&appinfo)
            .enabled_layer_names(&layers_names_raw)
            .enabled_extension_names(&extension_names_raw);

        entry
            .create_instance(&create_info, None)
            .map_err(|e| anyhow::anyhow!("Failed: {:?}", e))
    }

    pub unsafe fn setup_debug<E: EntryV1_0, I: InstanceV1_0>(
        entry: &E,
        instance: &I,
    ) -> Result<
        (
            ash::extensions::ext::DebugReport,
            ash::vk::DebugReportCallbackEXT,
        ),
        anyhow::Error,
    > {
        let debug_info = vk::DebugReportCallbackCreateInfoEXT::builder()
            .flags(
                vk::DebugReportFlagsEXT::ERROR
                    | vk::DebugReportFlagsEXT::WARNING
                    | vk::DebugReportFlagsEXT::PERFORMANCE_WARNING
                    | vk::DebugReportFlagsEXT::DEBUG,
            )
            .pfn_callback(Some(vk_debug_callback));

        let debug_report_loader = DebugReport::new(entry, instance);
        let debug_call_back =
            debug_report_loader.create_debug_report_callback(&debug_info, None)?;

        Ok((debug_report_loader, debug_call_back))
    }

    pub unsafe fn select_device(
        entry: &ash::Entry,
        instance: &ash::Instance,
        surface: vk::SurfaceKHR,
        _device_name: Option<&str>,
    ) -> Result<VulkanDevice, anyhow::Error> {
        let surface_loader = Surface::new(entry, instance);

        let physical_devices = instance
            .enumerate_physical_devices()
            .map_err(|e| anyhow::anyhow!("Failed: {:?}", e))?;

        let (physical_device, queue_family_index) = physical_devices
            .iter()
            .map(|physical_device| {
                instance
                    .get_physical_device_queue_family_properties(*physical_device)
                    .iter()
                    .enumerate()
                    .find_map(|(index, ref info)| {
                        let supports_graphic_and_surface =
                            info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                && surface_loader
                                    .get_physical_device_surface_support(
                                        *physical_device,
                                        index as u32,
                                        surface,
                                    )
                                    .unwrap();
                        if supports_graphic_and_surface {
                            Some((*physical_device, index))
                        } else {
                            None
                        }
                    })
            })
            .find_map(|v| v)
            .ok_or_else(|| anyhow::anyhow!("Failed to find device"))?;

        let queue_family_index = queue_family_index as u32;
        let device_extension_names_raw = [Swapchain::name().as_ptr()];
        let features = vk::PhysicalDeviceFeatures {
            shader_clip_distance: 1,
            ..Default::default()
        };
        let priorities = [1.0];

        let queue_info = [vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family_index)
            .queue_priorities(&priorities)
            .build()];

        let device_create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_info)
            .enabled_extension_names(&device_extension_names_raw)
            .enabled_features(&features);

        let memory_properties = instance.get_physical_device_memory_properties(physical_device);

        let surface_capabilities =
            surface_loader.get_physical_device_surface_capabilities(physical_device, surface)?;

        let surface_formats =
            surface_loader.get_physical_device_surface_formats(physical_device, surface)?;

        let present_modes =
            surface_loader.get_physical_device_surface_present_modes(physical_device, surface)?;

        let device: ash::Device =
            instance.create_device(physical_device, &device_create_info, None)?;
        let present_queue = device.get_device_queue(queue_family_index as u32, 0);

        Ok(VulkanDevice {
            surface_loader,
            device,
            surface_capabilities,
            surface_formats,
            physical_device,
            memory_properties,
            queue_family_index,
            present_queue,
            present_modes,
        })
    }

    pub unsafe fn create_swapchain(
        device: &VulkanDevice,
        surface: &VulkanSurface,
        swapchain_loader: &Swapchain,
    ) -> Result<ash::vk::SwapchainKHR, anyhow::Error> {
        let mut desired_image_count = device.surface_capabilities.min_image_count;
        if device.surface_capabilities.max_image_count > 0
            && desired_image_count > device.surface_capabilities.max_image_count
        {
            desired_image_count = device.surface_capabilities.max_image_count;
        }

        let pre_transform = if device
            .surface_capabilities
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            device.surface_capabilities.current_transform
        };

        let present_mode = device
            .present_modes
            .iter()
            .cloned()
            .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(vk::PresentModeKHR::FIFO);

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(*surface.as_ref())
            .min_image_count(desired_image_count)
            .image_color_space(surface.format.color_space)
            .image_format(surface.format.format)
            .image_extent(surface.resolution)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(pre_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .image_array_layers(1);

        swapchain_loader
            .create_swapchain(&swapchain_create_info, None)
            .map_err(|e| anyhow::anyhow!("Failed: {:?}", e))
    }

    pub unsafe fn select_surface(
        device: &VulkanDevice,
        surface: ash::vk::SurfaceKHR,
        window: &Window,
    ) -> Result<VulkanSurface, anyhow::Error> {
        let format = device
            .surface_formats
            .iter()
            .map(|sfmt| match sfmt.format {
                vk::Format::UNDEFINED => vk::SurfaceFormatKHR {
                    format: vk::Format::B8G8R8_UNORM,
                    color_space: sfmt.color_space,
                },
                _ => *sfmt,
            })
            .next()
            .ok_or_else(|| anyhow::anyhow!("Unable to find suitable surface format."))?;

        let resolution = match device.surface_capabilities.current_extent.width {
            std::u32::MAX => vk::Extent2D {
                width: window.inner_size().width as u32,
                height: window.inner_size().height as u32,
            },
            _ => device.surface_capabilities.current_extent,
        };

        Ok(VulkanSurface {
            surface,
            format,
            resolution,
        })
    }

    pub unsafe fn create_command_buffers(
        device: &VulkanDevice,
        pool: vk::CommandPool,
        count: usize,
    ) -> Result<Vec<vk::CommandBuffer>, anyhow::Error> {
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(count as u32)
            .command_pool(pool)
            .level(vk::CommandBufferLevel::PRIMARY);

        device
            .allocate_command_buffers(&command_buffer_allocate_info)
            .map_err(|e| anyhow::anyhow!("Failed: {:?}", e))
    }

    pub unsafe fn create_image_views(
        device: &VulkanDevice,
        surface: &VulkanSurface,
        swapchain_loader: &Swapchain,
        swapchain: ash::vk::SwapchainKHR,
    ) -> Result<(VulkanPresentImages, VulkanDepthImage), anyhow::Error> {
        let present_images = swapchain_loader.get_swapchain_images(swapchain)?;
        let present_image_views: Vec<vk::ImageView> = present_images
            .iter()
            .map(|&image| {
                let create_view_info = vk::ImageViewCreateInfo::builder()
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(surface.format.format)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::R,
                        g: vk::ComponentSwizzle::G,
                        b: vk::ComponentSwizzle::B,
                        a: vk::ComponentSwizzle::A,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image(image);

                device.create_image_view(&create_view_info, None).unwrap()
            })
            .collect();

        let depth_image_create_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::D16_UNORM)
            .extent(vk::Extent3D {
                width: surface.resolution.width,
                height: surface.resolution.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let depth_image = device.create_image(&depth_image_create_info, None)?;
        let depth_image_memory_req = device.get_image_memory_requirements(depth_image);
        let depth_image_memory_index = find_memorytype_index(
            &depth_image_memory_req,
            &device.memory_properties,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .ok_or_else(|| anyhow::anyhow!("Failed to acquire depth buffer memory type index"))?;

        let depth_image_allocate_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(depth_image_memory_req.size)
            .memory_type_index(depth_image_memory_index);

        let depth_image_memory = device.allocate_memory(&depth_image_allocate_info, None)?;

        device
            .bind_image_memory(depth_image, depth_image_memory, 0)
            .map_err(|e| anyhow::anyhow!("Failed: {:?}", e))?;

        let depth_image_view = device.create_image_view(
            &vk::ImageViewCreateInfo::builder()
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .level_count(1)
                        .layer_count(1)
                        .build(),
                )
                .image(depth_image)
                .format(depth_image_create_info.format)
                .view_type(vk::ImageViewType::TYPE_2D),
            None,
        )?;

        Ok((
            VulkanPresentImages {
                images: present_images,
                views: present_image_views,
            },
            VulkanDepthImage {
                image: depth_image,
                memory: depth_image_memory,
                view: depth_image_view,
            },
        ))
    }

    pub unsafe fn recreate_swapchain(&mut self, window: &Window) -> Result<(), anyhow::Error> {
        let surface_loader = Surface::new(&self.entry, &self.instance);

        self.device.surface_capabilities = surface_loader
            .get_physical_device_surface_capabilities(
                self.device.physical_device,
                self.surface.surface,
            )?;

        // Perform re-creation cleanup
        self.device
            .free_command_buffers(self.command_buffer_pool, &self.primary_command_buffers);

        self.device
            .free_command_buffers(self.command_buffer_pool, &[self.setup_command_buffer]);

        self.present_images.destroy(&self.device);
        self.depth_image.destroy(&self.device);

        //////
        let surface = self.surface.surface;
        self.surface = Self::select_surface(&self.device, surface, window)?;

        self.swapchain =
            Self::create_swapchain(&self.device, &self.surface, &self.swapchain_loader)?;

        let (present_images, depth_image) = Self::create_image_views(
            &self.device,
            &self.surface,
            &self.swapchain_loader,
            self.swapchain,
        )?;
        self.present_images = present_images;
        self.depth_image = depth_image;

        let mut command_buffers =
            Self::create_command_buffers(&self.device, self.command_buffer_pool, 3)?;
        self.setup_command_buffer = command_buffers.remove(0);
        self.primary_command_buffers = command_buffers;

        Ok(())
    }

    pub fn new(window: &Window) -> Result<VulkanContext, anyhow::Error> {
        unsafe {
            let entry = Entry::new()?;

            let instance = Self::create_instance(&entry)?;
            let (debug_report_loader, debug_call_back) = Self::setup_debug(&entry, &instance)?;
            let surface = Self::create_surface(&entry, &instance, &window)?;

            let device = Self::select_device(&entry, &instance, surface, None)?;

            let surface = Self::select_surface(&device, surface, window)?;

            let swapchain_loader = Swapchain::new(&instance, device.as_ref());
            let swapchain = Self::create_swapchain(&device, &surface, &swapchain_loader)?;

            let pool_create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(device.queue_family_index);

            let command_buffer_pool = device.create_command_pool(&pool_create_info, None)?;

            let mut command_buffers =
                Self::create_command_buffers(&device, command_buffer_pool, 3)?;
            let setup_command_buffer = command_buffers.remove(0);
            let primary_command_buffers = command_buffers;

            let (present_images, depth_image) =
                Self::create_image_views(&device, &surface, &swapchain_loader, swapchain)?;

            record_submit_commandbuffer(
                device.as_ref(),
                setup_command_buffer,
                device.present_queue,
                &[],
                &[],
                &[],
                |device, setup_command_buffer| {
                    let layout_transition_barriers = vk::ImageMemoryBarrier::builder()
                        .image(depth_image.image)
                        .dst_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .subresource_range(
                            vk::ImageSubresourceRange::builder()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .layer_count(1)
                                .level_count(1)
                                .build(),
                        );

                    device.cmd_pipeline_barrier(
                        setup_command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barriers.build()],
                    );
                },
            )?;

            let semaphore_create_info = vk::SemaphoreCreateInfo::default();

            let image_available_semaphores = vec![
                device.create_semaphore(&semaphore_create_info, None)?,
                device.create_semaphore(&semaphore_create_info, None)?,
            ];

            let rendering_complete_semaphores = vec![
                device.create_semaphore(&semaphore_create_info, None)?,
                device.create_semaphore(&semaphore_create_info, None)?,
            ];

            Ok(VulkanContext {
                entry,
                instance,
                device,
                swapchain_loader,
                swapchain,
                present_images,
                command_buffer_pool,
                primary_command_buffers,
                setup_command_buffer,
                depth_image,
                image_available_semaphores,
                rendering_complete_semaphores,
                surface,
                debug_call_back,
                debug_report_loader,
            })
        }
    }
}

#[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
fn extension_names() -> Vec<*const i8> {
    println!(
        "{:?}",
        vec![Surface::name(), XlibSurface::name(), DebugReport::name()]
    );

    vec![
        Surface::name().as_ptr(),
        XlibSurface::name().as_ptr(),
        DebugReport::name().as_ptr(),
    ]
}

#[cfg(target_os = "macos")]
fn extension_names() -> Vec<*const i8> {
    vec![
        Surface::name().as_ptr(),
        MacOSSurface::name().as_ptr(),
        DebugReport::name().as_ptr(),
    ]
}

#[cfg(all(windows))]
fn extension_names() -> Vec<*const i8> {
    vec![
        Surface::name().as_ptr(),
        Win32Surface::name().as_ptr(),
        DebugReport::name().as_ptr(),
    ]
}

unsafe extern "system" fn vk_debug_callback(
    _: vk::DebugReportFlagsEXT,
    _: vk::DebugReportObjectTypeEXT,
    _: u64,
    _: usize,
    _: i32,
    _: *const c_char,
    p_message: *const c_char,
    _: *mut c_void,
) -> u32 {
    println!("{:?}", CStr::from_ptr(p_message));
    vk::FALSE
}

pub fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    // Try to find an exactly matching memory flag
    let best_suitable_index =
        find_memorytype_index_f(memory_req, memory_prop, flags, |property_flags, flags| {
            property_flags == flags
        });
    if best_suitable_index.is_some() {
        return best_suitable_index;
    }
    // Otherwise find a memory flag that works
    find_memorytype_index_f(memory_req, memory_prop, flags, |property_flags, flags| {
        property_flags & flags == flags
    })
}

pub fn find_memorytype_index_f<F: Fn(vk::MemoryPropertyFlags, vk::MemoryPropertyFlags) -> bool>(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
    f: F,
) -> Option<u32> {
    let mut memory_type_bits = memory_req.memory_type_bits;
    for (index, ref memory_type) in memory_prop.memory_types.iter().enumerate() {
        if memory_type_bits & 1 == 1 && f(memory_type.property_flags, flags) {
            return Some(index as u32);
        }
        memory_type_bits >>= 1;
    }
    None
}

pub fn record_submit_commandbuffer<D: DeviceV1_0, F: FnOnce(&D, vk::CommandBuffer)>(
    device: &D,
    command_buffer: vk::CommandBuffer,
    submit_queue: vk::Queue,
    wait_mask: &[vk::PipelineStageFlags],
    wait_semaphores: &[vk::Semaphore],
    signal_semaphores: &[vk::Semaphore],
    f: F,
) -> Result<(), anyhow::Error> {
    unsafe {
        device
            .reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
            .expect("Reset command buffer failed.");

        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .expect("Begin commandbuffer");
        f(device, command_buffer);
        device
            .end_command_buffer(command_buffer)
            .expect("End commandbuffer");

        let submit_fence = device
            .create_fence(&vk::FenceCreateInfo::default(), None)
            .expect("Create fence failed.");

        let command_buffers = vec![command_buffer];

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(signal_semaphores);

        device
            .queue_submit(submit_queue, &[submit_info.build()], submit_fence)
            .expect("queue submit failed.");
        device
            .wait_for_fences(&[submit_fence], true, u64::max_value())
            .expect("Wait for fence failed.");
        device.destroy_fence(submit_fence, None);
    }

    Ok(())
}
