//! Direct IOKit FFI for Apple Silicon GPU unified memory.
//!
//! Reads `PerformanceStatistics` → `In use system memory` from the IOAccelerator
//! service — same data Activity Monitor displays. No subprocess calls.
//!
//! Returns `None` on non-Apple-Silicon or if IOKit query fails (trust > accuracy).

/// Read GPU "In use system memory" from IOKit IOAccelerator service.
///
/// Direct FFI — no subprocess. ~10μs. Returns None on failure or non-Apple-Silicon.
pub fn gpu_alloc_bytes() -> Option<u64> {
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        None
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        gpu_alloc_bytes_iokit()
    }
}

/// Total unified memory available to the GPU.
///
/// On Apple Silicon, GPU and CPU share the same physical memory.
/// Returns `sysinfo::System::total_memory()` which is the unified pool.
pub fn total_gpu_memory_bytes() -> Option<u64> {
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        None
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        let sys = sysinfo::System::new_all();
        Some(sys.total_memory())
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn gpu_alloc_bytes_iokit() -> Option<u64> {
    use core_foundation::base::TCFType;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::{kCFAllocatorDefault, CFRelease};
    use core_foundation_sys::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
    use core_foundation_sys::number::CFNumberRef;

    // Raw IOKit FFI — avoids IOKit-sys/CoreFoundation-sys type conflicts.
    type IOReturn = i32;
    const KERN_SUCCESS: IOReturn = 0;

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOServiceMatching(
            name: *const libc::c_char,
        ) -> *mut core_foundation_sys::dictionary::__CFDictionary;
        fn IOServiceGetMatchingServices(
            main_port: u32,
            matching: *mut core_foundation_sys::dictionary::__CFDictionary,
            existing: *mut u32,
        ) -> IOReturn;
        fn IOIteratorNext(iterator: u32) -> u32;
        fn IORegistryEntryCreateCFProperties(
            entry: u32,
            properties: *mut *mut core_foundation_sys::dictionary::__CFDictionary,
            allocator: *const libc::c_void,
            options: u32,
        ) -> IOReturn;
        fn IOObjectRelease(object: u32) -> IOReturn;
        static kIOMasterPortDefault: u32;
    }

    unsafe {
        let matching = IOServiceMatching(c"IOAccelerator".as_ptr());
        if matching.is_null() {
            return None;
        }

        let mut iterator: u32 = 0;
        let kr = IOServiceGetMatchingServices(kIOMasterPortDefault, matching, &mut iterator);
        if kr != KERN_SUCCESS {
            return None;
        }

        let mut total_alloc: u64 = 0;
        loop {
            let service = IOIteratorNext(iterator);
            if service == 0 {
                break;
            }

            let mut props: *mut core_foundation_sys::dictionary::__CFDictionary =
                std::ptr::null_mut();
            let kr = IORegistryEntryCreateCFProperties(
                service,
                &mut props,
                kCFAllocatorDefault as *const _,
                0,
            );
            IOObjectRelease(service);

            if kr != KERN_SUCCESS || props.is_null() {
                continue;
            }

            let props_ref = props as CFDictionaryRef;

            // Navigate: props → "PerformanceStatistics" → "In use system memory"
            let perf_key = CFString::new("PerformanceStatistics");
            let perf_val =
                CFDictionaryGetValue(props_ref, perf_key.as_concrete_TypeRef() as *const _);
            if !perf_val.is_null() {
                let perf_dict = perf_val as CFDictionaryRef;
                let mem_key = CFString::new("In use system memory");
                let mem_val =
                    CFDictionaryGetValue(perf_dict, mem_key.as_concrete_TypeRef() as *const _);
                if !mem_val.is_null() {
                    let num = CFNumber::wrap_under_get_rule(mem_val as CFNumberRef);
                    if let Some(val) = num.to_i64() {
                        total_alloc += val as u64;
                    }
                }
            }

            CFRelease(props as *const _);
        }
        IOObjectRelease(iterator);

        if total_alloc > 0 {
            Some(total_alloc)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_alloc_bytes_returns_some_on_apple_silicon() {
        if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
            let result = gpu_alloc_bytes();
            assert!(
                result.is_some(),
                "Apple Silicon should return GPU alloc bytes"
            );
            let bytes = result.unwrap();
            assert!(bytes > 1_000_000, "GPU alloc should be > 1MB, got {bytes}");
            assert!(
                bytes < 256_000_000_000,
                "GPU alloc suspiciously high: {bytes}"
            );
        }
    }

    #[test]
    fn total_gpu_memory_returns_some_on_apple_silicon() {
        if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
            let result = total_gpu_memory_bytes();
            assert!(result.is_some());
            let bytes = result.unwrap();
            assert!(bytes >= 8_000_000_000, "Total GPU mem too low: {bytes}");
        }
    }
}
