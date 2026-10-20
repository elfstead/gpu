//! Backend-neutral format catalog plus the D3D12 (DXGI) native mapping and
//! honest per-format capability probes. Unmapped vocabulary entries keep zero
//! capabilities: the format is known to the revision but not to this backend.
use super::*;
use windows::Win32::Graphics::Dxgi::Common as dxgic;

pub(super) struct FormatMeta {
    pub(super) id: u32,
    pub(super) aspects: u32,
    pub(super) block: [u32; 2],
    pub(super) color_bytes: u32,
    pub(super) depth_bytes: u32,
    pub(super) numeric: u8, // 1 float, 2 uint, 3 sint, 4 depth/stencil
}

macro_rules! numeric_id {
    (Float) => {
        1
    };
    (Uint) => {
        2
    };
    (Sint) => {
        3
    };
    (DepthStencil) => {
        4
    };
}

macro_rules! collect {
    ($($name:ident = $id:literal => ($native:ident, $aspects:literal, $class:literal, $numeric:ident, $depth:literal, ($w:literal, $h:literal, $bytes:literal)),)*) => {
        pub(super) const FORMATS: &[FormatMeta] = &[
            $(FormatMeta {
                id: $id,
                aspects: $aspects,
                block: [$w, $h],
                color_bytes: $bytes,
                depth_bytes: $depth,
                numeric: numeric_id!($numeric),
            },)*
        ];
    };
}
crate::foundation::format_catalog::format_catalog!(collect);

