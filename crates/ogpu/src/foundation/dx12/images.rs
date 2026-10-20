//! Explicit D3D12 images and views. Images are placed resources over opaque
//! memory (or dedicated committed backing); no implicit transition, staging or
//! initialization exists in any operation.
use super::*;

#[derive(Clone, Copy)]
struct Facts {
    format: u32,
    dimension: u32,
    flags: u32,
    mip_count: u32,
    layer_count: u32,
    sample_count: u32,
    extent: Extent,
    usage: u64,
}

pub struct Image {
    device: *const Device,
    facts: Facts,
    native: d3d::D3D12_RESOURCE_DESC,
    size: u64,
    alignment: u64,
    resource: std::cell::OnceCell<d3d::ID3D12Resource>,
    borrowed: bool,
}

/// A foundation view describes interpretation of an image for descriptors and
/// attachments. On D3D12 the native view is descriptor-heap content, so the
/// object is a validated interpretation record that descriptor materialization
/// and rendering consume (docs/dx12-backend-d1.md, "views are descriptor
/// contents").
pub struct View {
    pub(super) image: *const Image,
    pub(super) desc: ViewDesc,
    /// Native render/depth target descriptors owned by this view; attachment
    /// views carry exactly one of them.
    targets: Targets,
}

/// Native descriptor slots of an attachment view (cold creation).
#[derive(Clone, Copy)]
pub(super) enum Targets {
    None,
    RenderTarget {
        handle: d3d::D3D12_CPU_DESCRIPTOR_HANDLE,
    },
    DepthStencil {
        handle: d3d::D3D12_CPU_DESCRIPTOR_HANDLE,
    },
}

impl Drop for Image {
    fn drop(&mut self) {
        // The placed/committed resource releases its reference; the caller has
        // retired all uses and kept the backing alive in lifetime order.
    }
}

/// Image usage bits translate to native resource flags. Unknown bits are
/// rejected; TRANSIENT/INPUT_ATTACHMENT have no D3D12 resource flag.
fn resource_flags(usage: u64, format_aspects: u32) -> Result<d3d::D3D12_RESOURCE_FLAGS, Status> {
    if usage & !(1 | 2 | 4 | 8 | 16 | 32) != 0 {
        return Err(UNSUPPORTED);
    }
    let mut flags = d3d::D3D12_RESOURCE_FLAG_NONE;
    if usage & 8 != 0 {
        flags |= d3d::D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    }
    if usage & 16 != 0 {
        if format_aspects & 1 == 0 {
        return Err(INVALID);
        }
        flags |= d3d::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    }
    if usage & 32 != 0 {
        if format_aspects & (2 | 4) == 0 {
        return Err(INVALID);
        }
        flags |= d3d::D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL;
    }
    Ok(flags)
}

