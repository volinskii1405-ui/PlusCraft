//! Высокоуровневый рендерер: кадры в полёте, меши чанков, динамическая
//! геометрия, интерфейс. Весь Vulkan-`unsafe` изолирован в этом модуле и
//! подмодулях.

pub mod context;
pub mod pipeline;
pub mod swapchain;
pub mod texture;
pub mod vertex;

use std::collections::HashMap;

use anyhow::{Context, Result};
use ash::vk;
use glam::{DVec3, Mat4, Vec3, Vec4};
use gpu_allocator::MemoryLocation;
use winit::window::Window;

use context::{Buffer, Image, VkContext};
use pipeline::{create_pipeline, Blend, PipelineDesc};
use swapchain::Swapchain;
pub use vertex::{ChunkVertex, EntityVertex, Globals, UiVertex};

use crate::world::chunk::{ChunkPos, CHUNK_H, CHUNK_W};

pub const FRAMES_IN_FLIGHT: usize = 2;

/// Пиксели одного слоя текстуры (RGBA8).
pub struct TextureLayers {
    pub size: u32,
    pub layers: Vec<Vec<u8>>,
}

struct FrameData {
    cmd: vk::CommandBuffer,
    image_available: vk::Semaphore,
    fence: vk::Fence,
    ubo: Buffer,
    descriptor_set: vk::DescriptorSet,
    /// Кольцевой буфер для динамических вершин (мобы, UI, линии).
    dynamic: Buffer,
    /// Ресурсы, которые можно удалить, когда этот кадр завершится на GPU.
    garbage: Vec<Buffer>,
}

struct GpuMesh {
    opaque: Option<(Buffer, u32)>,
    translucent: Option<(Buffer, u32)>,
    min_y: f32,
    max_y: f32,
}

struct PendingUpload {
    staging: Buffer,
    dst: vk::Buffer,
    size: u64,
}

struct Pipelines {
    sky: vk::Pipeline,
    chunk_opaque: vk::Pipeline,
    chunk_translucent: vk::Pipeline,
    entity: vk::Pipeline,
    entity_translucent: vk::Pipeline,
    lines: vk::Pipeline,
    ui: vk::Pipeline,
}

/// Всё, что нужно нарисовать в кадре.
pub struct FrameInput<'a> {
    pub camera_pos: DVec3,
    /// Матрица вида без переноса (камера в начале координат).
    pub view: Mat4,
    pub fov_y: f32,
    pub globals: Globals,
    pub max_distance: f32,
    pub entities: &'a [EntityVertex],
    pub entities_translucent: &'a [EntityVertex],
    pub lines: &'a [EntityVertex],
    /// Геометрия поверх мира (предмет в руке): рисуется после очистки глубины.
    pub overlay: &'a [EntityVertex],
    pub ui: &'a [UiVertex],
    pub draw_world: bool,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct RenderStats {
    pub chunks_total: usize,
    pub chunks_drawn: usize,
    pub quads_drawn: u64,
}

pub struct Renderer {
    // Порядок полей важен только для Drop: всё уничтожаем вручную.
    ctx: VkContext,
    render_pass: vk::RenderPass,
    surface_format: vk::SurfaceFormatKHR,
    depth_format: vk::Format,
    swapchain: Option<Swapchain>,
    command_pool: vk::CommandPool,
    frames: Vec<FrameData>,
    frame_index: usize,
    set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    pipeline_layout: vk::PipelineLayout,
    pipelines: Pipelines,
    block_tex: Option<Image>,
    ui_tex: Option<Image>,
    block_sampler: vk::Sampler,
    ui_sampler: vk::Sampler,
    quad_indices: Option<Buffer>,
    quad_capacity: u32,
    meshes: HashMap<ChunkPos, GpuMesh>,
    pending: Vec<PendingUpload>,
    pending_garbage: Vec<Buffer>,
    window_size: (u32, u32),
    vsync: bool,
    needs_recreate: bool,
    pub stats: RenderStats,
}

