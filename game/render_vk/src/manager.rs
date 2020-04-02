use crate::{
    alloc::AllocatorPtr,
    data::{
        texture::{TextureHandle, TexturePtr},
        VulkanContext, VulkanDestroy,
    },
    pass::{RenderArgs, VkPass},
    RenderContext, ScreenDimensions,
};
use ash::{version::DeviceV1_0, vk};
use rl_core::{
    app,
    legion::prelude::*,
    settings::{DisplayMode, Settings},
    winit::{
        self,
        event_loop::EventLoop,
        window::{Window, WindowBuilder},
    },
    GameState, Manager, NamedSlotMap,
};
use std::{sync::Arc, time::Instant};

// TODO: Configurable?
const TARGET_FPS: f32 = 120.0;
const TARGET_DELTA: f32 = 1.0 / TARGET_FPS;

type PassConstructorFn = dyn FnOnce(
    &mut rl_core::GameState,
    &crate::data::VulkanContext,
    &AllocatorPtr,
) -> Result<Box<dyn VkPass>, anyhow::Error>;

pub struct RenderManagerBuilder {
    vk: VulkanContext,
    allocator: AllocatorPtr,
    pass_constructors: Vec<Box<PassConstructorFn>>,
    window: Arc<Window>,
}

impl RenderManagerBuilder {
    pub fn new(
        _app: &mut app::ApplicationContext,
        state: &mut GameState,
        event_loop: &EventLoop<()>,
    ) -> Result<Self, anyhow::Error> {
        //gl::load_with(|symbol| window_context.get_proc_address(symbol) as *const _);

        state
            .resources
            .insert(NamedSlotMap::<TextureHandle, TexturePtr>::default());

        let builder = {
            let settings = state.resources.get::<Settings>().unwrap();

            let mut builder = WindowBuilder::new().with_title(settings.window_title.clone());

            match settings.display_mode {
                DisplayMode::Fullscreen => {
                    unimplemented!(
                        // TODO: https://github.com/rust-windowing/winit/blob/master/examples/fullscreen.rs
                    )
                }
                DisplayMode::Windowed(w, h) => {
                    builder = builder.with_inner_size(winit::dpi::Size::Logical(
                        winit::dpi::LogicalSize::new(f64::from(w), f64::from(h)),
                    ))
                }
            }

            builder
        };

        let window = Arc::new(builder.build(event_loop)?);

        let vk = VulkanContext::new(&*window)?;
        let allocator = Arc::new(vk_mem::Allocator::new(&vk_mem::AllocatorCreateInfo {
            device: vk.device.clone(),
            physical_device: vk.device.physical_device,
            instance: vk.instance.clone(),
            frame_in_use_count: 3,
            ..Default::default()
        })?);

        state.resources.insert(ScreenDimensions {
            size: window.inner_size().to_logical(window.scale_factor()),
            dpi: window.scale_factor(),
        });
        state.resources.insert(window.clone());

        unsafe {
            vk.device
                .device
                .reset_command_buffer(
                    vk.setup_command_buffer,
                    vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                )
                .expect("Reset command buffer failed.");

            vk.device
                .device
                .begin_command_buffer(
                    vk.setup_command_buffer,
                    &vk::CommandBufferBeginInfo::builder()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .expect("Begin commandbuffer");
        }

        Ok(Self {
            vk,
            allocator,
            window,
            pass_constructors: Vec::default(),
        })
    }

    pub fn with_pass<F>(mut self, f: F) -> Self
    where
        F: 'static
            + FnOnce(
                &mut rl_core::GameState,
                &crate::data::VulkanContext,
                &AllocatorPtr,
            ) -> Result<Box<dyn VkPass>, anyhow::Error>,
    {
        self.pass_constructors.push(Box::new(f));
        self
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn build(self, state: &mut rl_core::GameState) -> Result<RenderManager, anyhow::Error> {
        let Self {
            mut pass_constructors,
            vk,
            window,
            allocator,
        } = self;

        let mut passes = pass_constructors
            .drain(..)
            .map(|pass| (pass)(state, &vk, &allocator).unwrap())
            .collect::<Vec<_>>();

        let subpasses = passes.iter().map(|pass| pass.subpass()).collect::<Vec<_>>();

        let subpass_dependencies = passes
            .iter()
            .enumerate()
            .map(|(n, pass)| pass.subpass_dependency(n as u32))
            .collect::<Vec<_>>();

        let (renderpass, framebuffers) =
            unsafe { create_render_pass(&vk, &subpasses, &subpass_dependencies)? };

        let context = RenderContext {
            vk,
            allocator,
            renderpass,
        };

        let passes = passes
            .drain(..)
            .enumerate()
            .map(|(n, mut pass)| unsafe { (pass.rebuild(&context, n as u32).unwrap(), pass) })
            .collect::<Vec<_>>();

        state.resources.insert(context);

        let fences = vec![None, None];

        let res = RenderManager {
            framebuffers,
            last_frame_time: Instant::now(),
            current_frame: 0,
            passes,
            fences,
            window,
            swapchain_invalid: false,
        };
        RenderManager::flush_setup(&state.resources);

        Ok(res)
    }
}

pub struct RenderManager {
    current_frame: usize,
    last_frame_time: Instant,
    passes: Vec<(ash::vk::Pipeline, Box<dyn VkPass>)>,
    swapchain_invalid: bool,
    framebuffers: Vec<ash::vk::Framebuffer>,
    window: Arc<Window>,

    fences: Vec<Option<ash::vk::Fence>>,
}

impl RenderManager {
    #[allow(clippy::cast_possible_truncation)]
    pub unsafe fn recreate_swapchain(
        &mut self,
        context: &mut RenderContext,
    ) -> Result<(), anyhow::Error> {
        let vk = &mut context.vk;

        vk.device
            .device_wait_idle()
            .expect("Failed to wait device idle!");

        self.framebuffers.destroy(&vk.device);

        vk.recreate_swapchain(&*self.window)?;

        self.passes
            .iter()
            .for_each(|(pipeline, _)| vk.device.destroy_pipeline(*pipeline, None));

        let subpasses = self
            .passes
            .iter()
            .map(|(_, pass)| pass.subpass())
            .collect::<Vec<_>>();

        let subpass_dependencies = self
            .passes
            .iter()
            .enumerate()
            .map(|(n, (_, pass))| pass.subpass_dependency(n as u32))
            .collect::<Vec<_>>();

        let (renderpass, framebuffers) =
            create_render_pass(&vk, &subpasses, &subpass_dependencies)?;

        context.renderpass = renderpass;
        self.framebuffers = framebuffers;

        self.passes.iter_mut().enumerate().for_each(|(n, entry)| {
            entry.0 = entry.1.rebuild(&context, n as u32).unwrap();
        });

        self.swapchain_invalid = false;

        Ok(())
    }

    pub fn restart_setup(resources: &Resources) {
        let context = resources.get::<RenderContext>().unwrap();
        let vk = &context.vk;
        unsafe {
            vk.device
                .device
                .begin_command_buffer(
                    vk.setup_command_buffer,
                    &vk::CommandBufferBeginInfo::builder()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .expect("Begin commandbuffer");
        }
    }
    pub fn flush_setup(resources: &Resources) {
        let context = resources.get_mut::<RenderContext>().unwrap();
        let vk = &context.vk;
        unsafe {
            vk.device
                .device
                .end_command_buffer(vk.setup_command_buffer)
                .expect("End commandbuffer");

            let submit_fence = vk
                .device
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .expect("Create fence failed.");

            vk.device
                .device
                .queue_submit(
                    vk.device.present_queue,
                    &[vk::SubmitInfo::builder()
                        .command_buffers(std::slice::from_ref(&vk.setup_command_buffer))
                        .build()],
                    submit_fence,
                )
                .expect("queue submit failed.");

            vk.device
                .device
                .wait_for_fences(&[submit_fence], true, u64::max_value())
                .expect("Wait for fence failed.");

            vk.device.destroy_fence(submit_fence, None);
        }
    }

    #[allow(clippy::too_many_lines)] // TODO:
    unsafe fn draw(
        &mut self,
        app_context: &mut app::ApplicationContext,
        state: &mut GameState,
    ) -> Result<(), anyhow::Error> {
        let mut context = state.resources.get_mut::<RenderContext>().unwrap();

        if self.swapchain_invalid {
            self.recreate_swapchain(&mut context)?;
        }

        if let Some(fence) = self.fences[self.current_frame].take() {
            context
                .vk
                .device
                .wait_for_fences(&[fence], true, u64::max_value())
                .expect("Wait for fence failed.");
            context.vk.device.destroy_fence(fence, None);
        }

        let (present_index, _) = if let Ok(a) = context.vk.swapchain_loader.acquire_next_image(
            context.vk.swapchain,
            u64::max_value(),
            context.vk.image_available_semaphores[self.current_frame],
            vk::Fence::null(),
        ) {
            a
        } else {
            self.swapchain_invalid = true;
            return Ok(());
        };

        context
            .vk
            .device
            .device
            .reset_command_buffer(
                context.vk.primary_command_buffers[self.current_frame],
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
            .expect("Reset command buffer failed.");

        context
            .vk
            .device
            .device
            .begin_command_buffer(
                context.vk.primary_command_buffers[self.current_frame],
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .expect("Begin commandbuffer");

        /* TODO: we cant do this without a new present staging memory barrier with a new attachment
        context.vk.device.device.cmd_clear_color_image(
            context.vk.primary_command_buffers[self.current_frame],
            context.vk.present_images.images[self.current_frame],
            vk::ImageLayout::PRESENT_SRC_KHR,
            &vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
            &[vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                level_count: 1,
                layer_count: 1,
                ..Default::default()
            }],
        );
        */

        context.vk.device.cmd_begin_render_pass(
            context.vk.primary_command_buffers[self.current_frame],
            &vk::RenderPassBeginInfo::builder()
                .render_pass(context.renderpass)
                .framebuffer(self.framebuffers[self.current_frame])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: context.vk.surface.resolution,
                })
                .clear_values(&[
                    vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [0.0, 0.0, 0.0, 1.0],
                        },
                    },
                    vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue {
                            depth: 1.0,
                            stencil: 0,
                        },
                    },
                ]),
            vk::SubpassContents::INLINE,
        );

        let count = self.passes.len();
        let mut secondary_command_buffers = Vec::with_capacity(32);

        for (n, (pipeline, pass)) in self.passes.iter_mut().enumerate() {
            context.vk.device.cmd_bind_pipeline(
                context.vk.primary_command_buffers[self.current_frame],
                vk::PipelineBindPoint::GRAPHICS,
                *pipeline,
            );
            let command_buffer = context.vk.primary_command_buffers[self.current_frame];
            pass.draw(
                RenderArgs {
                    render_context: &mut context,
                    app_context,
                    state,
                    current_frame: self.current_frame,
                    command_buffer,
                },
                &mut secondary_command_buffers,
            )?;

            if !secondary_command_buffers.is_empty() {
                context.vk.device.cmd_execute_commands(
                    context.vk.primary_command_buffers[self.current_frame],
                    &secondary_command_buffers,
                );
                secondary_command_buffers.clear();
            }

            if n + 1 < count {
                context.vk.device.cmd_next_subpass(
                    context.vk.primary_command_buffers[self.current_frame],
                    vk::SubpassContents::INLINE,
                )
            }
        }

        context
            .vk
            .device
            .cmd_end_render_pass(context.vk.primary_command_buffers[self.current_frame]);

        context
            .vk
            .device
            .end_command_buffer(context.vk.primary_command_buffers[self.current_frame])
            .expect("End commandbuffer");

        let submit_fence = context
            .vk
            .device
            .create_fence(&vk::FenceCreateInfo::default(), None)
            .expect("Create fence failed.");

        self.fences[self.current_frame] = Some(submit_fence);

        context
            .vk
            .device
            .queue_submit(
                context.vk.device.present_queue,
                &[vk::SubmitInfo::builder()
                    .wait_semaphores(std::slice::from_ref(
                        &context.vk.image_available_semaphores[self.current_frame],
                    ))
                    .wait_dst_stage_mask(&[vk::PipelineStageFlags::BOTTOM_OF_PIPE])
                    .command_buffers(std::slice::from_ref(
                        &context.vk.primary_command_buffers[self.current_frame],
                    ))
                    .signal_semaphores(std::slice::from_ref(
                        &context.vk.rendering_complete_semaphores[self.current_frame],
                    ))
                    .build()],
                submit_fence,
            )
            .expect("queue submit failed.");

        let present_info = vk::PresentInfoKHR {
            wait_semaphore_count: 1,
            p_wait_semaphores: &context.vk.rendering_complete_semaphores[self.current_frame],
            swapchain_count: 1,
            p_swapchains: &context.vk.swapchain,
            p_image_indices: &present_index,
            ..Default::default()
        };

        if context
            .vk
            .swapchain_loader
            .queue_present(context.vk.device.present_queue, &present_info)
            .is_err()
        {
            self.swapchain_invalid = true;
        }

        Ok(())
    }
}
impl Manager for RenderManager {
    fn destroy(&mut self, _: &mut app::ApplicationContext, state: &mut GameState) {
        let context = state.resources.remove::<RenderContext>().unwrap();
        unsafe {
            context
                .vk
                .device
                .device_wait_idle()
                .expect("Failed to wait device idle!");
        }
    }