fn prepare(d: &Device, desc: &ImageDesc) -> Result<(Facts, d3d::D3D12_RESOURCE_DESC, d3d::D3D12_RESOURCE_ALLOCATION_INFO), Status> {
    desc.header.validate::<ImageDesc>(IMAGE_DESC)?;
    if desc.mip_count == 0
        || desc.layer_count == 0
        || desc.sample_count == 0
        || desc.extent.x == 0
        || desc.extent.y == 0
        || desc.extent.z == 0
    {
        return Err(INVALID);
    }
    let meta = formats::meta(desc.format).ok_or(UNSUPPORTED)?;
    if desc.view_format_count != 0 {
        // View-format lists are part of the descriptor/view profile (D2).
        return Err(UNSUPPORTED);
    }
    let concurrent = desc.concurrent_domain_count;
    if concurrent == 1
        || concurrent as usize > d.snapshot.queues.len()
        || (concurrent != 0 && desc.concurrent_domains.is_null())
    {
        return Err(INVALID);
    }
    if concurrent != 0 {
        // SAFETY: caller provides the described domain array.
        let domains =
            unsafe { std::slice::from_raw_parts(desc.concurrent_domains, concurrent as usize) };
        for (index, domain) in domains.iter().enumerate() {
            if domains[..index].contains(domain) {
        return Err(INVALID);
            }
            if !d.snapshot.queues.iter().any(|q| q.domain == *domain) {
                return Err(UNSUPPORTED);
            }
        }
    }
    let native_format = formats::dxgi_format(desc.format);
    if native_format == windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN {
        return Err(UNSUPPORTED);
    }
    let flags = resource_flags(desc.usage, meta.aspects)?;
    let dimension = match desc.dimension {
        1 => d3d::D3D12_RESOURCE_DIMENSION_TEXTURE1D,
        2 => d3d::D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        3 => d3d::D3D12_RESOURCE_DIMENSION_TEXTURE3D,
        _ => return Err(INVALID),
    };
    if desc.dimension != 3 && desc.extent.z != 1 {
        return Err(INVALID);
    }
    if desc.dimension == 1 && (desc.extent.y != 1 || desc.sample_count != 1) {
        return Err(INVALID);
    }
    if desc.sample_count != 1 && desc.usage & 8 != 0 {
        // Multisampled storage images are outside this profile.
        return Err(UNSUPPORTED);
    }
    let native = d3d::D3D12_RESOURCE_DESC {
        Dimension: dimension,
        Width: u64::from(desc.extent.x),
        Height: desc.extent.y,
        DepthOrArraySize: if desc.dimension == 3 {
            desc.extent.z as u16
        } else {
            desc.layer_count as u16
        },
        MipLevels: desc.mip_count as u16,
        Format: native_format,
        SampleDesc: dxgi::Common::DXGI_SAMPLE_DESC {
            Count: desc.sample_count,
            Quality: 0,
        },
        Layout: d3d::D3D12_TEXTURE_LAYOUT_UNKNOWN,
        Flags: flags,
        Alignment: 0,
    };
    let info = d.allocation_info(&native);
    if info.SizeInBytes == 0 || info.SizeInBytes == u64::MAX || info.Alignment == 0 {
        return Err(UNSUPPORTED);
    }
    Ok((
        Facts {
            format: desc.format,
            dimension: desc.dimension,
            flags: desc.flags,
            mip_count: desc.mip_count,
            layer_count: desc.layer_count,
            sample_count: desc.sample_count,
            extent: desc.extent,
            usage: desc.usage,
        },
        native,
        info,
    ))
}

impl Device {
    /// Limits of the exact image profile tuple; no format substitution. This
    /// reports the implemented profile, not that every maximal value combines.
    pub(in crate::foundation) fn image_limits(
        &self,
        profile: &ImageProfile,
    ) -> Result<ImageLimits, Status> {
        self.ready()?;
        profile.header.validate::<ImageProfile>(IMAGE_PROFILE)?;
        if profile.reserved != 0 {
            return Err(UNSUPPORTED);
        }
        let meta = formats::meta(profile.format).ok_or(UNSUPPORTED)?;
        if profile.usage & !(1 | 2 | 4 | 8 | 16 | 32) != 0 {
            return Err(UNSUPPORTED);
        }
        resource_flags(profile.usage, meta.aspects)?;
        let (max_extent, max_mips) = match profile.dimension {
            1 => ([16384, 1, 1], 15),
            2 => ([16384, 16384, 1], 15),
            3 => ([2048, 2048, 2048], 12),
            _ => return Err(INVALID),
        };
        let mut sample_counts = 1;
        if profile.usage & 8 == 0 {
            for count in [2u32, 4, 8, 16] {
                let mut levels = d3d::D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS {
                    Format: formats::dxgi_format(profile.format),
                    SampleCount: count,
                    Flags: d3d::D3D12_MULTISAMPLE_QUALITY_LEVELS_FLAG_NONE,
                    ..Default::default()
                };
                // SAFETY: feature query with a well-formed structure.
                let supported = unsafe {
                    self.device
                        .CheckFeatureSupport(
                            d3d::D3D12_FEATURE_MULTISAMPLE_QUALITY_LEVELS,
                            ptr::from_mut(&mut levels).cast(),
                            size_of::<d3d::D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS>() as u32,
                        )
                        .is_ok()
                        && levels.NumQualityLevels != 0
                };
                if supported {
                    sample_counts |= count;
                }
            }
        }
        Ok(ImageLimits {
            max_extent,
            max_mips,
            max_layers: 2048,
            sample_counts,
            max_resource_size: self.snapshot.memory_limits.max_buffer_size,
        })
    }