impl Renderer {
    pub fn new(
        window: &Window,
        vsync: bool,
        block_textures: &TextureLayers,
        ui_textures: &TextureLayers,
    ) -> Result<Self> {
        let mut ctx = VkContext::new(window)?;
        let size = window.inner_size();
        let window_size = (size.width.max(1), size.height.max(1));

        let surface_format = swapchain::choose_surface_format(&ctx)?;
        let depth_format = ctx.find_depth_format()?;
        let render_pass = swapchain::create_render_pass(&ctx, surface_format.format, depth_format)?;
        let sc = Swapchain::new(
            &mut ctx,
            render_pass,
            surface_format,
            depth_format,
            window_size,
            vsync,
            vk::SwapchainKHR::null(),
        )?;

        // --- Текстуры ---
        let block_tex = texture::create_texture_array(
            &mut ctx,
            block_textures.size,
            block_textures.size,
            &block_textures.layers,
            true,
            vk::Format::R8G8B8A8_SRGB,
            "block textures",
        )?;
        let ui_tex = texture::create_texture_array(
            &mut ctx,
            ui_textures.size,
            ui_textures.size,
            &ui_textures.layers,
            false,
            vk::Format::R8G8B8A8_UNORM,
            "ui atlas",
        )?;
        let block_mips = 32 - block_textures.size.leading_zeros();
        let block_sampler = texture::create_sampler(&ctx, true, block_mips as f32, 4.0)?;
        let ui_sampler = texture::create_sampler(&ctx, false, 0.0, 1.0)?;

        // --- Дескрипторы ---
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        // SAFETY: создание объектов на валидном device; массивы живут до конца вызовов.
        let set_layout = unsafe {
            ctx.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )?
        };
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: FRAMES_IN_FLIGHT as u32,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: FRAMES_IN_FLIGHT as u32 * 2,
            },
        ];
        let descriptor_pool = unsafe {
            ctx.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(FRAMES_IN_FLIGHT as u32)
                    .pool_sizes(&pool_sizes),
                None,
            )?
        };
        let push_ranges = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: 32,
        }];
        let set_layouts = [set_layout];
        let pipeline_layout = unsafe {
            ctx.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&set_layouts)
                    .push_constant_ranges(&push_ranges),
                None,
            )?
        };

        let pipelines = Self::create_pipelines(&ctx, pipeline_layout, render_pass)?;

        // --- Кадры в полёте ---
        let command_pool = unsafe {
            ctx.device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(ctx.graphics_family)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?
        };
        let cmds = unsafe {
            ctx.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(FRAMES_IN_FLIGHT as u32),
            )?
        };
        let layouts = vec![set_layout; FRAMES_IN_FLIGHT];
        let sets = unsafe {
            ctx.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&layouts),
            )?
        };
        let mut frames = Vec::with_capacity(FRAMES_IN_FLIGHT);
        for i in 0..FRAMES_IN_FLIGHT {
            let ubo = ctx.create_buffer(
                std::mem::size_of::<Globals>() as u64,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                MemoryLocation::CpuToGpu,
                "globals ubo",
            )?;
            let dynamic = ctx.create_buffer(
                4 << 20,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                MemoryLocation::CpuToGpu,
                "dynamic vertices",
            )?;
            let image_available =
                unsafe { ctx.device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? };
            let fence = unsafe {
                ctx.device.create_fence(
                    &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                    None,
                )?
            };
            let buffer_info = [vk::DescriptorBufferInfo {
                buffer: ubo.buffer,
                offset: 0,
                range: vk::WHOLE_SIZE,
            }];
            let block_info = [vk::DescriptorImageInfo {
                sampler: block_sampler,
                image_view: block_tex.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }];
            let ui_info = [vk::DescriptorImageInfo {
                sampler: ui_sampler,
                image_view: ui_tex.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }];
            let writes = [
                vk::WriteDescriptorSet::default()
                    .dst_set(sets[i])
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(&buffer_info),
                vk::WriteDescriptorSet::default()
                    .dst_set(sets[i])
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&block_info),
                vk::WriteDescriptorSet::default()
                    .dst_set(sets[i])
                    .dst_binding(2)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&ui_info),
            ];
            unsafe { ctx.device.update_descriptor_sets(&writes, &[]) };
            frames.push(FrameData {
                cmd: cmds[i],
                image_available,
                fence,
                ubo,
                descriptor_set: sets[i],
                dynamic,
                garbage: Vec::new(),
            });
        }

        let mut r = Self {
            ctx,
            render_pass,
            surface_format,
            depth_format,
            swapchain: Some(sc),
            command_pool,
            frames,
            frame_index: 0,
            set_layout,
            descriptor_pool,
            pipeline_layout,
            pipelines,
            block_tex: Some(block_tex),
            ui_tex: Some(ui_tex),
            block_sampler,
            ui_sampler,
            quad_indices: None,
            quad_capacity: 0,
            meshes: HashMap::new(),
            pending: Vec::new(),
            pending_garbage: Vec::new(),
            window_size,
            vsync,
            needs_recreate: false,
            stats: RenderStats::default(),
        };
        r.ensure_quad_indices(1 << 18)?;
        Ok(r)
    }

    pub fn device_name(&self) -> &str {
        &self.ctx.device_name
    }

    fn create_pipelines(
        ctx: &VkContext,
        layout: vk::PipelineLayout,
        rp: vk::RenderPass,
    ) -> Result<Pipelines> {
        let chunk = |translucent: bool| PipelineDesc {
            vert: "chunk.vert",
            frag: "chunk.frag",
            binding: Some(ChunkVertex::binding()),
            attributes: ChunkVertex::attributes(),
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            cull: if translucent { vk::CullModeFlags::NONE } else { vk::CullModeFlags::BACK },
            depth_test: true,
            depth_write: !translucent,
            blend: if translucent { Blend::Alpha } else { Blend::None },
            alpha_cut: if translucent { 0.01 } else { 0.5 },
            depth_bias: false,
        };
        let entity = |translucent: bool, topology, depth_bias| PipelineDesc {
            vert: "entity.vert",
            frag: "entity.frag",
            binding: Some(EntityVertex::binding()),
            attributes: EntityVertex::attributes(),
            topology,
            cull: vk::CullModeFlags::NONE,
            depth_test: true,
            depth_write: !translucent,
            blend: if translucent { Blend::Alpha } else { Blend::None },
            alpha_cut: if translucent { 0.01 } else { 0.5 },
            depth_bias,
        };
        Ok(Pipelines {
            sky: create_pipeline(
                ctx,
                &PipelineDesc {
                    vert: "sky.vert",
                    frag: "sky.frag",
                    binding: None,
                    attributes: vec![],
                    topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                    cull: vk::CullModeFlags::NONE,
                    depth_test: false,
                    depth_write: false,
                    blend: Blend::None,
                    alpha_cut: 0.0,
                    depth_bias: false,
                },
                layout,
                rp,
            )?,
            chunk_opaque: create_pipeline(ctx, &chunk(false), layout, rp)?,
            chunk_translucent: create_pipeline(ctx, &chunk(true), layout, rp)?,
            entity: create_pipeline(
                ctx,
                &entity(false, vk::PrimitiveTopology::TRIANGLE_LIST, false),
                layout,
                rp,
            )?,
            entity_translucent: create_pipeline(
                ctx,
                &entity(true, vk::PrimitiveTopology::TRIANGLE_LIST, true),
                layout,
                rp,
            )?,
            lines: create_pipeline(
                ctx,
                &entity(true, vk::PrimitiveTopology::LINE_LIST, true),
                layout,
                rp,
            )?,
            ui: create_pipeline(
                ctx,
                &PipelineDesc {
                    vert: "ui.vert",
                    frag: "ui.frag",
                    binding: Some(UiVertex::binding()),
                    attributes: UiVertex::attributes(),
                    topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                    cull: vk::CullModeFlags::NONE,
                    depth_test: false,
                    depth_write: false,
                    blend: Blend::Alpha,
                    alpha_cut: 0.0,
                    depth_bias: false,
                },
                layout,
                rp,
            )?,
        })
    }

    /// Общий индексный буфер для квадов: 0,1,2, 2,3,0 со сдвигом 4 на квад.
    fn ensure_quad_indices(&mut self, quads: u32) -> Result<()> {
        if quads <= self.quad_capacity {
            return Ok(());
        }
        let cap = quads.next_power_of_two().max(1 << 16);
        let mut idx: Vec<u32> = Vec::with_capacity(cap as usize * 6);
        for q in 0..cap {
            let b = q * 4;
            idx.extend_from_slice(&[b, b + 1, b + 2, b + 2, b + 3, b]);
        }
        let bytes: &[u8] = bytemuck::cast_slice(&idx);
        let mut staging = self.ctx.create_buffer(
            bytes.len() as u64,
            vk::BufferUsageFlags::TRANSFER_SRC,
            MemoryLocation::CpuToGpu,
            "index staging",
        )?;
        staging.write_bytes(bytes)?;
        let dst = self.ctx.create_buffer(
            bytes.len() as u64,
            vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            MemoryLocation::GpuOnly,
            "quad indices",
        )?;
        let device = self.ctx.device.clone();
        let (src, dstb, size) = (staging.buffer, dst.buffer, bytes.len() as u64);
        // Старый буфер может использоваться кадрами в полёте — ждём простоя.
        self.ctx.wait_idle();
        self.ctx.one_shot(|cmd| unsafe {
            // SAFETY: буферы валидны, размер совпадает.
            device.cmd_copy_buffer(cmd, src, dstb, &[vk::BufferCopy { src_offset: 0, dst_offset: 0, size }]);
        })?;
        self.ctx.destroy_buffer(staging);
        if let Some(old) = self.quad_indices.replace(dst) {
            self.ctx.destroy_buffer(old);
        }
        self.quad_capacity = cap;
        Ok(())
    }

    /// Загружает (или заменяет) меш чанка. Копирование на GPU выполняется в
    /// начале следующего кадра, старые буферы удаляются отложенно.
    pub fn upload_chunk_mesh(
        &mut self,
        pos: ChunkPos,
        opaque: &[ChunkVertex],
        translucent: &[ChunkVertex],
        min_y: f32,
        max_y: f32,
    ) -> Result<()> {
        let max_quads = (opaque.len().max(translucent.len()) / 4) as u32;
        self.ensure_quad_indices(max_quads)?;
        let o = self.make_mesh_buffer(opaque, "chunk opaque")?;
        let t = self.make_mesh_buffer(translucent, "chunk translucent")?;
        let mesh = GpuMesh { opaque: o, translucent: t, min_y, max_y };
        if let Some(old) = self.meshes.insert(pos, mesh) {
            self.retire_mesh(old);
        }
        Ok(())
    }

    fn make_mesh_buffer(&mut self, verts: &[ChunkVertex], name: &str) -> Result<Option<(Buffer, u32)>> {
        if verts.is_empty() {
            return Ok(None);
        }
        let bytes: &[u8] = bytemuck::cast_slice(verts);
        let mut staging = self.ctx.create_buffer(
            bytes.len() as u64,
            vk::BufferUsageFlags::TRANSFER_SRC,
            MemoryLocation::CpuToGpu,
            "mesh staging",
        )?;
        staging.write_bytes(bytes)?;
        let dst = self.ctx.create_buffer(
            bytes.len() as u64,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            MemoryLocation::GpuOnly,
            name,
        )?;
        self.pending.push(PendingUpload { staging, dst: dst.buffer, size: bytes.len() as u64 });
        Ok(Some((dst, (verts.len() / 4) as u32)))
    }

    fn retire_mesh(&mut self, mesh: GpuMesh) {
        if let Some((b, _)) = mesh.opaque {
            self.pending_garbage.push(b);
        }
        if let Some((b, _)) = mesh.translucent {
            self.pending_garbage.push(b);
        }
    }

    pub fn remove_chunk_mesh(&mut self, pos: ChunkPos) {
        if let Some(old) = self.meshes.remove(&pos) {
            self.retire_mesh(old);
        }
    }

    pub fn has_mesh(&self, pos: ChunkPos) -> bool {
        self.meshes.contains_key(&pos)
    }

    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    pub fn clear_meshes(&mut self) {
        let keys: Vec<_> = self.meshes.keys().copied().collect();
        for k in keys {
            self.remove_chunk_mesh(k);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.window_size = (width, height);
        self.needs_recreate = true;
    }

    pub fn set_vsync(&mut self, vsync: bool) {
        if self.vsync != vsync {
            self.vsync = vsync;
            self.needs_recreate = true;
        }
    }

    pub fn extent(&self) -> (u32, u32) {
        self.swapchain
            .as_ref()
            .map(|s| (s.extent.width, s.extent.height))
            .unwrap_or(self.window_size)
    }

    fn recreate_swapchain(&mut self) -> Result<()> {
        if self.window_size.0 == 0 || self.window_size.1 == 0 {
            return Ok(());
        }
        self.ctx.wait_idle();
        let old = self.swapchain.take();
        let old_handle = old.as_ref().map(|s| s.swapchain).unwrap_or_default();
        let mut old = old;
        if let Some(o) = old.as_mut() {
            o.destroy_resources(&mut self.ctx);
        }
        let new = Swapchain::new(
            &mut self.ctx,
            self.render_pass,
            self.surface_format,
            self.depth_format,
            self.window_size,
            self.vsync,
            old_handle,
        );
        if let Some(o) = old {
            o.destroy(&mut self.ctx);
        }
        self.swapchain = Some(new?);
        self.needs_recreate = false;
        Ok(())
    }

    /// Рисует кадр. Возвращает Ok(false), если кадр пропущен (окно свёрнуто).
    pub fn render(&mut self, input: &FrameInput) -> Result<bool> {
        if self.window_size.0 == 0 || self.window_size.1 == 0 {
            return Ok(false);
        }
        if self.needs_recreate || self.swapchain.is_none() {
            self.recreate_swapchain()?;
            if self.swapchain.is_none() {
                return Ok(false);
            }
        }

        let fi = self.frame_index;
        let device = self.ctx.device.clone();
        // SAFETY: ожидание fence этого слота гарантирует, что GPU закончил
        // работу с его командным буфером, UBO и динамическим буфером.
        unsafe {
            device.wait_for_fences(&[self.frames[fi].fence], true, u64::MAX)?;
        }
        // Теперь безопасно удалить мусор этого слота.
        let garbage = std::mem::take(&mut self.frames[fi].garbage);
        for b in garbage {
            self.ctx.destroy_buffer(b);
        }

        let Some(sc) = self.swapchain.as_ref() else { return Ok(false) };
        // SAFETY: swapchain и семафор валидны.
        let acquire = unsafe {
            self.ctx.swapchain_loader.acquire_next_image(
                sc.swapchain,
                u64::MAX,
                self.frames[fi].image_available,
                vk::Fence::null(),
            )
        };
        let image_index = match acquire {
            Ok((i, suboptimal)) => {
                if suboptimal {
                    self.needs_recreate = true;
                }
                i
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.needs_recreate = true;
                return Ok(false);
            }
            Err(e) => return Err(e).context("vkAcquireNextImageKHR"),
        };
        // Сбрасываем fence только когда точно будем сабмитить.
        unsafe { device.reset_fences(&[self.frames[fi].fence])? };

        let extent = sc.extent;
        let framebuffer = sc.framebuffers[image_index as usize];
        let render_finished = sc.render_finished[image_index as usize];
        let swapchain_handle = sc.swapchain;

        // --- Матрицы ---
        let aspect = extent.width as f32 / extent.height.max(1) as f32;
        let mut proj = Mat4::perspective_infinite_reverse_rh(input.fov_y, aspect, 0.05);
        proj.y_axis.y *= -1.0; // Vulkan: ось Y экрана направлена вниз
        let view_proj = proj * input.view;
        let mut globals = input.globals;
        globals.view_proj = view_proj.to_cols_array_2d();
        globals.inv_view_proj = view_proj.inverse().to_cols_array_2d();
        self.frames[fi].ubo.write_bytes(bytemuck::bytes_of(&globals))?;

        // --- Динамические вершины: entities | translucent | lines | ui | overlay ---
        let parts: [&[u8]; 5] = [
            bytemuck::cast_slice(input.entities),
            bytemuck::cast_slice(input.entities_translucent),
            bytemuck::cast_slice(input.lines),
            bytemuck::cast_slice(input.ui),
            bytemuck::cast_slice(input.overlay),
        ];
        let total: usize = parts.iter().map(|p| p.len()).sum();
        if total as u64 > self.frames[fi].dynamic.size {
            let new_size = (total as u64).next_power_of_two();
            let nb = self.ctx.create_buffer(
                new_size,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                MemoryLocation::CpuToGpu,
                "dynamic vertices",
            )?;
            // Старый буфер этого слота GPU уже не использует (fence дождались).
            let old = std::mem::replace(&mut self.frames[fi].dynamic, nb);
            self.ctx.destroy_buffer(old);
        }
        let mut offsets = [0u64; 5];
        {
            let dst = self.frames[fi]
                .dynamic
                .mapped_mut()
                .context("динамический буфер не отображён")?;
            let mut off = 0usize;
            for (i, p) in parts.iter().enumerate() {
                offsets[i] = off as u64;
                dst[off..off + p.len()].copy_from_slice(p);
                off += p.len();
            }
        }

        // --- Запись команд ---
        let cmd = self.frames[fi].cmd;
        let set = self.frames[fi].descriptor_set;
        let dyn_buf = self.frames[fi].dynamic.buffer;
        let quad_ib = self.quad_indices.as_ref().map(|b| b.buffer).unwrap_or_default();

        // Ожидающие загрузки мешей: их staging-буферы отправляем в мусор этого кадра.
        let pending = std::mem::take(&mut self.pending);
        let mut stats = RenderStats { chunks_total: self.meshes.len(), ..Default::default() };

        // SAFETY: командный буфер не используется GPU (fence дождались),
        // все хэндлы валидны на время записи и выполнения кадра.
        unsafe {
            device.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;
            device.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;

            if !pending.is_empty() {
                for p in &pending {
                    device.cmd_copy_buffer(
                        cmd,
                        p.staging.buffer,
                        p.dst,
                        &[vk::BufferCopy { src_offset: 0, dst_offset: 0, size: p.size }],
                    );
                }
                let barrier = vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::VERTEX_ATTRIBUTE_READ);
                device.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::VERTEX_INPUT,
                    vk::DependencyFlags::empty(),
                    &[barrier],
                    &[],
                    &[],
                );
            }

            let clear = [
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] } },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue { depth: 0.0, stencil: 0 },
                },
            ];
            let rp_begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass)
                .framebuffer(framebuffer)
                .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent })
                .clear_values(&clear);
            device.cmd_begin_render_pass(cmd, &rp_begin, vk::SubpassContents::INLINE);
            device.cmd_set_viewport(
                cmd,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: extent.width as f32,
                    height: extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            device.cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent }]);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[set],
                &[],
            );

            let layout = self.pipeline_layout;
            let push = |cmd: vk::CommandBuffer, data: [f32; 8]| {
                device.cmd_push_constants(
                    cmd,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(&data),
                );
            };

            if input.draw_world {
                // 1. Небо.
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.sky);
                device.cmd_draw(cmd, 3, 1, 0, 0);

                // 2. Непрозрачные чанки (с отсечением по пирамиде видимости).
                let frustum = Frustum::from_matrix(view_proj);
                let cam = input.camera_pos;
                let mut visible: Vec<(f32, ChunkPos)> = Vec::new();
                for (pos, mesh) in &self.meshes {
                    let ox = (pos.x as f64 * CHUNK_W as f64 - cam.x) as f32;
                    let oz = (pos.z as f64 * CHUNK_W as f64 - cam.z) as f32;
                    let min = Vec3::new(ox, mesh.min_y - cam.y as f32, oz);
                    let max = Vec3::new(ox + CHUNK_W as f32, mesh.max_y - cam.y as f32, oz + CHUNK_W as f32);
                    let cx = ox + 8.0;
                    let cz = oz + 8.0;
                    let dist = (cx * cx + cz * cz).sqrt();
                    if dist > input.max_distance + 24.0 {
                        continue;
                    }
                    if !frustum.intersects_aabb(min, max) {
                        continue;
                    }
                    visible.push((dist, *pos));
                }
                // Спереди назад — лучше работает ранний тест глубины.
                visible.sort_by(|a, b| a.0.total_cmp(&b.0));
                stats.chunks_drawn = visible.len();

                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.chunk_opaque);
                device.cmd_bind_index_buffer(cmd, quad_ib, 0, vk::IndexType::UINT32);
                for (_, pos) in &visible {
                    let Some(mesh) = self.meshes.get(pos) else { continue };
                    if let Some((buf, quads)) = &mesh.opaque {
                        let o = chunk_origin(*pos, cam);
                        push(cmd, [o.x, o.y, o.z, 0.0, 0.0, 0.0, 0.0, 0.0]);
                        device.cmd_bind_vertex_buffers(cmd, 0, &[buf.buffer], &[0]);
                        device.cmd_draw_indexed(cmd, quads * 6, 1, 0, 0, 0);
                        stats.quads_drawn += *quads as u64;
                    }
                }

                // 3. Непрозрачные сущности.
                if !input.entities.is_empty() {
                    device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.entity);
                    device.cmd_bind_vertex_buffers(cmd, 0, &[dyn_buf], &[offsets[0]]);
                    device.cmd_draw(cmd, input.entities.len() as u32, 1, 0, 0);
                }

                // 4. Полупрозрачные чанки — сзади наперёд.
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.chunk_translucent);
                device.cmd_bind_index_buffer(cmd, quad_ib, 0, vk::IndexType::UINT32);
                for (_, pos) in visible.iter().rev() {
                    let Some(mesh) = self.meshes.get(pos) else { continue };
                    if let Some((buf, quads)) = &mesh.translucent {
                        let o = chunk_origin(*pos, cam);
                        push(cmd, [o.x, o.y, o.z, 0.0, 0.0, 0.0, 0.0, 0.0]);
                        device.cmd_bind_vertex_buffers(cmd, 0, &[buf.buffer], &[0]);
                        device.cmd_draw_indexed(cmd, quads * 6, 1, 0, 0, 0);
                        stats.quads_drawn += *quads as u64;
                    }
                }

                // 5. Полупрозрачные сущности (трещины, частицы).
                if !input.entities_translucent.is_empty() {
                    device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.entity_translucent);
                    device.cmd_bind_vertex_buffers(cmd, 0, &[dyn_buf], &[offsets[1]]);
                    device.cmd_draw(cmd, input.entities_translucent.len() as u32, 1, 0, 0);
                }

                // 6. Линии (рамка выбранного блока).
                if !input.lines.is_empty() {
                    device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.lines);
                    device.cmd_bind_vertex_buffers(cmd, 0, &[dyn_buf], &[offsets[2]]);
                    device.cmd_draw(cmd, input.lines.len() as u32, 1, 0, 0);
                }

                // 7. Предмет в руке: очищаем глубину, чтобы он не уходил в стены.
                if !input.overlay.is_empty() {
                    let clear = vk::ClearAttachment {
                        aspect_mask: vk::ImageAspectFlags::DEPTH,
                        color_attachment: 0,
                        clear_value: vk::ClearValue {
                            depth_stencil: vk::ClearDepthStencilValue { depth: 0.0, stencil: 0 },
                        },
                    };
                    let rect = vk::ClearRect {
                        rect: vk::Rect2D { offset: vk::Offset2D::default(), extent },
                        base_array_layer: 0,
                        layer_count: 1,
                    };
                    device.cmd_clear_attachments(cmd, &[clear], &[rect]);
                    device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.entity);
                    device.cmd_bind_vertex_buffers(cmd, 0, &[dyn_buf], &[offsets[4]]);
                    device.cmd_draw(cmd, input.overlay.len() as u32, 1, 0, 0);
                }
            }

            // 8. Интерфейс.
            if !input.ui.is_empty() {
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.ui);
                push(
                    cmd,
                    [extent.width as f32, extent.height as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                );
                device.cmd_bind_vertex_buffers(cmd, 0, &[dyn_buf], &[offsets[3]]);
                device.cmd_draw(cmd, input.ui.len() as u32, 1, 0, 0);
            }

            device.cmd_end_render_pass(cmd);
            device.end_command_buffer(cmd)?;

            let wait = [self.frames[fi].image_available];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let signal = [render_finished];
            let cmds = [cmd];
            let submit = vk::SubmitInfo::default()
                .wait_semaphores(&wait)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&cmds)
                .signal_semaphores(&signal);
            device
                .queue_submit(self.ctx.graphics_queue, &[submit], self.frames[fi].fence)
                .context("vkQueueSubmit")?;

            let swapchains = [swapchain_handle];
            let indices = [image_index];
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal)
                .swapchains(&swapchains)
                .image_indices(&indices);
            match self.ctx.swapchain_loader.queue_present(self.ctx.present_queue, &present) {
                Ok(suboptimal) => {
                    if suboptimal {
                        self.needs_recreate = true;
                    }
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.needs_recreate = true,
                Err(e) => return Err(e).context("vkQueuePresentKHR"),
            }
        }

        // Staging-буферы и заменённые меши удалим, когда этот кадр завершится.
        for p in pending {
            self.frames[fi].garbage.push(p.staging);
        }
        let retired = std::mem::take(&mut self.pending_garbage);
        self.frames[fi].garbage.extend(retired);

        self.stats = stats;
        self.frame_index = (fi + 1) % FRAMES_IN_FLIGHT;
        Ok(true)
    }
}

