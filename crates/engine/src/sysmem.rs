//! System memory for the default budget, and process counters for the profiler.

/// Fallback when the size cannot be read: 8 GiB.
const FALLBACK: u64 = 8 << 30;

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    #[derive(Default)]
    #[repr(C)]
    pub struct FileTime {
        low: u32,
        high: u32,
    }

    impl FileTime {
        pub fn ns(&self) -> u64 {
            ((u64::from(self.high) << 32) | u64::from(self.low)) * 100
        }
    }

    #[derive(Default)]
    #[repr(C)]
    pub struct ProcessMemoryCounters {
        pub cb: u32,
        page_fault_count: u32,
        pub peak_working_set_size: usize,
        pub working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        pub peak_pagefile_usage: usize,
    }

    #[repr(C)]
    pub struct MemoryStatusEx {
        pub length: u32,
        pub memory_load: u32,
        pub total_phys: u64,
        pub avail_phys: u64,
        pub total_page_file: u64,
        pub avail_page_file: u64,
        pub total_virtual: u64,
        pub avail_virtual: u64,
        pub avail_extended_virtual: u64,
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn GlobalMemoryStatusEx(buf: *mut MemoryStatusEx) -> i32;
        pub fn GetCurrentProcess() -> *mut c_void;
        pub fn GetProcessTimes(
            process: *mut c_void,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        pub fn K32GetProcessMemoryInfo(
            process: *mut c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }
}

/// Total physical memory in bytes (`GlobalMemoryStatusEx`), or 8 GiB when it cannot be read.
#[cfg(windows)]
pub fn total_ram() -> u64 {
    let mut m = win::MemoryStatusEx {
        length: std::mem::size_of::<win::MemoryStatusEx>() as u32,
        memory_load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page_file: 0,
        avail_page_file: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended_virtual: 0,
    };
    // SAFETY: `m` is a correctly sized, writable MEMORYSTATUSEX with `dwLength` set, as the API requires.
    if unsafe { win::GlobalMemoryStatusEx(&mut m) } != 0 && m.total_phys > 0 {
        m.total_phys
    } else {
        FALLBACK
    }
}

/// Total physical memory in bytes; the tool targets Windows only, so other hosts get the fallback.
#[cfg(not(windows))]
pub fn total_ram() -> u64 {
    FALLBACK
}

#[derive(Default)]
pub(crate) struct ProcessCounters {
    pub user_cpu_ns: u64,
    pub kernel_cpu_ns: u64,
    pub peak_working_set: u64,
    pub peak_commit: u64,
    pub working_set: u64,
}

#[cfg(windows)]
pub(crate) fn process_counters() -> ProcessCounters {
    let mut result = ProcessCounters::default();
    let (mut created, mut exited, mut kernel, mut user) = Default::default();
    let mut memory = win::ProcessMemoryCounters::default();
    let size = std::mem::size_of_val(&memory) as u32;
    memory.cb = size;
    // SAFETY: the pseudo-handle needs no close. Each output has the API's layout and is writable.
    unsafe {
        let process = win::GetCurrentProcess();
        if win::GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) != 0 {
            result.user_cpu_ns = user.ns();
            result.kernel_cpu_ns = kernel.ns();
        }
        if win::K32GetProcessMemoryInfo(process, &mut memory, size) != 0 {
            result.working_set = memory.working_set_size as u64;
            result.peak_working_set = memory.peak_working_set_size as u64;
            result.peak_commit = memory.peak_pagefile_usage as u64;
        }
    }
    result
}

#[cfg(not(windows))]
pub(crate) fn process_counters() -> ProcessCounters {
    ProcessCounters::default()
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn process_counters_are_plausible() {
        let pages = vec![1u8; 64 << 20];
        std::hint::black_box(&pages);
        let c = super::process_counters();
        assert!(c.peak_working_set >= 64 << 20);
        assert!(c.peak_commit >= 64 << 20);
        assert!(c.user_cpu_ns + c.kernel_cpu_ns > 0);
    }

    #[test]
    fn total_ram_is_plausible() {
        let r = super::total_ram();
        assert!(r >= 256 << 20, "{r}");
    }
}
