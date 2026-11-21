//! PCRE2 JIT code is immutable after compilation. Match data, contexts and JIT
//! stacks are private to a worker; no native mutable state is shared by matches.
use pcre2_sys as ffi;
use std::{cell::RefCell, ffi::c_void, ptr::NonNull, sync::Arc};
use thread_local::ThreadLocal;

type Span = Option<(usize, usize)>;

#[derive(Debug)]
struct Code(NonNull<c_void>);

// SAFETY: PCRE2 explicitly permits simultaneous matching of compiled code.
// JIT compilation finishes before publishing this immutable code allocation.
unsafe impl Send for Code {}
unsafe impl Sync for Code {}

impl Drop for Code {
    fn drop(&mut self) {
        // SAFETY: this is the unique owner of a successful pcre2_compile result.
        unsafe {
            ffi::pcre2_code_free_8(self.0.as_ptr().cast());
        }
    }
}

#[derive(Debug)]
struct Scratch {
    data: NonNull<c_void>,
    context: NonNull<c_void>,
    stack: NonNull<c_void>,
    spans: Vec<Span>,
    // Keep code live until after every native scratch allocation is released.
    _code: Arc<Code>,
}

// SAFETY: scratch owns independent allocations. It is accessed only through a
// worker-local RefCell; moving or dropping these allocations on another thread
// is supported by PCRE2's default allocator.
unsafe impl Send for Scratch {}

impl Scratch {
    fn new(code: Arc<Code>, groups: usize) -> Result<Self, String> {
        // SAFETY: code was successfully compiled and remains owned by `code`.
        unsafe {
            let data = NonNull::new(
                ffi::pcre2_match_data_create_from_pattern_8(
                    code.0.as_ptr().cast(),
                    std::ptr::null_mut(),
                )
                .cast::<c_void>(),
            )
            .ok_or("PCRE2 match-data allocation failed")?;
            let Some(context) = NonNull::new(
                ffi::pcre2_match_context_create_8(std::ptr::null_mut()).cast::<c_void>(),
            ) else {
                ffi::pcre2_match_data_free_8(data.as_ptr().cast());
                return Err("PCRE2 match-context allocation failed".into());
            };
            let Some(stack) = NonNull::new(
                ffi::pcre2_jit_stack_create_8(32 * 1024, 1024 * 1024, std::ptr::null_mut())
                    .cast::<c_void>(),
            ) else {
                ffi::pcre2_match_data_free_8(data.as_ptr().cast());
                ffi::pcre2_match_context_free_8(context.as_ptr().cast());
                return Err("PCRE2 JIT-stack allocation failed".into());
            };
            ffi::pcre2_jit_stack_assign_8(context.as_ptr().cast(), None, stack.as_ptr());
            // Bound native work before using the existing bounded Rust engine
            // if PCRE2 reports a work/stack limit instead of a match result.
            ffi::pcre2_set_match_limit_8(context.as_ptr().cast(), 1_000_000);
            Ok(Self {
                data,
                context,
                stack,
                spans: vec![None; groups],
                _code: code,
            })
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // SAFETY: the allocations are independent, uniquely owned, and no match
        // can still be using them while this worker-local value is dropped.
        unsafe {
            ffi::pcre2_match_data_free_8(self.data.as_ptr().cast());
            ffi::pcre2_match_context_free_8(self.context.as_ptr().cast());
            ffi::pcre2_jit_stack_free_8(self.stack.as_ptr().cast());
        }
    }
}

#[derive(Debug)]
pub(crate) struct Pcre2 {
    code: Arc<Code>,
    groups: usize,
    pub(crate) jit_bytes: usize,
    scratch: ThreadLocal<RefCell<Option<Scratch>>>,
}

impl Pcre2 {
    pub(crate) fn compile(pattern: &str, expected_groups: usize) -> Result<Self, String> {
        let pattern = super::translate::translate(pattern)?;
        let mut error = 0;
        let mut offset = 0;
        // SAFETY: pattern is valid UTF-8 and its buffer remains live for compile.
        // All output pointers refer to properly aligned local storage.
        unsafe {
            let raw = ffi::pcre2_compile_8(
                pattern.as_ptr(),
                pattern.len(),
                ffi::PCRE2_UTF | ffi::PCRE2_UCP | ffi::PCRE2_DOLLAR_ENDONLY,
                &raw mut error,
                &raw mut offset,
                std::ptr::null_mut(),
            );
            let code = Arc::new(Code(
                NonNull::new(raw.cast::<c_void>())
                    .ok_or_else(|| format!("PCRE2 compile error {error} at byte {offset}"))?,
            ));
            let result = ffi::pcre2_jit_compile_8(code.0.as_ptr().cast(), ffi::PCRE2_JIT_COMPLETE);
            if result != 0 {
                return Err(format!("PCRE2 JIT compilation failed: {result}"));
            }
            let mut groups = 0u32;
            if ffi::pcre2_pattern_info_8(
                code.0.as_ptr().cast(),
                ffi::PCRE2_INFO_CAPTURECOUNT,
                (&raw mut groups).cast(),
            ) != 0
                || groups as usize + 1 != expected_groups
            {
                return Err("PCRE2 and Rust capture layouts differ".into());
            }
            let mut jit_bytes = 0usize;
            if ffi::pcre2_pattern_info_8(
                code.0.as_ptr().cast(),
                ffi::PCRE2_INFO_JITSIZE,
                (&raw mut jit_bytes).cast(),
            ) != 0
                || jit_bytes == 0
            {
                return Err("PCRE2 produced no JIT code".into());
            }
            Ok(Self {
                code,
                groups: expected_groups,
                jit_bytes,
                scratch: ThreadLocal::new(),
            })
        }
    }

