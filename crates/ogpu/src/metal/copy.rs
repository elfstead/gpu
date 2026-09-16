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
        let encoder = self.encoder()?;
        if (so | doff | size) % 4 == 0 {
            unsafe {
                let _: () = msg_send![encoder,
                    copyFromBuffer: source.raw.as_ptr()
                    sourceOffset: so
                    toBuffer: destination.raw.as_ptr()
                    destinationOffset: doff
                    size: size];
            }
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
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    parameters.as_ptr().cast::<u8>(),
                    std::mem::size_of_val(&parameters),
                )
            };
            let parameters_address = self.upload(bytes)?;
            let table = self.argument_table(&[
                source.raw.gpu_address(),
                destination.raw.gpu_address(),
                parameters_address,
            ])?;
            unsafe {
                let pipeline_object = &*pipeline.as_ptr().cast::<Object>();
                let table_object = &*table.0;
                let _: () = msg_send![encoder, setComputePipelineState: pipeline_object];
                let _: () = msg_send![encoder, setArgumentTable: table_object];
                let grid = MTLSize {
                    width: groups,
                    height: 1,
                    depth: 1,
                };
                let group = MTLSize {
                    width,
                    height: 1,
                    depth: 1,
                };
                let _: () = msg_send![encoder,
                    dispatchThreadgroups: grid threadsPerThreadgroup: group];
            }
            self.tables.push(table);
        }
        contract::retain(&mut self.retained, source.clone());
        contract::retain(&mut self.retained, destination.clone());
        Ok(())
    }
}
