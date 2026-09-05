//! Flush-to-zero / denormals-are-zero control and algorithmic denormal
//! guards. `set_ftz_daz`/`FtzGuard` are called from the control thread when
//! the audio worker starts; `flush_denormal*` are RT-safe per-sample guards
//! for recursive state (03-audio-runtime-spec.md §CPU 尖峰防线).

#[cfg(target_arch = "x86_64")]
const FTZ_DAZ_MASK: u32 = 0x8040;

/// Enable or disable FTZ (bit 15) and DAZ (bit 6) on the calling thread.
/// Control-thread only; call before entering the render loop.
#[cfg(target_arch = "x86_64")]
pub fn set_ftz_daz(enabled: bool) {
    // SAFETY: touches only the calling thread's MXCSR; no memory is accessed
    // and the register is fully restored by `FtzGuard` users.
    unsafe {
        let mut csr = core::arch::x86_64::_mm_getcsr();
        if enabled {
            csr |= FTZ_DAZ_MASK;
        } else {
            csr &= !FTZ_DAZ_MASK;
        }
        core::arch::x86_64::_mm_setcsr(csr);
    }
}

/// aarch64 flushes denormals in hardware at full speed; nothing to set.
#[cfg(not(target_arch = "x86_64"))]
pub fn set_ftz_daz(_enabled: bool) {}

/// Whether FTZ+DAZ are active on this thread. Always true on aarch64.
#[cfg(target_arch = "x86_64")]
pub fn ftz_daz_enabled() -> bool {
    // SAFETY: read-only access to the calling thread's MXCSR.
    unsafe { core::arch::x86_64::_mm_getcsr() & FTZ_DAZ_MASK == FTZ_DAZ_MASK }
}

/// aarch64 flushes denormals in hardware at full speed.
#[cfg(not(target_arch = "x86_64"))]
pub fn ftz_daz_enabled() -> bool {
    true
}

/// RAII guard that enables FTZ/DAZ on the current thread and restores the
/// previous MXCSR on drop. Control-thread only (create before the render
/// loop of a worker thread, drop when the thread exits).
pub struct FtzGuard {
    #[cfg(target_arch = "x86_64")]
    saved: u32,
}

impl FtzGuard {
    pub fn new() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            // SAFETY: reads/writes only the calling thread's MXCSR.
            let saved = unsafe { core::arch::x86_64::_mm_getcsr() };
            set_ftz_daz(true);
            Self { saved }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            Self {}
        }
    }
}

impl Default for FtzGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FtzGuard {
    fn drop(&mut self) {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: writes only the calling thread's MXCSR, restoring the value
        // observed by `new` on the same thread.
        unsafe {
            core::arch::x86_64::_mm_setcsr(self.saved);
        }
    }
}

/// Flush an `f32` subnormal to zero, preserving sign-free normal values.
/// RT-safe.
#[inline]
pub fn flush_denormal(x: f32) -> f32 {
    if x.to_bits() & 0x7f80_0000 == 0 {
        0.0
    } else {
        x
    }
}

/// Flush an `f64` subnormal to zero. RT-safe.
#[inline]
pub fn flush_denormal_f64(x: f64) -> f64 {
    if x.to_bits() & 0x7ff0_0000_0000_0000 == 0 {
        0.0
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_enables_ftz_daz() {
        let _guard = FtzGuard::new();
        assert!(ftz_daz_enabled());
    }

    #[test]
    fn flush_kills_subnormals_only() {
        assert_eq!(flush_denormal(1e-42), 0.0);
        assert_eq!(flush_denormal(-1e-42), 0.0);
        assert_eq!(flush_denormal(1e-37), 1e-37);
        assert_eq!(flush_denormal(0.0), 0.0);
        assert_eq!(flush_denormal_f64(1e-320), 0.0);
        assert_eq!(flush_denormal_f64(1e-300), 1e-300);
    }
}
