//! Загрузка массивов текстур с mip-уровнями (mipmaps строятся на CPU).

use anyhow::Result;
use ash::vk;
use gpu_allocator::MemoryLocation;

use super::context::{Image, VkContext};

/// Строит цепочку mip-уровней для RGBA-изображения (бокс-фильтр 2×2).
/// Для альфа-текстур (листва) цвет усредняется с весом альфы, чтобы
/// прозрачные пиксели не «пачкали» края тёмным.
pub fn build_mips(w: u32, h: u32, base: &[u8], levels: u32) -> Vec<Vec<u8>> {
    let mut out = vec![base.to_vec()];
    let (mut cw, mut ch) = (w, h);
    for _ in 1..levels {
        let prev = out.last().map(|v| v.as_slice()).unwrap_or(base);
        let nw = (cw / 2).max(1);
        let nh = (ch / 2).max(1);
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0f32; 4];
                let mut wsum = 0f32;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let sx = (x * 2 + dx).min(cw - 1);
                        let sy = (y * 2 + dy).min(ch - 1);
                        let i = ((sy * cw + sx) * 4) as usize;
                        let a = prev[i + 3] as f32 / 255.0;
                        for c in 0..3 {
                            acc[c] += prev[i + c] as f32 * a;
                        }
                        acc[3] += prev[i + 3] as f32;
                        wsum += a;
                    }
                }
                let o = ((y * nw + x) * 4) as usize;
                for c in 0..3 {
                    next[o + c] = if wsum > 0.0 { (acc[c] / wsum).round() as u8 } else { 0 };
                }
                next[o + 3] = (acc[3] / 4.0).round() as u8;
            }
        }
        out.push(next);
        cw = nw;
        ch = nh;
    }
    out
}

/// Создаёт `sampler2DArray` из набора слоёв одинакового размера.
pub fn create_texture_array(
    ctx: &mut VkContext,
    width: u32,
    height: u32,
    layers: &[Vec<u8>],
    mipmaps: bool,
    format: vk::Format,
    name: &str,
) -> Result<Image> {
    let mip_levels = if mipmaps {
        32 - width.max(height).leading_zeros()
    } else {
        1
    };
    let layer_count = layers.len().max(1) as u32;

    // Раскладка staging-буфера: для каждого mip-уровня все слои подряд.
    let per_layer_mips: Vec<Vec<Vec<u8>>> =
        layers.iter().map(|l| build_mips(width, height, l, mip_levels)).collect();
    let mut data = Vec::new();
    let mut regions = Vec::new();
    for level in 0..mip_levels {
        let offset = data.len() as u64;
        for mips in &per_layer_mips {
            data.extend_from_slice(&mips[level as usize]);
        }
        regions.push(
            vk::BufferImageCopy::default()
                .buffer_offset(offset)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: level,
                    base_array_layer: 0,
                    layer_count,
                })
                .image_extent(vk::Extent3D {
                    width: (width >> level).max(1),
                    height: (height >> level).max(1),
                    depth: 1,
                }),
        );
    }

    let mut staging = ctx.create_buffer(
        data.len() as u64,
        vk::BufferUsageFlags::TRANSFER_SRC,
        MemoryLocation::CpuToGpu,
        "texture staging",
    )?;
    staging.write_bytes(&data)?;

    let image = ctx.create_image(
        vk::Extent2D { width, height },
        mip_levels,
        layer_count,
        format,
        vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        vk::ImageAspectFlags::COLOR,
        vk::ImageViewType::TYPE_2D_ARRAY,
        name,
    )?;

    let range = vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: 0,
        level_count: mip_levels,
        base_array_layer: 0,
        layer_count,
    };
    let device = ctx.device.clone();
    let (img, buf) = (image.image, staging.buffer);
    ctx.one_shot(|cmd| {
        // SAFETY: командный буфер в состоянии записи; изображение и буфер валидны.
        unsafe {
            let to_dst = vk::ImageMemoryBarrier::default()
                .image(img)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .subresource_range(range);
            device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_dst],
            );
            device.cmd_copy_buffer_to_image(
                cmd,
                buf,
                img,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
            );
            let to_read = vk::ImageMemoryBarrier::default()
                .image(img)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .subresource_range(range);
            device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_read],
            );
        }
    })?;
    ctx.destroy_buffer(staging);
    Ok(image)
}

pub fn create_sampler(
    ctx: &VkContext,
    pixelated: bool,
    max_lod: f32,
    anisotropy: f32,
) -> Result<vk::Sampler> {
    let aniso = anisotropy.min(ctx.limits.max_sampler_anisotropy);
    let info = vk::SamplerCreateInfo::default()
        .mag_filter(if pixelated { vk::Filter::NEAREST } else { vk::Filter::LINEAR })
        // Пиксель-арт: вблизи — чёткие тексели, вдали — трилинейная фильтрация.
        .min_filter(if pixelated && max_lod <= 0.0 { vk::Filter::NEAREST } else { vk::Filter::LINEAR })
        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
        .address_mode_u(vk::SamplerAddressMode::REPEAT)
        .address_mode_v(vk::SamplerAddressMode::REPEAT)
        .address_mode_w(vk::SamplerAddressMode::REPEAT)
        .anisotropy_enable(aniso > 1.0)
        .max_anisotropy(aniso.max(1.0))
        .min_lod(0.0)
        .max_lod(max_lod)
        .border_color(vk::BorderColor::INT_OPAQUE_BLACK);
    // SAFETY: валидные параметры; anisotropy включена только если поддерживается.
    Ok(unsafe { ctx.device.create_sampler(&info, None)? })
}
