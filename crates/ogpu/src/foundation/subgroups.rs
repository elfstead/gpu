//! Cold, per-stage execution constraints; no assumed wave size in command recording.
use super::*;

pub(super) unsafe fn prepare(
    d: &Device,
    shader: &Shader,
    local: [u32; 3],
) -> Result<(u32, vk::VkPipelineShaderStageRequiredSubgroupSizeCreateInfo), Status> {
    let mut native = vk::VkPipelineShaderStageRequiredSubgroupSizeCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_REQUIRED_SUBGROUP_SIZE_CREATE_INFO,
        ..Default::default()
    };
    if shader.subgroup.is_null() {
        return Ok((0, native));
    }
    let state = unsafe { record::<SubgroupState>(shader.subgroup, SUBGROUP_STATE)? };
    validate(
        &d.snapshot.subgroup_limits,
        d.snapshot.features.enabled,
        shader.stage as u64,
        local,
        state,
    )?;
    native.requiredSubgroupSize = state.required_size;
    let flags = (if state.flags & 1 != 0 {
        vk::VkPipelineShaderStageCreateFlagBits_VK_PIPELINE_SHADER_STAGE_CREATE_ALLOW_VARYING_SUBGROUP_SIZE_BIT
    } else {
        0
    }) | (if state.flags & 2 != 0 {
        vk::VkPipelineShaderStageCreateFlagBits_VK_PIPELINE_SHADER_STAGE_CREATE_REQUIRE_FULL_SUBGROUPS_BIT
    } else {
        0
    });
    Ok((flags, native))
}

fn validate(
    limits: &SubgroupLimits,
    enabled: u64,
    stage: u64,
    local: [u32; 3],
    state: &SubgroupState,
) -> Result<(), Status> {
    if state.flags & !3 != 0 || state.reserved != 0 || state.operations & !255 != 0 {
        return Err(UNSUPPORTED);
    }
    if state.operations != 0
        && (limits.stages & stage == 0 || state.operations & !limits.operations != 0)
    {
        return Err(UNSUPPORTED);
    }
    if state.operations & 128 != 0 && stage != 4 && stage != 16 && limits.quad_all_stages == 0 {
        return Err(UNSUPPORTED);
    }
    let varying = state.flags & 1 != 0;
    let full = state.flags & 2 != 0;
    let size = state.required_size;
    if size != 0 && (!size.is_power_of_two() || varying) {
        return Err(INVALID);
    }
    if (varying || size != 0) && enabled & SUBGROUP_SIZE_CONTROL == 0 {
        return Err(UNSUPPORTED);
    }
    if size != 0
        && (size < limits.min_size
            || size > limits.max_size
            || limits.required_size_stages & stage == 0)
    {
        return Err(UNSUPPORTED);
    }
    if full && (stage != 4 || enabled & FULL_SUBGROUPS == 0) {
        return Err(UNSUPPORTED);
    }
    if stage == 4 {
        if local.contains(&0) {
            return Err(INVALID);
        }
        let invocations = local
            .into_iter()
            .try_fold(1u64, |n, v| n.checked_mul(u64::from(v)))
            .ok_or(INVALID)?;
        if size != 0
            && invocations > u64::from(size) * u64::from(limits.max_compute_workgroup_subgroups)
        {
            return Err(UNSUPPORTED);
        }
        let full_size = if size != 0 {
            size
        } else if varying {
            limits.max_size
        } else {
            limits.default_size
        };
        if full && (full_size == 0 || local[0] % full_size != 0) {
            return Err(INVALID);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subgroup_contracts_preserve_native_size_choices() {
        let limits = SubgroupLimits {
            default_size: 32,
            min_size: 8,
            max_size: 64,
            max_compute_workgroup_subgroups: 4,
            stages: 28,
            required_size_stages: 4,
            operations: 255,
            quad_all_stages: 0,
        };
        let all = SUBGROUP_SIZE_CONTROL | FULL_SUBGROUPS;
        let mut state = SubgroupState {
            header: Record::new::<SubgroupState>(SUBGROUP_STATE),
            operations: 5,
            required_size: 0,
            flags: 0,
            reserved: 0,
        };
        assert_eq!(validate(&limits, 0, 4, [33, 1, 1], &state), Ok(()));
        state.required_size = 3;
        assert_eq!(validate(&limits, all, 4, [64, 1, 1], &state), Err(INVALID));
        state.required_size = 8;
        assert_eq!(
            validate(&limits, 0, 4, [32, 1, 1], &state),
            Err(UNSUPPORTED)
        );
        assert_eq!(validate(&limits, all, 4, [32, 1, 1], &state), Ok(()));
        assert_eq!(
            validate(&limits, all, 4, [33, 1, 1], &state),
            Err(UNSUPPORTED)
        );
        state.required_size = 32;
        state.flags = 1;
        assert_eq!(validate(&limits, all, 4, [64, 1, 1], &state), Err(INVALID));
        state.flags = 2;
        assert_eq!(validate(&limits, all, 4, [32, 2, 1], &state), Ok(()));
        assert_eq!(validate(&limits, all, 4, [16, 4, 1], &state), Err(INVALID));
        assert_eq!(
            validate(&limits, SUBGROUP_SIZE_CONTROL, 4, [64, 1, 1], &state),
            Err(UNSUPPORTED)
        );
        state.required_size = 0;
        assert_eq!(
            validate(&limits, FULL_SUBGROUPS, 4, [32, 1, 1], &state),
            Ok(())
        );
        state.flags = 3;
        assert_eq!(validate(&limits, all, 4, [32, 1, 1], &state), Err(INVALID));
        assert_eq!(validate(&limits, all, 4, [64, 1, 1], &state), Ok(()));
        assert_eq!(validate(&limits, all, 16, [0; 3], &state), Err(UNSUPPORTED));
        state.flags = 0;
        state.operations = 128;
        assert_eq!(validate(&limits, 0, 8, [0; 3], &state), Err(UNSUPPORTED));
        assert_eq!(validate(&limits, 0, 16, [0; 3], &state), Ok(()));
        state.operations = 256;
        assert_eq!(
            validate(&limits, all, 4, [64, 1, 1], &state),
            Err(UNSUPPORTED)
        );
    }
}
