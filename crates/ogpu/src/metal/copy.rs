//! Native lowering of byte-granular copies; no shader input adapter involved.
use super::*;

impl Batch {
    pub(super) fn copy(
        &mut self,
        source: &Rc<Buffer>,
        so: usize,
        destination: &Rc<Buffer>,
        doff: usize,
        size: usize,
    ) -> Result<(), Error> {
        if (so | doff | size) % 4 == 0 {
            let enc = self.cb()?.new_blit_command_encoder();
            enc.copy_from_buffer(
                &source.raw,
                so as u64,
                &destination.raw,
                doff as u64,
                size as u64,
            );
            enc.end_encoding();
        } else {
            if self.device.byte_copy.get().is_none() {
                let library = self
                    .device
                    .raw
                    .new_library_with_source(
                        include_str!("copy.metal"),
                        &::metal::CompileOptions::new(),
                    )
                    .map_err(|e| {
                        fail(INTERNAL_ERROR, format!("Byte-copy compilation failed: {e}"))
                    })?;
                let function = library
                    .get_function("ogpu_copy_bytes", None)
                    .map_err(|e| fail(INTERNAL_ERROR, format!("Byte-copy entry failed: {e}")))?;
                let pipeline = self
                    .device
                    .raw
                    .new_compute_pipeline_state_with_function(&function)
                    .map_err(|e| fail(INTERNAL_ERROR, format!("Byte-copy pipeline failed: {e}")))?;
                let _ = self.device.byte_copy.set(pipeline);
            }
            let pipeline = self.device.byte_copy.get().unwrap().clone();
            let width = pipeline.max_total_threads_per_threadgroup().min(256);
            let groups = (size as u64).div_ceil(width).min(65535);
            let parameters = [so as u64, doff as u64, size as u64, groups * width];
            let enc = self.cb()?.new_compute_command_encoder();
            enc.set_compute_pipeline_state(&pipeline);
            enc.set_buffer(0, Some(&source.raw), 0);
            enc.set_buffer(1, Some(&destination.raw), 0);
            enc.set_bytes(
                2,
                std::mem::size_of_val(&parameters) as u64,
                parameters.as_ptr().cast(),
            );
            enc.dispatch_thread_groups(
                MTLSize {
                    width: groups,
                    height: 1,
                    depth: 1,
                },
                MTLSize {
                    width,
                    height: 1,
                    depth: 1,
                },
            );
            enc.end_encoding();
        }
        contract::retain(&mut self.retained, source.clone());
        contract::retain(&mut self.retained, destination.clone());
        Ok(())
    }
}
