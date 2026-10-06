//! Swapchain, буфер глубины, render pass и framebuffer'ы.
//! Пересоздаётся целиком при изменении размера окна или смене VSync.

use anyhow::{Context, Result};
use ash::vk;

use super::context::{Image, VkContext};

pub struct Swapchain {
    pub swapchain: vk::SwapchainKHR,
    pub format: vk::SurfaceFormatKHR,
    pub extent: vk::Extent2D,
    pub images: Vec<vk::Image>,
    pub views: Vec<vk::ImageView>,
    pub depth: Option<Image>,
    pub framebuffers: Vec<vk::Framebuffer>,
    /// Семафоры «рендер закончен» — по одному на изображение swapchain, чтобы
    /// не переиспользовать семафор, который ещё ждёт presentation engine.
    pub render_finished: Vec<vk::Semaphore>,
    pub present_mode: vk::PresentModeKHR,
}

pub fn choose_surface_format(ctx: &VkContext) -> Result<vk::SurfaceFormatKHR> {
    // SAFETY: запрос свойств поверхности.
    let formats = unsafe {
        ctx.surface_loader
            .get_physical_device_surface_formats(ctx.physical_device, ctx.surface)?
    };
    let preferred = [vk::Format::B8G8R8A8_SRGB, vk::Format::R8G8B8A8_SRGB];
    for p in preferred {
        if let Some(f) = formats
            .iter()
            .find(|f| f.format == p && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
        {
            return Ok(*f);
        }
    }
    formats
        .first()
        .copied()
        .context("поверхность не поддерживает ни одного формата")
}

pub fn create_render_pass(
    ctx: &VkContext,
    color_format: vk::Format,
    depth_format: vk::Format,
) -> Result<vk::RenderPass> {
    let attachments = [
        vk::AttachmentDescription::default()
            .format(color_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR),
        vk::AttachmentDescription::default()
            .format(depth_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
    ];
    let color_ref = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let depth_ref = vk::AttachmentReference {
        attachment: 1,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_ref)
        .depth_stencil_attachment(&depth_ref)];
    let deps = [vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
        )
        .dst_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
        )];
    let info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&deps);
    // SAFETY: все массивы живут до конца вызова.
    Ok(unsafe { ctx.device.create_render_pass(&info, None)? })
}

impl Swapchain {
    pub fn new(
        ctx: &mut VkContext,
        render_pass: vk::RenderPass,
        format: vk::SurfaceFormatKHR,
        depth_format: vk::Format,
        window_size: (u32, u32),
        vsync: bool,
        old: vk::SwapchainKHR,
    ) -> Result<Self> {
        // SAFETY: запросы свойств и создание объектов на валидном device.
        let caps = unsafe {
            ctx.surface_loader
                .get_physical_device_surface_capabilities(ctx.physical_device, ctx.surface)?
        };
        let modes = unsafe {
            ctx.surface_loader
                .get_physical_device_surface_present_modes(ctx.physical_device, ctx.surface)?
        };
        let present_mode = if vsync {
            vk::PresentModeKHR::FIFO
        } else if modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else if modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
            vk::PresentModeKHR::IMMEDIATE
        } else {
            vk::PresentModeKHR::FIFO
        };

        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: window_size
                    .0
                    .clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: window_size
                    .1
                    .clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        };

        let mut image_count = caps.min_image_count + 1;
        if caps.max_image_count > 0 {
            image_count = image_count.min(caps.max_image_count);
        }

        let families = [ctx.graphics_family, ctx.present_family];
        let mut info = vk::SwapchainCreateInfoKHR::default()
            .surface(ctx.surface)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .pre_transform(caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(old);
        if ctx.graphics_family != ctx.present_family {
            info = info
                .image_sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&families);
        } else {
            info = info.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
        }
        if !caps
            .supported_composite_alpha
            .contains(vk::CompositeAlphaFlagsKHR::OPAQUE)
        {
            info = info.composite_alpha(vk::CompositeAlphaFlagsKHR::INHERIT);
        }

        let swapchain = unsafe { ctx.swapchain_loader.create_swapchain(&info, None) }
            .context("vkCreateSwapchainKHR")?;
        let images = unsafe { ctx.swapchain_loader.get_swapchain_images(swapchain)? };
        let mut views = Vec::with_capacity(images.len());
        for &img in &images {
            let view_info = vk::ImageViewCreateInfo::default()
                .image(img)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format.format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            views.push(unsafe { ctx.device.create_image_view(&view_info, None)? });
        }

        let depth = ctx.create_image(
            extent,
            1,
            1,
            depth_format,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageAspectFlags::DEPTH,
            vk::ImageViewType::TYPE_2D,
            "depth",
        )?;

        let mut framebuffers = Vec::with_capacity(views.len());
        for &v in &views {
            let attachments = [v, depth.view];
            let fb_info = vk::FramebufferCreateInfo::default()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(extent.width)
                .height(extent.height)
                .layers(1);
            framebuffers.push(unsafe { ctx.device.create_framebuffer(&fb_info, None)? });
        }

        let mut render_finished = Vec::with_capacity(images.len());
        for _ in 0..images.len() {
            render_finished.push(unsafe {
                ctx.device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?
            });
        }

        log::info!(
            "Swapchain {}x{}, {} изобр., режим {:?}",
            extent.width,
            extent.height,
            images.len(),
            present_mode
        );

        Ok(Self {
            swapchain,
            format,
            extent,
            images,
            views,
            depth: Some(depth),
            framebuffers,
            render_finished,
            present_mode,
        })
    }

    /// Уничтожает всё, кроме самого VkSwapchainKHR (его можно передать как old).
    pub fn destroy_resources(&mut self, ctx: &mut VkContext) {
        // SAFETY: вызывается после device_wait_idle.
        unsafe {
            for &fb in &self.framebuffers {
                ctx.device.destroy_framebuffer(fb, None);
            }
            for &v in &self.views {
                ctx.device.destroy_image_view(v, None);
            }
            for &s in &self.render_finished {
                ctx.device.destroy_semaphore(s, None);
            }
        }
        self.framebuffers.clear();
        self.views.clear();
        self.render_finished.clear();
        if let Some(d) = self.depth.take() {
            ctx.destroy_image(d);
        }
    }

    pub fn destroy(mut self, ctx: &mut VkContext) {
        self.destroy_resources(ctx);
        // SAFETY: после wait_idle.
        unsafe { ctx.swapchain_loader.destroy_swapchain(self.swapchain, None) };
    }
}
