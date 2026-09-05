//! Mach time-constraint thread policy for the realtime render worker
//! (03-audio-runtime-spec.md §线程模型: worker 使用 time-constraint 线程
//! 策略). Declared against libSystem directly; `mach2` would add a
//! dependency for four syscalls.

#![cfg(target_os = "macos")]

/// `mach_timebase_info_data_t`.
#[repr(C)]
struct MachTimebaseInfoData {
    numer: u32,
    denom: u32,
}

const THREAD_TIME_CONSTRAINT_POLICY: u32 = 2;
const THREAD_TIME_CONSTRAINT_POLICY_COUNT: u32 = 4;

extern "C" {
    fn mach_timebase_info(info: *mut MachTimebaseInfoData) -> i32;
    fn mach_task_self() -> u32;
    fn mach_thread_self() -> u32;
    fn mach_port_deallocate(task: u32, port: u32) -> i32;
    fn thread_policy_set(thread: u32, flavor: u32, info: *const u32, count: u32) -> i32;
}

fn nanos_to_absolute(nanos: u64) -> u64 {
    let mut info = MachTimebaseInfoData { numer: 1, denom: 1 };
    // SAFETY: `info` is a valid out-pointer; the call only writes into it.
    unsafe { mach_timebase_info(&mut info) };
    nanos * u64::from(info.denom) / u64::from(info.numer.max(1))
}

/// Apply `THREAD_TIME_CONSTRAINT_POLICY` to the calling thread. Durations
/// are clamped into `u32` absolute-time units. Returns false if the kernel
/// rejected the policy (the caller continues; scheduling priority is a
/// best-effort hint, not a correctness requirement).
pub fn set_time_constraint(
    period: std::time::Duration,
    computation: std::time::Duration,
    constraint: std::time::Duration,
) -> bool {
    let to_abs = |d: std::time::Duration| -> u32 {
        nanos_to_absolute(d.as_nanos() as u64).min(u64::from(u32::MAX)) as u32
    };
    // thread_time_constraint_policy_data_t layout.
    let policy = [
        to_abs(period),
        to_abs(computation),
        to_abs(constraint),
        1, // preemptible
    ];
    // SAFETY: `mach_thread_self` returns a send right for the calling
    // thread; `policy` points at THREAD_TIME_CONSTRAINT_POLICY_COUNT u32s;
    // the send right is deallocated exactly once below.
    unsafe {
        let thread = mach_thread_self();
        let status = thread_policy_set(
            thread,
            THREAD_TIME_CONSTRAINT_POLICY,
            policy.as_ptr(),
            THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        );
        mach_port_deallocate(mach_task_self(), thread);
        status == 0
    }
}
