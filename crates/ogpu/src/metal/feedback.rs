//! The callback shares only owned diagnostics, never Rc/native resource owners.
use super::*;
use std::{
    ffi::CStr,
    sync::{Condvar, Mutex},
};

#[derive(Default)]
pub(super) struct Feedback {
    outcome: Mutex<Option<Result<(), Error>>>,
    ready: Condvar,
}

impl Feedback {
    #[cfg(test)]
    pub(super) fn inject_drained_error(&self) {
        let mut outcome = self.outcome.lock().unwrap();
        assert!(
            outcome.is_some(),
            "Never manufacture terminal completion of live work"
        );
        *outcome = Some(Err(fail(INTERNAL_ERROR, "injected drained commit failure")));
    }

    fn complete(&self, outcome: Result<(), Error>) {
        *self.outcome.lock().unwrap_or_else(|e| e.into_inner()) = Some(outcome);
        self.ready.notify_all();
    }

    pub(super) fn observe(&self, wait: bool) -> Option<Result<(), Error>> {
        let mut outcome = self.outcome.lock().unwrap_or_else(|e| e.into_inner());
        while wait && outcome.is_none() {
            outcome = self.ready.wait(outcome).unwrap_or_else(|e| e.into_inner());
        }
        outcome.clone()
    }
}

// Copy the NSError while it is valid in the callback. Metal errors aren't
// VkResults; report INTERNAL_ERROR with the native code and description.
unsafe fn native_result(error: *mut Object) -> Result<(), Error> {
    if error.is_null() {
        return Ok(());
    }
    unsafe {
        let code: isize = msg_send![error, code];
        let description: *mut Object = msg_send![error, localizedDescription];
        let text: *const std::ffi::c_char = msg_send![description, UTF8String];
        let text = if text.is_null() {
            "no description".into()
        } else {
            CStr::from_ptr(text).to_string_lossy()
        };
        Err(fail(
            INTERNAL_ERROR,
            format!("Metal 4 commit failed ({code}): {text}"),
        ))
    }
}

pub(super) fn options(feedback: Arc<Feedback>) -> Result<Mtl4, Error> {
    unsafe {
        let options = Mtl4::owned(msg_send![class!(MTL4CommitOptions), new], "commit options")?;
        let handler = block::ConcreteBlock::new(move |native: *mut Object| {
            crate::boundary::native_scope(|| {
                let error: *mut Object = msg_send![native, error];
                feedback.complete(native_result(error));
            });
        })
        .copy();
        // addFeedbackHandler copies the block. Only its Arc crosses threads;
        // resource retirement stays on the externally serialized OGPU caller.
        let _: () = msg_send![options.0, addFeedbackHandler: &*handler];
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_success_and_cross_thread_failure() {
        let feedback = Arc::new(Feedback::default());
        assert!(feedback.observe(false).is_none());
        let writer = feedback.clone();
        let thread = std::thread::spawn(move || {
            writer.complete(Err(fail(INTERNAL_ERROR, "terminal failure")));
        });
        assert_eq!(
            feedback.observe(true).unwrap().unwrap_err().message,
            "terminal failure"
        );
        assert_eq!(
            feedback.observe(false).unwrap().unwrap_err().status,
            INTERNAL_ERROR
        );
        thread.join().unwrap();
        let success = Feedback::default();
        success.complete(Ok(()));
        assert!(success.observe(false).unwrap().is_ok());
    }

    #[test]
    fn nullable_native_error_becomes_owned_diagnostic() {
        let outcome = crate::boundary::native_scope(|| unsafe {
            assert!(native_result(ptr::null_mut()).is_ok());
            let domain: *mut Object = msg_send![class!(NSString),
                stringWithUTF8String: c"OGPUTestError".as_ptr()];
            let error: *mut Object = msg_send![class!(NSError),
                errorWithDomain: domain code: 42isize userInfo: ptr::null_mut::<Object>()];
            native_result(error)
        });
        // The NSError's autorelease pool has drained; the diagnostic survives.
        let error = outcome.unwrap_err();
        assert_eq!(error.status, INTERNAL_ERROR);
        assert_eq!(error.vk, 0);
        assert!(error.message.contains("42"));
        assert!(error.message.contains("OGPUTestError"));
    }
}
