//! Низкоуровневый Vulkan-контекст: instance, surface, физическое/логическое
//! устройство, очереди и аллокатор памяти.
//!
//! Весь `unsafe` здесь — это вызовы Vulkan API через `ash`. Инварианты, которые
//! мы соблюдаем:
//! * объекты уничтожаются строго в обратном порядке создания (см. `Drop`);
//! * все хэндлы, передаваемые в функции, созданы этим же instance/device;
//! * указатели на строки/структуры живут дольше вызова (локальные переменные).

use std::ffi::{c_char, CStr, CString};

use anyhow::{anyhow, bail, Context, Result};
use ash::vk;
use gpu_allocator::vulkan::{
    Allocation, AllocationCreateDesc, AllocationScheme, Allocator, AllocatorCreateDesc,
};
use gpu_allocator::MemoryLocation;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

/// Валидационные слои включаем только в debug-сборке.
pub const ENABLE_VALIDATION: bool = cfg!(debug_assertions);

const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";

pub struct VkContext {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    debug: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
    pub surface_loader: ash::khr::surface::Instance,
    pub surface: vk::SurfaceKHR,
    pub physical_device: vk::PhysicalDevice,
    pub device_name: String,
    pub limits: vk::PhysicalDeviceLimits,
    pub device: ash::Device,
    pub swapchain_loader: ash::khr::swapchain::Device,
    pub graphics_family: u32,
    pub present_family: u32,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    /// Пул для одноразовых команд загрузки (текстуры при старте).
    pub upload_pool: vk::CommandPool,
    allocator: Option<Allocator>,
}

/// Буфер вместе с его памятью.
pub struct Buffer {
    pub buffer: vk::Buffer,
    pub allocation: Option<Allocation>,
    pub size: u64,
}

impl Buffer {
    /// Отображённая в память CPU область (только для CpuToGpu-буферов).
    pub fn mapped_mut(&mut self) -> Option<&mut [u8]> {
        self.allocation.as_mut().and_then(|a| a.mapped_slice_mut())
    }

    /// Копирует байты в начало отображённого буфера.
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<()> {
        let slice = self
            .mapped_mut()
            .ok_or_else(|| anyhow!("буфер не отображён в память CPU"))?;
        if data.len() > slice.len() {
            bail!("запись {} байт в буфер размером {}", data.len(), slice.len());
        }
        slice[..data.len()].copy_from_slice(data);
        Ok(())
    }
}

/// Изображение вместе с памятью и view.
pub struct Image {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub allocation: Option<Allocation>,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
}

unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _ty: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user: *mut std::ffi::c_void,
) -> vk::Bool32 {
    // SAFETY: валидационный слой гарантирует корректность указателя на время вызова.
    let msg = unsafe {
        if data.is_null() || (*data).p_message.is_null() {
            "<пусто>".to_string()
        } else {
            CStr::from_ptr((*data).p_message).to_string_lossy().into_owned()
        }
    };
    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        log::error!("[vulkan] {msg}");
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        log::warn!("[vulkan] {msg}");
    } else {
        log::debug!("[vulkan] {msg}");
    }
    vk::FALSE
}

