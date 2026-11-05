//! Application-only conveniences for this consumer's single-batch calls.
//! Errors propagate to its fail-stop/retain-resources policy; no recovery runtime.
use ogpu::foundation::{rust as gpu, types::*};

pub unsafe fn one(
    queue: gpu::QueueHandle,
    lists: &[gpu::ListHandle],
    waits: &[SyncPoint],
    signals: &[SyncPoint],
    binary_waits: &[BinaryPoint],
    binary_signals: &[BinaryPoint],
    scratch: HostSpan,
) -> gpu::Result<()> {
    let count = |n| u32::try_from(n).map_err(|_| CAPACITY);
    let desc = SubmitDesc {
        list_count: count(lists.len())?,
        wait_count: count(waits.len())?,
        signal_count: count(signals.len())?,
        binary_wait_count: count(binary_waits.len())?,
        binary_signal_count: count(binary_signals.len())?,
        flags: 0,
        lists: lists.as_ptr().cast(),
        waits: waits.as_ptr(),
        signals: signals.as_ptr(),
        binary_waits: binary_waits.as_ptr(),
        binary_signals: binary_signals.as_ptr(),
    };
    let mut outcome = [SubmitOutcome {
        status: INVALID,
        acceptance: SUBMIT_REJECTED,
    }];
    let result = unsafe { gpu::queue_submit(queue, &[desc], &mut outcome, scratch) };
    // The application never treats UNKNOWN as safely rejected or retries it.
    // All errors leave GPU objects retained by its caller until process exit.
    result?;
    assert_eq!(outcome[0].status, OK);
    assert_eq!(outcome[0].acceptance, SUBMIT_ACCEPTED);
    Ok(())
}