    /// Exact placement requirements of one image. No GPU object is created.
    /// # Safety
    /// Valid description arrays and writable output storage.
    pub(in crate::foundation) unsafe fn image_requirements(
        &self,
        desc: &ImageDesc,
        out: &mut Requirements,
    ) -> Result<(), Status> {
        if out.compatible_type_capacity != 0 && out.compatible_types.is_null() {
        return Err(INVALID);
        }
        let (_facts, _native, info) = prepare(self, desc)?;
        out.size = info.SizeInBytes;
        out.alignment = info.Alignment;
        // Tier-1 heaps keep RT/DS textures in dedicated heaps: report that as an
        // exact dedicated requirement instead of promising placement.
        out.dedicated_required = u32::from(self.heap_tier < 2 && desc.usage & (16 | 32) != 0);
        out.dedicated_preferred = 0;
        let types: Vec<u32> = self
            .snapshot
            .memory_types
            .iter()
            .filter(|m| m.properties & (32 | 64 | 128) == 0)
            .map(|m| m.id)
            .collect();
        out.compatible_type_count = types.len() as u32;
        if out.compatible_type_capacity == 0 {
            return Ok(());
        }
        if out.compatible_type_capacity < out.compatible_type_count {
            return Err(CAPACITY);
        }
        // SAFETY: caller guarantees writable array for capacity elements.
        unsafe {
            ptr::copy_nonoverlapping(types.as_ptr(), out.compatible_types, types.len());
        }
        Ok(())
    }
}

impl Image {
    /// # Safety
    /// Live device and valid description arrays; device outlives the image.
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &ImageDesc,
    ) -> Result<Box<Self>, Status> {
        let (facts, native, info) = prepare(d, desc)?;
        Ok(Box::new(Self {
            device: d,
            facts,
            native,
            size: info.SizeInBytes,
            alignment: info.Alignment,
            resource: std::cell::OnceCell::new(),
            borrowed: false,
        }))
    }

    /// Bind the image to explicit opaque backing exactly once.
    /// # Safety
    /// Placement is valid, compatible and outlives the image and all uses.
    pub(in crate::foundation) unsafe fn bind(&self, placement: Span) -> Result<(), Status> {
        // SAFETY: span memory handle is a trusted live object by contract.
        let memory = unsafe { placement.memory.as_ref() }.ok_or(INVALID)?;
        if self.resource.get().is_some() {
        return Err(INVALID);
        }
        // SAFETY: device pointer and image stay live during binding.
        let d = unsafe { &*self.device };
        d.ready()?;
        memory.image_placement(
            self.device,
            self,
            self.size,
            self.alignment,
            false,
            placement.offset,
            placement.size,
        )?;
        // Dedicated backing is adopted outright; nothing is placed or copied.
        if let Some(committed) = memory.committed() {
            return self.resource.set(committed).map_err(|_| INVALID);
        }
        let heap = memory.heap().ok_or(INVALID)?;
        let mut resource: Option<d3d::ID3D12Resource> = None;
        // SAFETY: validated placement on a live heap; output adopted below.
        unsafe {
            d.device
                .CreatePlacedResource(
                    heap,
                    placement.offset,
                    ptr::from_ref(&self.native),
                    d3d::D3D12_RESOURCE_STATE_COMMON,
                    None,
                    &mut resource,
                )
                .map_err(map_error)?
        };
        // Binding is one-shot; a second bind fails without side effects.
        self.resource
            .set(resource.ok_or(BACKEND_ERROR)?)
            .map_err(|_| INVALID)
    }

    /// Create the dedicated committed backing owner for exactly this image.
    /// It does not bind; binding remains a separate explicit step.
    /// # Safety
    /// Live unbound image and exclusive binding access.
    pub(in crate::foundation) fn dedicated_memory(
        &self,
        memory_type: u32,
    ) -> Result<Box<Memory>, Status> {
        if self.resource.get().is_some() {
        return Err(INVALID);
        }
        // SAFETY: image stays live for backing creation.
        let d = unsafe { &*self.device };
        Memory::dedicated_image(d, self, memory_type)
    }

    pub(in crate::foundation) fn is_borrowed(&self) -> bool {
        self.borrowed
    }

    pub(super) fn resource(&self) -> Result<&d3d::ID3D12Resource, Status> {
        self.resource.get().ok_or(INVALID)
    }

    pub(super) fn native_desc(&self) -> d3d::D3D12_RESOURCE_DESC {
        self.native
    }

    pub(super) fn requirements(&self) -> (u64, u64) {
        (self.size, self.alignment)
    }

    pub(super) fn mip_count(&self) -> u32 {
        self.facts.mip_count
    }

    pub(super) fn sample_count(&self) -> u32 {
        self.facts.sample_count
    }

    pub(super) fn layer_count(&self) -> u32 {
        self.facts.layer_count
    }

    /// Mip extent in texels of one subresource.
    fn mip_extent(&self, mip: u32) -> Result<Extent, Status> {
        if mip >= self.facts.mip_count {
        return Err(INVALID);
        }
        let shift = |v: u32| (v >> mip).max(1);
        Ok(Extent {
            x: shift(self.facts.extent.x),
            y: shift(self.facts.extent.y),
            z: shift(self.facts.extent.z),
        })
    }
}