/// Native format of each OGPU format ID. Entries without a DXGI equivalent are
/// UNKNOWN and advertise zero capabilities; they are never substituted.
pub(in crate::foundation) fn dxgi_format(id: u32) -> dxgic::DXGI_FORMAT {
    use dxgic::*;
    use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT_B4G4R4A4_UNORM, DXGI_FORMAT_B5G6R5_UNORM,
        DXGI_FORMAT_B5G5R5A1_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
        DXGI_FORMAT_BC1_UNORM, DXGI_FORMAT_BC1_UNORM_SRGB,
        DXGI_FORMAT_BC2_UNORM, DXGI_FORMAT_BC2_UNORM_SRGB, DXGI_FORMAT_BC3_UNORM,
        DXGI_FORMAT_BC3_UNORM_SRGB, DXGI_FORMAT_BC4_SNORM, DXGI_FORMAT_BC4_UNORM,
        DXGI_FORMAT_BC5_SNORM, DXGI_FORMAT_BC5_UNORM, DXGI_FORMAT_BC6H_SF16,
        DXGI_FORMAT_BC6H_UF16, DXGI_FORMAT_BC7_UNORM, DXGI_FORMAT_BC7_UNORM_SRGB,
        DXGI_FORMAT_D16_UNORM, DXGI_FORMAT_D24_UNORM_S8_UINT, DXGI_FORMAT_D32_FLOAT,
        DXGI_FORMAT_D32_FLOAT_S8X24_UINT, DXGI_FORMAT_R10G10B10A2_UNORM,
        DXGI_FORMAT_R10G10B10A2_UINT, DXGI_FORMAT_R11G11B10_FLOAT, DXGI_FORMAT_R16_FLOAT,
        DXGI_FORMAT_R16_SINT, DXGI_FORMAT_R16_SNORM, DXGI_FORMAT_R16_UINT, DXGI_FORMAT_R16_UNORM,
        DXGI_FORMAT_R16G16_FLOAT, DXGI_FORMAT_R16G16_SINT, DXGI_FORMAT_R16G16_SNORM,
        DXGI_FORMAT_R16G16_UINT, DXGI_FORMAT_R16G16_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT,
        DXGI_FORMAT_R16G16B16A16_SINT, DXGI_FORMAT_R16G16B16A16_SNORM,
        DXGI_FORMAT_R16G16B16A16_UINT, DXGI_FORMAT_R16G16B16A16_UNORM, DXGI_FORMAT_R32_FLOAT,
        DXGI_FORMAT_R32_SINT, DXGI_FORMAT_R32_UINT, DXGI_FORMAT_R32G32_FLOAT,
        DXGI_FORMAT_R32G32_SINT, DXGI_FORMAT_R32G32_UINT, DXGI_FORMAT_R32G32B32_FLOAT,
        DXGI_FORMAT_R32G32B32_SINT, DXGI_FORMAT_R32G32B32_UINT, DXGI_FORMAT_R32G32B32A32_FLOAT,
        DXGI_FORMAT_R32G32B32A32_SINT, DXGI_FORMAT_R32G32B32A32_UINT, DXGI_FORMAT_R8_SINT,
        DXGI_FORMAT_R8_SNORM, DXGI_FORMAT_R8_UINT, DXGI_FORMAT_R8_UNORM, DXGI_FORMAT_R8G8_SINT,
        DXGI_FORMAT_R8G8_SNORM, DXGI_FORMAT_R8G8_UINT, DXGI_FORMAT_R8G8_UNORM,
        DXGI_FORMAT_R8G8B8A8_SINT, DXGI_FORMAT_R8G8B8A8_SNORM, DXGI_FORMAT_R8G8B8A8_UINT,
        DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, DXGI_FORMAT_R9G9B9E5_SHAREDEXP,
    };
    match id {
        1 => DXGI_FORMAT_R8_UNORM,
        2 => DXGI_FORMAT_R8G8_UNORM,
        3 | 64 => DXGI_FORMAT_R8G8B8A8_UNORM,
        4 | 70 => DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        5 => DXGI_FORMAT_B8G8R8A8_UNORM,
        6 => DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
        7 => DXGI_FORMAT_R16_FLOAT,
        8 => DXGI_FORMAT_R16G16_FLOAT,
        9 => DXGI_FORMAT_R16G16B16A16_FLOAT,
        10 => DXGI_FORMAT_R32_FLOAT,
        11 => DXGI_FORMAT_R32_UINT,
        12 => DXGI_FORMAT_R32G32B32A32_FLOAT,
        13 => DXGI_FORMAT_D16_UNORM,
        14 => DXGI_FORMAT_D32_FLOAT,
        15 => DXGI_FORMAT_D24_UNORM_S8_UINT,
        16 => DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
        17 => DXGI_FORMAT_R32G32_FLOAT,
        18 => DXGI_FORMAT_R32G32B32_FLOAT,
        19 => DXGI_FORMAT_R16G16B16A16_UNORM,
        22 => DXGI_FORMAT_B4G4R4A4_UNORM,
        24 => DXGI_FORMAT_B5G6R5_UNORM,
        26 => DXGI_FORMAT_B5G5R5A1_UNORM,
        28 => DXGI_FORMAT_R8_SNORM,
        31 => DXGI_FORMAT_R8_UINT,
        32 => DXGI_FORMAT_R8_SINT,
        34 => DXGI_FORMAT_R8G8_SNORM,
        37 => DXGI_FORMAT_R8G8_UINT,
        38 => DXGI_FORMAT_R8G8_SINT,
        54 | 65 => DXGI_FORMAT_R8G8B8A8_SNORM,
        57 | 68 => DXGI_FORMAT_R8G8B8A8_UINT,
        58 | 69 => DXGI_FORMAT_R8G8B8A8_SINT,
        78 => DXGI_FORMAT_R10G10B10A2_UNORM,
        81 => DXGI_FORMAT_R10G10B10A2_UINT,
        83 => DXGI_FORMAT_R16_UNORM,
        84 => DXGI_FORMAT_R16_SNORM,
        87 => DXGI_FORMAT_R16_UINT,
        88 => DXGI_FORMAT_R16_SINT,
        89 => DXGI_FORMAT_R16G16_UNORM,
        90 => DXGI_FORMAT_R16G16_SNORM,
        93 => DXGI_FORMAT_R16G16_UINT,
        94 => DXGI_FORMAT_R16G16_SINT,
        102 => DXGI_FORMAT_R16G16B16A16_SNORM,
        105 => DXGI_FORMAT_R16G16B16A16_UINT,
        106 => DXGI_FORMAT_R16G16B16A16_SINT,
        107 => DXGI_FORMAT_R32_SINT,
        108 => DXGI_FORMAT_R32G32_UINT,
        109 => DXGI_FORMAT_R32G32_SINT,
        110 => DXGI_FORMAT_R32G32B32_UINT,
        111 => DXGI_FORMAT_R32G32B32_SINT,
        112 => DXGI_FORMAT_R32G32B32A32_UINT,
        113 => DXGI_FORMAT_R32G32B32A32_SINT,
        114 => DXGI_FORMAT_R11G11B10_FLOAT,
        115 => DXGI_FORMAT_R9G9B9E5_SHAREDEXP,
        119 | 121 => DXGI_FORMAT_BC1_UNORM,
        120 | 122 => DXGI_FORMAT_BC1_UNORM_SRGB,
        123 => DXGI_FORMAT_BC2_UNORM,
        124 => DXGI_FORMAT_BC2_UNORM_SRGB,
        125 => DXGI_FORMAT_BC3_UNORM,
        126 => DXGI_FORMAT_BC3_UNORM_SRGB,
        127 => DXGI_FORMAT_BC4_UNORM,
        128 => DXGI_FORMAT_BC4_SNORM,
        129 => DXGI_FORMAT_BC5_UNORM,
        130 => DXGI_FORMAT_BC5_SNORM,
        131 => DXGI_FORMAT_BC6H_UF16,
        132 => DXGI_FORMAT_BC6H_SF16,
        133 => DXGI_FORMAT_BC7_UNORM,
        134 => DXGI_FORMAT_BC7_UNORM_SRGB,
        _ => DXGI_FORMAT_UNKNOWN,
    }
}

pub(super) fn meta(id: u32) -> Option<&'static FormatMeta> {
    FORMATS.iter().find(|f| f.id == id)
}