    pub(crate) fn with_captures<R>(
        &self,
        subject: &str,
        start: usize,
        f: impl FnOnce(&[Span]) -> R,
    ) -> Result<Option<R>, String> {
        if start > subject.len() || !subject.is_char_boundary(start) {
            return Err("PCRE2 start offset is not a UTF-8 boundary".into());
        }
        let local = self.scratch.get_or(|| RefCell::new(None));
        let mut independent;
        let mut borrowed;
        let scratch = if let Ok(guard) = local.try_borrow_mut() {
            borrowed = guard;
            if borrowed.is_none() {
                *borrowed = Some(Scratch::new(self.code.clone(), self.groups)?);
            }
            borrowed.as_mut().expect("scratch initialized")
        } else {
            // Closure-based regex replacement can reenter the same pattern.
            independent = Scratch::new(self.code.clone(), self.groups)?;
            &mut independent
        };
        // SAFETY: subject is valid UTF-8 and start is a checked UTF-8 boundary.
        // Compiled code is immutable and every writable native argument belongs
        // to this exclusively borrowed scratch. Direct JIT has no interpreter
        // fallback. The callback sees copied offsets, never native pointers.
        unsafe {
            let result = ffi::pcre2_jit_match_8(
                self.code.0.as_ptr().cast(),
                subject.as_ptr(),
                subject.len(),
                start,
                0,
                scratch.data.as_ptr().cast(),
                scratch.context.as_ptr().cast(),
            );
            #[cfg(test)]
            super::record_jit();
            if result == ffi::PCRE2_ERROR_NOMATCH {
                return Ok(None);
            }
            if result <= 0 {
                return Err(format!("PCRE2 JIT match failed: {result}"));
            }
            let offsets = std::slice::from_raw_parts(
                ffi::pcre2_get_ovector_pointer_8(scratch.data.as_ptr().cast()),
                self.groups * 2,
            );
            for (i, span) in scratch.spans.iter_mut().enumerate() {
                let a = offsets[2 * i];
                let b = offsets[2 * i + 1];
                if a == usize::MAX {
                    *span = None;
                } else if a <= b
                    && b <= subject.len()
                    && subject.is_char_boundary(a)
                    && subject.is_char_boundary(b)
                {
                    *span = Some((a, b));
                } else {
                    return Err("PCRE2 returned an invalid capture span".into());
                }
            }
        }
        Ok(Some(f(&scratch.spans)))
    }
}