impl View {
    /// Create a validated view record; the view outlives its image.
    /// # Safety
    /// Live bound image and valid description arrays.
    pub(in crate::foundation) fn create(
        image: &Image,
        desc: &ViewDesc,
    ) -> Result<Box<Self>, Status> {
        desc.header.validate::<ViewDesc>(VIEW_DESC)?;
        if image.resource.get().is_none() {
            // Unbound images cannot create views.
        return Err(INVALID);
        }
        if desc.usage == 0 {
        return Err(INVALID);
        }
        let known = (IMAGE_SAMPLED
            | IMAGE_STORAGE
            | IMAGE_COLOR_ATTACHMENT
            | IMAGE_DEPTH_STENCIL_ATTACHMENT
            | IMAGE_INPUT_ATTACHMENT) as u32;
        if desc.usage & !known != 0 || u64::from(desc.usage) & !image.facts.usage != 0 {
        return Err(INVALID);
        }
        // D3D12 views have no component swizzles; shaders swizzle instead.
        if desc.component_mapping != [0, 0, 0, 0] {
            return Err(UNSUPPORTED);
        }
        if !(VIEW_1D..=VIEW_CUBE_ARRAY).contains(&desc.dimension) {
        return Err(INVALID);
        }
        let meta = formats::meta(image.facts.format).ok_or(UNSUPPORTED)?;
        if desc.range.aspects & !meta.aspects != 0 || desc.range.aspects == 0 {
        return Err(INVALID);
        }
        if desc.range.mip_count == 0
            || desc.range.layer_count == 0
            || desc
                .range
                .first_mip
                .checked_add(desc.range.mip_count)
                .is_none_or(|end| end > image.facts.mip_count)
            || desc
                .range
                .first_layer
                .checked_add(desc.range.layer_count)
                .is_none_or(|end| end > image.facts.layer_count)
        {
        return Err(INVALID);
        }
        if desc.format != image.facts.format {
            // Reinterpretation needs MUTABLE_FORMAT and a compatible class.
            if image.facts.flags & 1 == 0 {
        return Err(INVALID);
            }
            return Err(UNSUPPORTED);
        }
        if desc.dimension == VIEW_CUBE || desc.dimension == VIEW_CUBE_ARRAY {
            if image.facts.flags & 2 == 0 {
        return Err(INVALID);
            }
            return Err(UNSUPPORTED);
        }
        if desc.dimension == VIEW_3D || image.facts.dimension == 3 {
            // 2D views of 3D slices are outside the current profile.
            return Err(UNSUPPORTED);
        }
        // SAFETY: the view's image is live and carries its device.
        let d = unsafe { &*image.device };
        let mut targets = Targets::None;
        if desc.usage & IMAGE_COLOR_ATTACHMENT as u32 != 0 {
            let handle = d.render_target_view(image, desc)?;
            targets = Targets::RenderTarget { handle };
        } else if desc.usage & IMAGE_DEPTH_STENCIL_ATTACHMENT as u32 != 0 {
            let handle = d.depth_stencil_view(image, desc)?;
            targets = Targets::DepthStencil { handle };
        }
        Ok(Box::new(Self {
            image,
            desc: *desc,
            targets,
        }))
    }

    /// The view's render-target descriptor, when it is a color attachment view.
    pub(super) fn render_target(&self) -> Option<d3d::D3D12_CPU_DESCRIPTOR_HANDLE> {
        match self.targets {
            Targets::RenderTarget { handle } => Some(handle),
            _ => None,
        }
    }

    /// The view's depth/stencil descriptor, when it is a depth attachment view.
    pub(super) fn depth_stencil(&self) -> Option<d3d::D3D12_CPU_DESCRIPTOR_HANDLE> {
        match self.targets {
            Targets::DepthStencil { handle } => Some(handle),
            _ => None,
        }
    }