/// Immutable per-format capabilities from `CheckFeatureSupport`. D3D12 has no
/// linear image tiling and no native blit: those masks stay zero. Copy masks are
/// advertised wherever the format backs a native resource at all.
pub(super) fn format_snapshot(device: &d3d::ID3D12Device) -> Vec<FormatInfo> {
    FORMATS
        .iter()
        .map(|format| {
            let native = dxgi_format(format.id);
            let mut support1 = d3d::D3D12_FEATURE_DATA_FORMAT_SUPPORT::default();
            let supported = native != DXGI_FORMAT_UNKNOWN
                && unsafe {
                    support1.Format = native;
                    device
                        .CheckFeatureSupport(
                            d3d::D3D12_FEATURE_FORMAT_SUPPORT,
                            ptr::from_mut(&mut support1).cast(),
                            size_of::<d3d::D3D12_FEATURE_DATA_FORMAT_SUPPORT>() as u32,
                        )
                        .is_ok()
                };
            let (mut optimal, mut buffer) = (0u64, 0u64);
            if supported {
                let s1 = support1.Support1;
                let s2 = support1.Support2;
                let sampled = s1.contains(d3d::D3D12_FORMAT_SUPPORT1_SHADER_SAMPLE);
                optimal |= feature(format_features::SAMPLED, sampled);
                optimal |= feature(
                    format_features::STORAGE,
                    s1.contains(d3d::D3D12_FORMAT_SUPPORT1_TYPED_UNORDERED_ACCESS_VIEW),
                );
                optimal |= feature(
                    format_features::STORAGE_ATOMIC,
                    (s2.0
                        & (d3d::D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_ADD.0
                            | d3d::D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_EXCHANGE.0
                            | d3d::D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_BITWISE_OPS.0
                            | d3d::D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_SIGNED_MIN_OR_MAX.0
                            | d3d::D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_UNSIGNED_MIN_OR_MAX.0
                            | d3d::D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_COMPARE_STORE_OR_COMPARE_EXCHANGE
                                .0))
                        != 0,
                );
                optimal |= feature(
                    format_features::COLOR,
                    s1.contains(d3d::D3D12_FORMAT_SUPPORT1_RENDER_TARGET),
                );
                optimal |= feature(
                    format_features::BLEND,
                    s1.contains(d3d::D3D12_FORMAT_SUPPORT1_BLENDABLE),
                );
                optimal |= feature(
                    format_features::DEPTH_STENCIL,
                    s1.contains(d3d::D3D12_FORMAT_SUPPORT1_DEPTH_STENCIL),
                );
                // No native blit exists: BLIT masks stay zero on this backend.
                // Linear filtering is implied for sampled float/normalized
                // formats; integer formats never filter linearly.
                optimal |= feature(
                    format_features::FILTER_LINEAR,
                    sampled && matches!(format.numeric, 1 | 4),
                );
                let copy = (s1.0
                    & (d3d::D3D12_FORMAT_SUPPORT1_BUFFER.0
                        | d3d::D3D12_FORMAT_SUPPORT1_TEXTURE1D.0
                        | d3d::D3D12_FORMAT_SUPPORT1_TEXTURE2D.0
                        | d3d::D3D12_FORMAT_SUPPORT1_TEXTURE3D.0))
                    != 0;
                optimal |= feature(format_features::COPY_SRC, copy);
                optimal |= feature(format_features::COPY_DST, copy);
                buffer |= feature(
                    format_buffer_features::VERTEX,
                    s1.contains(d3d::D3D12_FORMAT_SUPPORT1_IA_VERTEX_BUFFER),
                );
                buffer |= feature(
                    format_buffer_features::UNIFORM_TEXEL,
                    s1.contains(d3d::D3D12_FORMAT_SUPPORT1_BUFFER),
                );
                buffer |= feature(
                    format_buffer_features::STORAGE_TEXEL,
                    s1.contains(d3d::D3D12_FORMAT_SUPPORT1_TYPED_UNORDERED_ACCESS_VIEW),
                );
                buffer |= feature(
                    format_buffer_features::ATOMIC_TEXEL,
                    (s2.0 & d3d::D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_EXCHANGE.0) != 0,
                );
            }
            let stencil_bytes = if format.aspects & 4 != 0 { 1 } else { 0 };
            FormatInfo {
                format: format.id,
                aspects: format.aspects,
                optimal_features: optimal,
                // No linear image tiling exists on D3D12.
                linear_features: 0,
                buffer_features: buffer,
                copy_block_extent: [format.block[0], format.block[1], 1],
                copy_block_bytes: [
                    if format.aspects & 1 != 0 { format.color_bytes } else { 0 },
                    if format.aspects & 2 != 0 { format.depth_bytes } else { 0 },
                    stencil_bytes,
                ],
            }
        })
        .collect()
}

fn feature(bit: u64, present: bool) -> u64 {
    if present {
        bit
    } else {
        0
    }
}