impl VkContext {
    pub fn new(window: &Window) -> Result<Self> {
        // SAFETY: загрузка libvulkan.so.1 через dlopen; ошибка обрабатывается.
        let entry = unsafe { ash::Entry::load() }
            .context("не удалось загрузить Vulkan (libvulkan.so.1). Установлен ли драйвер Vulkan?")?;

        let display_handle = window.display_handle()?.as_raw();
        let window_handle = window.window_handle()?.as_raw();

        // --- Instance ---
        let app_name = CString::new("PlusCraft")?;
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 0, 1, 0))
            .engine_name(&app_name)
            .engine_version(vk::make_api_version(0, 0, 1, 0))
            .api_version(vk::API_VERSION_1_1);

        let mut extensions: Vec<*const c_char> =
            ash_window::enumerate_required_extensions(display_handle)?.to_vec();

        let mut layers: Vec<*const c_char> = Vec::new();
        let mut validation = false;
        if ENABLE_VALIDATION {
            // SAFETY: простой запрос свойств.
            let available = unsafe { entry.enumerate_instance_layer_properties()? };
            let has = available.iter().any(|l| {
                l.layer_name_as_c_str().map(|n| n == VALIDATION_LAYER).unwrap_or(false)
            });
            if has {
                layers.push(VALIDATION_LAYER.as_ptr());
                extensions.push(ash::ext::debug_utils::NAME.as_ptr());
                validation = true;
                log::info!("Валидационные слои Vulkan включены");
            } else {
                log::warn!("VK_LAYER_KHRONOS_validation недоступен — валидация выключена");
            }
        }

        let instance_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layers);
        // SAFETY: все указатели в instance_info живут до конца вызова.
        let instance = unsafe { entry.create_instance(&instance_info, None) }
            .context("vkCreateInstance")?;

        let debug = if validation {
            let loader = ash::ext::debug_utils::Instance::new(&entry, &instance);
            let info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(debug_callback));
            // SAFETY: callback — статическая функция с правильной сигнатурой.
            let messenger = unsafe { loader.create_debug_utils_messenger(&info, None)? };
            Some((loader, messenger))
        } else {
            None
        };

        // --- Surface ---
        let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);
        // SAFETY: хэндлы окна валидны, пока жив `window`; surface уничтожается
        // в Drop раньше, чем окно (Renderer хранится в App до окна).
        let surface = unsafe {
            ash_window::create_surface(&entry, &instance, display_handle, window_handle, None)
        }
        .context("не удалось создать Vulkan surface")?;

        // --- Физическое устройство ---
        // SAFETY: запросы свойств у валидного instance.
        let physical_devices = unsafe { instance.enumerate_physical_devices()? };
        if physical_devices.is_empty() {
            bail!("не найдено ни одного устройства с поддержкой Vulkan");
        }

        let mut best: Option<(i32, vk::PhysicalDevice, u32, u32)> = None;
        for &pd in &physical_devices {
            let props = unsafe { instance.get_physical_device_properties(pd) };
            if props.api_version < vk::API_VERSION_1_1 {
                continue;
            }
            let exts = unsafe { instance.enumerate_device_extension_properties(pd)? };
            let has_swapchain = exts.iter().any(|e| {
                e.extension_name_as_c_str().map(|n| n == ash::khr::swapchain::NAME).unwrap_or(false)
            });
            if !has_swapchain {
                continue;
            }
            let families = unsafe { instance.get_physical_device_queue_family_properties(pd) };
            let mut graphics = None;
            let mut present = None;
            for (i, f) in families.iter().enumerate() {
                let i = i as u32;
                let supports_present = unsafe {
                    surface_loader.get_physical_device_surface_support(pd, i, surface)?
                };
                if f.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                    if graphics.is_none() {
                        graphics = Some(i);
                    }
                    // Предпочитаем одно семейство для графики и показа.
                    if supports_present {
                        graphics = Some(i);
                        present = Some(i);
                        break;
                    }
                }
                if supports_present && present.is_none() {
                    present = Some(i);
                }
            }
            let (Some(g), Some(p)) = (graphics, present) else { continue };
            let score = match props.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 1000,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 500,
                vk::PhysicalDeviceType::VIRTUAL_GPU => 200,
                vk::PhysicalDeviceType::CPU => 50,
                _ => 10,
            };
            if best.map(|b| score > b.0).unwrap_or(true) {
                best = Some((score, pd, g, p));
            }
        }
        let (_, physical_device, graphics_family, present_family) =
            best.ok_or_else(|| anyhow!("нет подходящего GPU (нужен Vulkan 1.1 + swapchain)"))?;

        let props = unsafe { instance.get_physical_device_properties(physical_device) };
        let device_name = props
            .device_name_as_c_str()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "?".into());
        log::info!(
            "GPU: {device_name} (Vulkan {}.{}.{})",
            vk::api_version_major(props.api_version),
            vk::api_version_minor(props.api_version),
            vk::api_version_patch(props.api_version)
        );

        // --- Логическое устройство ---
        let priorities = [1.0f32];
        let mut queue_infos = vec![vk::DeviceQueueCreateInfo::default()
            .queue_family_index(graphics_family)
            .queue_priorities(&priorities)];
        if present_family != graphics_family {
            queue_infos.push(
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(present_family)
                    .queue_priorities(&priorities),
            );
        }
        let supported = unsafe { instance.get_physical_device_features(physical_device) };
        let features = vk::PhysicalDeviceFeatures::default()
            .sampler_anisotropy(supported.sampler_anisotropy == vk::TRUE)
            .fill_mode_non_solid(supported.fill_mode_non_solid == vk::TRUE);
        let device_exts = [ash::khr::swapchain::NAME.as_ptr()];
        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&device_exts)
            .enabled_features(&features);
        // SAFETY: параметры валидны, расширение swapchain проверено выше.
        let device = unsafe { instance.create_device(physical_device, &device_info, None) }
            .context("vkCreateDevice")?;
        let graphics_queue = unsafe { device.get_device_queue(graphics_family, 0) };
        let present_queue = unsafe { device.get_device_queue(present_family, 0) };
        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);

        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device,
            debug_settings: Default::default(),
            buffer_device_address: false,
            allocation_sizes: Default::default(),
        })
        .context("инициализация gpu-allocator")?;

        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(graphics_family)
            .flags(vk::CommandPoolCreateFlags::TRANSIENT | vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let upload_pool = unsafe { device.create_command_pool(&pool_info, None)? };

        let mut limits = props.limits;
        if supported.sampler_anisotropy != vk::TRUE {
            limits.max_sampler_anisotropy = 1.0;
        }

        Ok(Self {
            entry,
            instance,
            debug,
            surface_loader,
            surface,
            physical_device,
            device_name,
            limits,
            device,
            swapchain_loader,
            graphics_family,
            present_family,
            graphics_queue,
            present_queue,
            upload_pool,
            allocator: Some(allocator),
        })
    }

    fn allocator(&mut self) -> Result<&mut Allocator> {
        self.allocator.as_mut().ok_or_else(|| anyhow!("аллокатор уже уничтожен"))
    }

    pub fn create_buffer(
        &mut self,
        size: u64,
        usage: vk::BufferUsageFlags,
        location: MemoryLocation,
        name: &str,
    ) -> Result<Buffer> {
        let size = size.max(16);
        let info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        // SAFETY: валидный device и info.
        let buffer = unsafe { self.device.create_buffer(&info, None)? };
        let requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        let allocation = match self.allocator()?.allocate(&AllocationCreateDesc {
            name,
            requirements,
            location,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        }) {
            Ok(a) => a,
            Err(e) => {
                unsafe { self.device.destroy_buffer(buffer, None) };
                return Err(anyhow!("выделение памяти под буфер '{name}': {e}"));
            }
        };
        // SAFETY: память выделена под требования именно этого буфера.
        unsafe {
            self.device
                .bind_buffer_memory(buffer, allocation.memory(), allocation.offset())?
        };
        Ok(Buffer { buffer, allocation: Some(allocation), size })
    }

    pub fn destroy_buffer(&mut self, mut buffer: Buffer) {
        if let Some(a) = buffer.allocation.take() {
            if let Ok(alloc) = self.allocator() {
                if let Err(e) = alloc.free(a) {
                    log::error!("free buffer: {e}");
                }
            }
        }
        // SAFETY: вызывающий гарантирует, что GPU больше не использует буфер
        // (отложенное удаление через кадры в полёте).
        unsafe { self.device.destroy_buffer(buffer.buffer, None) };
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_image(
        &mut self,
        extent: vk::Extent2D,
        mip_levels: u32,
        layers: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
        view_type: vk::ImageViewType,
        name: &str,
    ) -> Result<Image> {
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
            .mip_levels(mip_levels)
            .array_layers(layers)
            .format(format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(usage)
            .samples(vk::SampleCountFlags::TYPE_1)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        // SAFETY: валидные параметры.
        let image = unsafe { self.device.create_image(&info, None)? };
        let requirements = unsafe { self.device.get_image_memory_requirements(image) };
        let allocation = self
            .allocator()?
            .allocate(&AllocationCreateDesc {
                name,
                requirements,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|e| anyhow!("выделение памяти под изображение '{name}': {e}"))?;
        unsafe {
            self.device
                .bind_image_memory(image, allocation.memory(), allocation.offset())?
        };
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(view_type)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: aspect,
                base_mip_level: 0,
                level_count: mip_levels,
                base_array_layer: 0,
                layer_count: layers,
            });
        let view = unsafe { self.device.create_image_view(&view_info, None)? };
        Ok(Image { image, view, allocation: Some(allocation), format, extent })
    }

    pub fn destroy_image(&mut self, mut image: Image) {
        // SAFETY: GPU больше не использует изображение (вызывается после wait_idle
        // или отложенно).
        unsafe {
            self.device.destroy_image_view(image.view, None);
            self.device.destroy_image(image.image, None);
        }
        if let Some(a) = image.allocation.take() {
            if let Ok(alloc) = self.allocator() {
                if let Err(e) = alloc.free(a) {
                    log::error!("free image: {e}");
                }
            }
        }
    }

    /// Выполняет одноразовые команды синхронно (используется только при загрузке).
    pub fn one_shot<F: FnOnce(vk::CommandBuffer)>(&self, f: F) -> Result<()> {
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.upload_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        // SAFETY: пул принадлежит этому device; командный буфер освобождается ниже
        // после ожидания очереди.
        unsafe {
            let cmd = self.device.allocate_command_buffers(&alloc_info)?[0];
            self.device.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            f(cmd);
            self.device.end_command_buffer(cmd)?;
            let cmds = [cmd];
            let submit = vk::SubmitInfo::default().command_buffers(&cmds);
            let fence = self.device.create_fence(&vk::FenceCreateInfo::default(), None)?;
            self.device.queue_submit(self.graphics_queue, &[submit], fence)?;
            self.device.wait_for_fences(&[fence], true, u64::MAX)?;
            self.device.destroy_fence(fence, None);
            self.device.free_command_buffers(self.upload_pool, &cmds);
        }
        Ok(())
    }

    pub fn find_depth_format(&self) -> Result<vk::Format> {
        for f in [
            vk::Format::D32_SFLOAT,
            vk::Format::D32_SFLOAT_S8_UINT,
            vk::Format::D24_UNORM_S8_UINT,
        ] {
            // SAFETY: запрос свойств формата.
            let props = unsafe {
                self.instance
                    .get_physical_device_format_properties(self.physical_device, f)
            };
            if props
                .optimal_tiling_features
                .contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)
            {
                return Ok(f);
            }
        }
        bail!("нет поддерживаемого формата буфера глубины")
    }

    pub fn wait_idle(&self) {
        // SAFETY: просто ожидание.
        unsafe {
            let _ = self.device.device_wait_idle();
        }
    }
}

impl Drop for VkContext {
    fn drop(&mut self) {
        // SAFETY: обратный порядок создания; перед этим ждём завершения GPU.
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_command_pool(self.upload_pool, None);
            // Аллокатор должен умереть раньше device.
            if let Some(a) = self.allocator.take() {
                a.report_memory_leaks(log::Level::Warn);
                drop(a);
            }
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            if let Some((loader, messenger)) = self.debug.take() {
                loader.destroy_debug_utils_messenger(messenger, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}