    /// Native shader-descriptor content of this view: the descriptor-profile
    /// materialization derives SRV/UAV descs from the validated interpretation.
    pub(super) fn native_view_desc(&self) -> Result<NativeView, Status> {
        // SAFETY: the view's image is live through every recorded use.
        let image = unsafe { &*self.image };
        let format = formats::dxgi_format(self.desc.format);
        let meta = formats::meta(self.desc.format).ok_or(UNSUPPORTED)?;
        if self.desc.range.mip_count == 0 || self.desc.range.layer_count == 0 {
        return Err(INVALID);
        }
        let array = self.desc.range.layer_count > 1;
        let plane = if meta.aspects & (2 | 4) != 0 {
            // Depth/stencil descriptors address the depth plane here.
            0
        } else {
            0
        };
        let mip_levels = if self.desc.range.mip_count == u32::MAX {
            0
        } else {
            self.desc.range.mip_count
        };
        let srv = d3d::D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: format,
            ViewDimension: if array {
                d3d::D3D12_SRV_DIMENSION_TEXTURE2DARRAY
            } else {
                d3d::D3D12_SRV_DIMENSION_TEXTURE2D
            },
            Shader4ComponentMapping: 0x1688,
            Anonymous: d3d::D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: d3d::D3D12_TEX2D_SRV {
                    MostDetailedMip: self.desc.range.first_mip,
                    MipLevels: mip_levels,
                    PlaneSlice: plane,
                    ResourceMinLODClamp: 0.0,
                },
            },
        };
        let uav = d3d::D3D12_UNORDERED_ACCESS_VIEW_DESC {
            Format: format,
            ViewDimension: if array {
                d3d::D3D12_UAV_DIMENSION_TEXTURE2DARRAY
            } else {
                d3d::D3D12_UAV_DIMENSION_TEXTURE2D
            },
            Anonymous: d3d::D3D12_UNORDERED_ACCESS_VIEW_DESC_0 {
                Texture2D: d3d::D3D12_TEX2D_UAV {
                    MipSlice: self.desc.range.first_mip,
                    PlaneSlice: plane,
                },
            },
        };
        let _ = image;
        Ok(NativeView { srv, uav })
    }
}

/// Native descriptor content of a validated view record.
pub(super) struct NativeView {
    srv: d3d::D3D12_SHADER_RESOURCE_VIEW_DESC,
    uav: d3d::D3D12_UNORDERED_ACCESS_VIEW_DESC,
}

impl NativeView {
    pub(super) fn shader_resource(&self) -> d3d::D3D12_SHADER_RESOURCE_VIEW_DESC {
        self.srv
    }
    pub(super) fn unordered_access(&self) -> d3d::D3D12_UNORDERED_ACCESS_VIEW_DESC {
        self.uav
    }
}

impl Device {
    /// Create the render-target descriptor of an attachment view. The native
    /// descriptor belongs to the view; cold creation may allocate slot storage.
    fn render_target_view(
        &self,
        image: &Image,
        desc: &ViewDesc,
    ) -> Result<d3d::D3D12_CPU_DESCRIPTOR_HANDLE, Status> {
        let format = formats::dxgi_format(desc.format);
        let dimension = if desc.range.layer_count > 1 {
            d3d::D3D12_RTV_DIMENSION_TEXTURE2DARRAY
        } else {
            d3d::D3D12_RTV_DIMENSION_TEXTURE2D
        };
        let view = d3d::D3D12_RENDER_TARGET_VIEW_DESC {
            Format: format,
            ViewDimension: dimension,
            Anonymous: if desc.range.layer_count > 1 {
                d3d::D3D12_RENDER_TARGET_VIEW_DESC_0 {
                    Texture2DArray: d3d::D3D12_TEX2D_ARRAY_RTV {
                        MipSlice: desc.range.first_mip,
                        FirstArraySlice: desc.range.first_layer,
                        ArraySize: desc.range.layer_count,
                        PlaneSlice: 0,
                    },
                }
            } else {
                d3d::D3D12_RENDER_TARGET_VIEW_DESC_0 {
                    Texture2D: d3d::D3D12_TEX2D_RTV {
                        MipSlice: desc.range.first_mip,
                        PlaneSlice: 0,
                    },
                }
            },
        };
        // SAFETY: validated image and view record; output slot is owned by the view.
        let handle = unsafe {
            self.next_view_slot(d3d::D3D12_DESCRIPTOR_HEAP_TYPE_RTV)?
        };
        // SAFETY: live resource and owned descriptor slot.
        unsafe {
            self.device.CreateRenderTargetView(
                Some(image.resource()?),
                Some(&view),
                handle,
            );
        }
        Ok(handle)
    }