fn chunk_origin(pos: ChunkPos, cam: DVec3) -> Vec3 {
    Vec3::new(
        (pos.x as f64 * CHUNK_W as f64 - cam.x) as f32,
        (-cam.y) as f32,
        (pos.z as f64 * CHUNK_W as f64 - cam.z) as f32,
    )
}

/// Пирамида видимости (плоскости извлекаются из матрицы view-proj).
pub struct Frustum {
    planes: [Vec4; 5],
}

impl Frustum {
    pub fn from_matrix(m: Mat4) -> Self {
        let r0 = m.row(0);
        let r1 = m.row(1);
        let r2 = m.row(2);
        let r3 = m.row(3);
        // Бесконечная дальняя плоскость при обратном Z — её не проверяем.
        let mut planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 - r2];
        for p in &mut planes {
            let len = p.truncate().length();
            if len > 0.0 {
                *p /= len;
            }
        }
        Self { planes }
    }

    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for p in &self.planes {
            let n = p.truncate();
            // «Положительная» вершина бокса относительно нормали плоскости.
            let v = Vec3::new(
                if n.x >= 0.0 { max.x } else { min.x },
                if n.y >= 0.0 { max.y } else { min.y },
                if n.z >= 0.0 { max.z } else { min.z },
            );
            if n.dot(v) + p.w < 0.0 {
                return false;
            }
        }
        true
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.ctx.wait_idle();
        let meshes: Vec<_> = self.meshes.drain().map(|(_, m)| m).collect();
        for m in meshes {
            self.retire_mesh(m);
        }
        let mut all: Vec<Buffer> = std::mem::take(&mut self.pending_garbage);
        for p in std::mem::take(&mut self.pending) {
            all.push(p.staging);
        }
        let frames = std::mem::take(&mut self.frames);
        // SAFETY: GPU простаивает (wait_idle выше); уничтожаем всё созданное.
        unsafe {
            for f in &frames {
                self.ctx.device.destroy_semaphore(f.image_available, None);
                self.ctx.device.destroy_fence(f.fence, None);
            }
        }
        for f in frames {
            all.push(f.ubo);
            all.push(f.dynamic);
            all.extend(f.garbage);
        }
        if let Some(b) = self.quad_indices.take() {
            all.push(b);
        }
        for b in all {
            self.ctx.destroy_buffer(b);
        }
        if let Some(i) = self.block_tex.take() {
            self.ctx.destroy_image(i);
        }
        if let Some(i) = self.ui_tex.take() {
            self.ctx.destroy_image(i);
        }
        if let Some(sc) = self.swapchain.take() {
            sc.destroy(&mut self.ctx);
        }
        unsafe {
            let d = &self.ctx.device;
            for p in [
                self.pipelines.sky,
                self.pipelines.chunk_opaque,
                self.pipelines.chunk_translucent,
                self.pipelines.entity,
                self.pipelines.entity_translucent,
                self.pipelines.lines,
                self.pipelines.ui,
            ] {
                d.destroy_pipeline(p, None);
            }
            d.destroy_pipeline_layout(self.pipeline_layout, None);
            d.destroy_descriptor_pool(self.descriptor_pool, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_sampler(self.block_sampler, None);
            d.destroy_sampler(self.ui_sampler, None);
            d.destroy_command_pool(self.command_pool, None);
            d.destroy_render_pass(self.render_pass, None);
        }
        // ctx уничтожится своим Drop после этого.
    }
}

#[allow(dead_code)]
const _: () = assert!(CHUNK_H <= 256);
