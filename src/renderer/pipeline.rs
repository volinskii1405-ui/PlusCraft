//! Загрузка SPIR-V и сборка графических пайплайнов.

use std::io::Cursor;

use anyhow::{Context, Result};
use ash::vk;

use super::context::VkContext;

/// Встроенные в бинарник SPIR-V (собраны build.rs).
fn embedded_spirv(name: &str) -> Option<&'static [u8]> {
    macro_rules! shader {
        ($n:literal) => {
            include_bytes!(concat!(env!("OUT_DIR"), "/shaders/", $n, ".spv")).as_slice()
        };
    }
    Some(match name {
        "chunk.vert" => shader!("chunk.vert"),
        "chunk.frag" => shader!("chunk.frag"),
        "entity.vert" => shader!("entity.vert"),
        "entity.frag" => shader!("entity.frag"),
        "sky.vert" => shader!("sky.vert"),
        "sky.frag" => shader!("sky.frag"),
        "ui.vert" => shader!("ui.vert"),
        "ui.frag" => shader!("ui.frag"),
        _ => return None,
    })
}

/// Загружает шейдер: сначала из `assets/shaders/<name>.spv` (можно подменить
/// без пересборки), иначе — встроенную копию.
pub fn load_shader(ctx: &VkContext, name: &str) -> Result<vk::ShaderModule> {
    let path = crate::paths::assets_dir().join("shaders").join(format!("{name}.spv"));
    let bytes: Vec<u8> = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => embedded_spirv(name)
            .with_context(|| format!("шейдер {name} не найден"))?
            .to_vec(),
    };
    let code = ash::util::read_spv(&mut Cursor::new(&bytes))
        .with_context(|| format!("некорректный SPIR-V: {name}"))?;
    let info = vk::ShaderModuleCreateInfo::default().code(&code);
    // SAFETY: код — корректно выровненный SPIR-V (read_spv проверяет magic).
    Ok(unsafe { ctx.device.create_shader_module(&info, None)? })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    None,
    Alpha,
}

pub struct PipelineDesc<'a> {
    pub vert: &'a str,
    pub frag: &'a str,
    pub binding: Option<vk::VertexInputBindingDescription>,
    pub attributes: Vec<vk::VertexInputAttributeDescription>,
    pub topology: vk::PrimitiveTopology,
    pub cull: vk::CullModeFlags,
    pub depth_test: bool,
    pub depth_write: bool,
    pub blend: Blend,
    /// Значение спец. константы 0 во фрагментном шейдере (порог альфа-теста).
    pub alpha_cut: f32,
    pub depth_bias: bool,
}

pub fn create_pipeline(
    ctx: &VkContext,
    desc: &PipelineDesc,
    layout: vk::PipelineLayout,
    render_pass: vk::RenderPass,
) -> Result<vk::Pipeline> {
    let vert = load_shader(ctx, desc.vert)?;
    let frag = load_shader(ctx, desc.frag)?;

    let spec_entries = [vk::SpecializationMapEntry { constant_id: 0, offset: 0, size: 4 }];
    let spec_data = desc.alpha_cut.to_ne_bytes();
    let spec = vk::SpecializationInfo::default()
        .map_entries(&spec_entries)
        .data(&spec_data);

    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert)
            .name(c"main"),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag)
            .name(c"main")
            .specialization_info(&spec),
    ];

    let bindings: Vec<_> = desc.binding.into_iter().collect();
    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&bindings)
        .vertex_attribute_descriptions(&desc.attributes);
    let input_assembly =
        vk::PipelineInputAssemblyStateCreateInfo::default().topology(desc.topology);
    let viewport = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let raster = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(desc.cull)
        // Вершины граней генерируются против часовой стрелки при взгляде снаружи.
        // Переворот Y в проекции компенсирует направленную вниз ось Y
        // framebuffer'а Vulkan, поэтому лицевая сторона остаётся CCW.
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .line_width(1.0)
        .depth_bias_enable(desc.depth_bias)
        // Обратный Z: «ближе» = больше, поэтому смещение положительное.
        .depth_bias_constant_factor(if desc.depth_bias { 1.0 } else { 0.0 })
        .depth_bias_slope_factor(if desc.depth_bias { 1.0 } else { 0.0 });
    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);
    // Обратный Z: очищаем глубину нулём, проходит большее значение.
    let depth = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(desc.depth_test)
        .depth_write_enable(desc.depth_write)
        .depth_compare_op(vk::CompareOp::GREATER_OR_EQUAL);
    let blend_attachment = match desc.blend {
        Blend::None => vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(false)
            .color_write_mask(vk::ColorComponentFlags::RGBA),
        Blend::Alpha => vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA),
    };
    let attachments = [blend_attachment];
    let color_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);
    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport)
        .rasterization_state(&raster)
        .multisample_state(&multisample)
        .depth_stencil_state(&depth)
        .color_blend_state(&color_blend)
        .dynamic_state(&dynamic)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0);

    // SAFETY: все структуры живут до конца вызова; модули уничтожаем после.
    let result = unsafe {
        ctx.device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
    };
    unsafe {
        ctx.device.destroy_shader_module(vert, None);
        ctx.device.destroy_shader_module(frag, None);
    }
    let pipelines = result.map_err(|(_, e)| e).with_context(|| {
        format!("создание пайплайна {} / {}", desc.vert, desc.frag)
    })?;
    Ok(pipelines[0])
}