    /// Create the depth/stencil descriptor of an attachment view.
    fn depth_stencil_view(
        &self,
        image: &Image,
        desc: &ViewDesc,
    ) -> Result<d3d::D3D12_CPU_DESCRIPTOR_HANDLE, Status> {
        let format = formats::dxgi_format(desc.format);
        let dimension = if desc.range.layer_count > 1 {
            d3d::D3D12_DSV_DIMENSION_TEXTURE2DARRAY
        } else {
            d3d::D3D12_DSV_DIMENSION_TEXTURE2D
        };
        let view = d3d::D3D12_DEPTH_STENCIL_VIEW_DESC {
            Format: format,
            ViewDimension: dimension,
            Flags: d3d::D3D12_DSV_FLAG_NONE,
            Anonymous: if desc.range.layer_count > 1 {
                d3d::D3D12_DEPTH_STENCIL_VIEW_DESC_0 {
                    Texture2DArray: d3d::D3D12_TEX2D_ARRAY_DSV {
                        MipSlice: desc.range.first_mip,
                        FirstArraySlice: desc.range.first_layer,
                        ArraySize: desc.range.layer_count,
                    },
                }
            } else {
                d3d::D3D12_DEPTH_STENCIL_VIEW_DESC_0 {
                    Texture2D: d3d::D3D12_TEX2D_DSV {
                        MipSlice: desc.range.first_mip,
                    },
                }
            },
        };
        // SAFETY: validated image and view record; output slot is owned by the view.
        let handle = unsafe {
            self.next_view_slot(d3d::D3D12_DESCRIPTOR_HEAP_TYPE_DSV)?
        };
        // SAFETY: live resource and owned descriptor slot.
        unsafe {
            self.device.CreateDepthStencilView(Some(image.resource()?), Some(&view), handle);
        }
        Ok(handle)
    }
}

