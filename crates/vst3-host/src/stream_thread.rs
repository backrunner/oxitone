//! macOS scheduling for isolated helper/IO threads, never called by the audio port.
use std::time::Duration;

#[cfg(target_os = "macos")]
#[repr(C)]
struct Timebase {
    numer: u32,
    denom: u32,
}
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn mach_timebase_info(info: *mut Timebase) -> libc::kern_return_t;
    fn mach_absolute_time() -> u64;
    fn mach_wait_until(deadline: u64) -> libc::kern_return_t;
}
#[cfg(target_os = "macos")]
fn timebase() -> Option<Timebase> {
    let mut info = Timebase { numer: 0, denom: 0 };
    // SAFETY: a valid stack-owned out pointer.
    let status = unsafe { mach_timebase_info(&mut info) };
    (status == 0 && info.numer != 0 && info.denom != 0).then_some(info)
}

/// Best effort: a rejected policy is reported, never mistaken for a deadline guarantee.
pub(crate) fn configure(sample_rate: u32, block_size: usize) -> bool {
    #[cfg(target_os = "macos")]
    {
        let Some(info) = timebase() else { return false };
        let period = (block_size as u128 * 1_000_000_000 * u128::from(info.denom)
            / (u128::from(sample_rate) * u128::from(info.numer)))
        .clamp(2, u32::MAX as u128) as u32;
        let mut policy = libc::thread_time_constraint_policy {
            period,
            computation: period / 2,
            constraint: period,
            preemptible: 1,
        };
        // SAFETY: pthread_mach_thread_np returns a borrowed port, not an owned send right.
        // The policy has the ABI-defined size and remains valid for this synchronous call.
        unsafe {
            libc::thread_policy_set(
                libc::pthread_mach_thread_np(libc::pthread_self()),
                libc::THREAD_TIME_CONSTRAINT_POLICY as libc::thread_policy_flavor_t,
                (&mut policy as *mut libc::thread_time_constraint_policy).cast(),
                libc::THREAD_TIME_CONSTRAINT_POLICY_COUNT,
            ) == 0
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (sample_rate, block_size);
        false
    }
}

pub(crate) struct IdleWait {
    #[cfg(target_os = "macos")]
    ticks: Option<u64>,
}
impl IdleWait {
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            ticks: timebase()
                .map(|info| (100_000u64 * u64::from(info.denom) / u64::from(info.numer)).max(1)),
        }
    }
    pub(crate) fn wait(&self) {
        #[cfg(target_os = "macos")]
        if let Some(ticks) = self.ticks {
            // SAFETY: libSystem absolute-time wait; no pointers. Unlike ordinary sleep this
            // does not add a timer-coalescing window. Early interruption just repolls the queue.
            unsafe { mach_wait_until(mach_absolute_time().saturating_add(ticks)) };
            return;
        }
        std::thread::sleep(Duration::from_micros(100));
    }
}