    fn tick(
        &mut self,
        app_context: &mut app::ApplicationContext,
        state: &mut GameState,
    ) -> Result<(), anyhow::Error> {
        //if (Instant::now() - self.last_frame_time).as_secs_f32() < TARGET_DELTA {
        //    return Ok(());
        //}

        unsafe {
            self.draw(app_context, state)?;
        }

        self.current_frame = (self.current_frame + 1) % 2;

        Ok(())
    }
}

unsafe fn create_render_pass(
    vk: &VulkanContext,
    subpasses: &[vk::SubpassDescription],
    dependencies: &[vk::SubpassDependency],
) -> Result<(vk::RenderPass, Vec<vk::Framebuffer>), anyhow::Error> {
    let render_pass_attachments = [
        vk::AttachmentDescription {
            format: vk.surface.format.format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            ..Default::default()
        },
        vk::AttachmentDescription {
            format: vk::Format::D16_UNORM,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            ..Default::default()
        },
    ];

    let renderpass_create_info = vk::RenderPassCreateInfo::builder()
        .attachments(&render_pass_attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);

    let renderpass = vk
        .device
        .create_render_pass(&renderpass_create_info, None)?;

    let framebuffers = vk
        .present_images
        .views
        .iter()
        .map(|&present_image_view| {
            let framebuffer_attachments = [present_image_view, vk.depth_image.view];
            vk.device
                .create_framebuffer(
                    &vk::FramebufferCreateInfo::builder()
                        .render_pass(renderpass)
                        .attachments(&framebuffer_attachments)
                        .width(vk.surface.resolution.width)
                        .height(vk.surface.resolution.height)
                        .layers(1)
                        .build(),
                    None,
                )
                .unwrap()
        })
        .collect();

    Ok((renderpass, framebuffers))
}