impl List {
    /// Clear an image subresource range through native render-target or
    /// depth/stencil clears. D3D12 clears whole views: one temporary RTV/DSV
    /// per mip slice covers the layer range, with no retention (the native
    /// clear consumes the descriptor at record time). Float/normalized color
    /// and depth/stencil formats clear through their matching value member;
    /// integer-format clears need the UAV clear path this profile does not
    /// expose and stay visible. Compressed clears are never synthesized.
    pub(in crate::foundation) fn clear_image(
        &self,
        image: &Image,
        state: u32,
        range: Subresources,
        value: ClearValue,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        // Native clears are render-target operations on this backend.
        let domain = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == self.domain)
            .ok_or(INVALID)?;
        if domain.flags & super::GRAPHICS == 0 {
            return Err(UNSUPPORTED);
        }
        let meta = super::formats::meta(image.facts.format).ok_or(UNSUPPORTED)?;
        if meta.block != [1, 1] {
            // Compressed clears are UNSUPPORTED, never synthesized.
            return Err(UNSUPPORTED);
        }
        if range.first_mip >= image.facts.mip_count
            || range.mip_count == 0
            || range.first_layer >= image.facts.layer_count
            || range.layer_count == 0
        {
            return Err(INVALID);
        }
        let mips = if range.mip_count == u32::MAX {
            image.facts.mip_count - range.first_mip
        } else {
            range.mip_count
        };
        let layers = if range.layer_count == u32::MAX {
            image.facts.layer_count - range.first_layer
        } else {
            range.layer_count
        };
        if range.first_mip + mips > image.facts.mip_count
            || range.first_layer + layers > image.facts.layer_count
        {
            return Err(INVALID);
        }
        let depth = meta.aspects & 6 != 0;
        // The state names the matching attachment state; the caller owns it.
        let expected = if depth {
            STATE_GENERAL | STATE_DEPTH_STENCIL_ATTACHMENT
        } else {
            STATE_GENERAL | STATE_COLOR_ATTACHMENT
        };
        if state & !expected != 0 || state == 0 {
            return Err(INVALID);
        }
        match meta.numeric {
            4 => {
                // Depth/stencil clears validate their value member.
                // SAFETY: the caller selected the depth_stencil member.
                let clear = unsafe { value.depth_stencil };
                if !clear.depth.is_finite() || !(0.0..=1.0).contains(&clear.depth) {
                    return Err(INVALID);
                }
            }
            1 => {
                // SAFETY: the caller selected the float member.
                let clear = unsafe { value.f32 };
                if clear.iter().any(|v| !v.is_finite()) {
                    return Err(INVALID);
                }
            }
            // Integer-format clears have no float-free native path here.
            _ => return Err(UNSUPPORTED),
        }
        for mip in range.first_mip..range.first_mip + mips {
            let view = ViewDesc {
                header: Record::new::<ViewDesc>(VIEW_DESC),
                format: image.facts.format,
                dimension: VIEW_2D,
                usage: if depth {
                    IMAGE_DEPTH_STENCIL_ATTACHMENT as u32
                } else {
                    IMAGE_COLOR_ATTACHMENT as u32
                },
                component_mapping: [0, 0, 0, 0],
                range: Subresources {
                    aspects: meta.aspects,
                    first_mip: mip,
                    mip_count: 1,
                    first_layer: range.first_layer,
                    layer_count: layers,
                },
            };
            // SAFETY: validated image and per-mip view record; the slot is
            // consumed at record time and never retained.
            unsafe {
                if depth {
                    let handle = d.depth_stencil_view(image, &view)?;
                    let clear = value.depth_stencil;
                    let mut flags = d3d::D3D12_CLEAR_FLAGS(0);
                    if meta.aspects & 2 != 0 {
                        flags |= d3d::D3D12_CLEAR_FLAG_DEPTH;
                    }
                    if meta.aspects & 4 != 0 {
                        flags |= d3d::D3D12_CLEAR_FLAG_STENCIL;
                    }
                    self.command.ClearDepthStencilView(
                        handle,
                        flags,
                        clear.depth,
                        clear.stencil as u8,
                        None,
                    );
                } else {
                    let handle = d.render_target_view(image, &view)?;
                    self.command
                        .ClearRenderTargetView(handle, &value.f32, None);
                }
            }
        }
        Ok(())
    }

    /// Buffer/image copy in either direction through one placed footprint per
    /// layer. D3D12 copies one subresource per native call; a multi-layer region
    /// records one CopyTextureRegion per layer without allocation. Row pitches
    /// must satisfy the native placed-footprint pitch alignment (256 bytes);
    /// other pitches are rejected instead of staged.
    /// # Safety
    /// Image, span and copy description stay live through all pending uses.
    pub(in crate::foundation) unsafe fn copy_image_memory(
        &self,
        image: &Image,
        span: Span,
        copy: &ImageCopy,
        to_image: bool,
    ) -> Result<(), Status> {
        self.recording()?;
        let meta = formats::meta(image.facts.format).ok_or(UNSUPPORTED)?;
        // Depth/stencil copies are a separate graphics-family profile.
        if copy.region.aspect != ASPECT_COLOR || meta.aspects & 1 == 0 {
            return Err(UNSUPPORTED);
        }
        if image.facts.sample_count != 1 {
            return Err(UNSUPPORTED);
        }
        let state_ok = if to_image {
            copy.state == STATE_GENERAL || copy.state == STATE_COPY_DST
        } else {
            copy.state == STATE_GENERAL || copy.state == STATE_COPY_SRC
        };
        if !state_ok {
        return Err(INVALID);
        }
        // SAFETY: span memory handle is a trusted live object by contract.
        let memory = unsafe { span.memory.as_ref() }.ok_or(INVALID)?;
        let usage = if to_image { USAGE_COPY_SRC } else { USAGE_COPY_DST };
        let (buffer, buffer_offset, _) =
            memory.command_range(self.device, self.domain, span.offset, span.size, usage)?;
        let image_usage = if to_image { 2 } else { 1 };
        if image.facts.usage & image_usage == 0 {
        return Err(INVALID);
        }
        let resource = image.resource()?;
        let extent = image.mip_extent(copy.region.mip)?;
        let end_layer = copy
            .region
            .first_layer
            .checked_add(copy.region.layer_count)
            .ok_or(INVALID)?;
        if copy.region.layer_count == 0 || end_layer > image.facts.layer_count {
        return Err(INVALID);
        }
        // Region offsets/extents are block-aligned and within the mip.
        let block = meta.block;
        if copy.region.offset.x < 0
            || copy.region.offset.y < 0
            || copy.region.offset.z != 0
            || u32::try_from(copy.region.offset.x).map_err(|_| INVALID)? % block[0] != 0
            || copy.region.offset.y as u32 % block[1] != 0
        {
        return Err(INVALID);
        }
        let x = u32::try_from(copy.region.offset.x).map_err(|_| INVALID)?;
        let y = copy.region.offset.y as u32;
        let blocks_x = copy.region.extent.x.div_ceil(block[0]);
        let blocks_y = copy.region.extent.y.div_ceil(block[1]);
        if copy.region.extent.z != 1
            || x.checked_add(copy.region.extent.x).ok_or(INVALID)? > extent.x
            || y.checked_add(copy.region.extent.y).ok_or(INVALID)? > extent.y
        {
        return Err(INVALID);
        }
        let row_bytes = u64::from(blocks_x) * u64::from(meta.color_bytes);
        let row_pitch = if copy.row_pitch == 0 {
            row_bytes
        } else {
            copy.row_pitch
        };
        if row_pitch < row_bytes
            || row_pitch % u64::from(meta.color_bytes * block[0]) != 0
            || row_pitch % 256 != 0
        {
            // The native placed-footprint pitch constraint; no hidden staging.
            return Err(UNSUPPORTED);
        }
        let rows = blocks_y * block[1];
        let slice_bytes = row_pitch * u64::from(rows);
        let slice_pitch = if copy.slice_pitch == 0 {
            slice_bytes
        } else {
            copy.slice_pitch
        };
        if slice_pitch < slice_bytes || slice_pitch % slice_bytes != 0 {
        return Err(INVALID);
        }
        if copy.region.layer_count == 0 {
            return Ok(());
        }
        let total = slice_pitch * u64::from(copy.region.layer_count);
        let needed = buffer_offset.checked_add(total).ok_or(INVALID)?;
        if needed > memory_span_end(span) {
        return Err(INVALID);
        }
        for layer in 0..copy.region.layer_count {
            let subresource = copy.region.mip
                + (copy.region.first_layer + layer) * image.facts.mip_count;
            let offset = buffer_offset + u64::from(layer) * slice_pitch;
            let placed = d3d::D3D12_TEXTURE_COPY_LOCATION {
                // Borrowed resource reference: the location is never dropped.
                pResource: core::mem::ManuallyDrop::new(Some(unsafe {
                    d3d::ID3D12Resource::from_raw(buffer.as_raw())
                })),
                Type: d3d::D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: d3d::D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: d3d::D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: offset,
                        Footprint: d3d::D3D12_SUBRESOURCE_FOOTPRINT {
                            Format: image.native.Format,
                            Width: copy.region.extent.x,
                            Height: copy.region.extent.y,
                            Depth: 1,
                            RowPitch: u32::try_from(row_pitch).map_err(|_| INVALID)?,
                        },
                    },
                },
            };
            let texture = d3d::D3D12_TEXTURE_COPY_LOCATION {
                // Borrowed resource reference: the location is never dropped.
                pResource: core::mem::ManuallyDrop::new(Some(unsafe {
                    d3d::ID3D12Resource::from_raw(resource.as_raw())
                })),
                Type: d3d::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: d3d::D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: subresource,
                },
            };
            // SAFETY: validated disjoint ranges of live resources; recording only.
            // A placed-footprint endpoint takes zero coordinates; region offsets
            // belong to the texture endpoint or its explicit source box.
            unsafe {
                if to_image {
                    self.command.CopyTextureRegion(&texture, x, y, 0, &placed, None);
                } else {
                    let box_ = d3d::D3D12_BOX {
                        left: x,
                        top: y,
                        front: 0,
                        right: x + copy.region.extent.x,
                        bottom: y + copy.region.extent.y,
                        back: 1,
                    };
                    self.command.CopyTextureRegion(
                        &placed,
                        0,
                        0,
                        0,
                        &texture,
                        Some(ptr::from_ref(&box_)),
                    );
                }
            };
        }
        Ok(())
    }
}

fn memory_span_end(span: Span) -> u64 {
    span.offset.saturating_add(span.size)
}
