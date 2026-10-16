//! Atlas Daedalus — Host Telemetry & Resource Engine
//! (C) 2026 Atlas Research Labs — MIT License
//! Provides real host metrics: memory (total, free, process RSS), CPU count, and platform specs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostMetrics {
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
    pub process_rss_mb: u64,
    pub cpu_cores: usize,
    pub os_name: String,
    pub uptime_secs: u64,
}

pub struct HostInspector;

impl HostInspector {
    pub fn collect() -> HostMetrics {
        let cpu_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        let os_name = std::env::consts::OS.to_string();

        #[cfg(target_os = "windows")]
        let (total_memory_mb, available_memory_mb, process_rss_mb, uptime_secs) = {
            Self::collect_windows()
        };

        #[cfg(not(target_os = "windows"))]
        let (total_memory_mb, available_memory_mb, process_rss_mb, uptime_secs) = {
            Self::collect_unix()
        };

        HostMetrics {
            total_memory_mb,
            available_memory_mb,
            process_rss_mb,
            cpu_cores,
            os_name,
            uptime_secs,
        }
    }

    #[cfg(target_os = "windows")]
    fn collect_windows() -> (u64, u64, u64, u64) {
        #[allow(non_snake_case)]
        #[repr(C)]
        struct MEMORYSTATUSEX {
            dwLength: u32,
            dwMemoryLoad: u32,
            ullTotalPhys: u64,
            ullAvailPhys: u64,
            ullTotalPageFile: u64,
            ullAvailPageFile: u64,
            ullTotalVirtual: u64,
            ullAvailVirtual: u64,
            ullAvailExtendedVirtual: u64,
        }

        #[allow(non_snake_case)]
        #[repr(C)]
        struct PROCESS_MEMORY_COUNTERS {
            cb: u32,
            PageFaultCount: u32,
            PeakWorkingSetSize: usize,
            WorkingSetSize: usize,
            QuotaPeakPagedPoolUsage: usize,
            QuotaPagedPoolUsage: usize,
            QuotaPeakNonPagedPoolUsage: usize,
            QuotaNonPagedPoolUsage: usize,
            PagefileUsage: usize,
            PeakPagefileUsage: usize,
        }

        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalMemoryStatusEx(lpBuffer: *mut MEMORYSTATUSEX) -> i32;
            fn GetCurrentProcess() -> isize;
            fn GetTickCount64() -> u64;
            fn K32GetProcessMemoryInfo(
                hProcess: isize,
                ppmc: *mut PROCESS_MEMORY_COUNTERS,
                cb: u32,
            ) -> i32;
        }

        let mut mem = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            dwMemoryLoad: 0,
            ullTotalPhys: 0,
            ullAvailPhys: 0,
            ullTotalPageFile: 0,
            ullAvailPageFile: 0,
            ullTotalVirtual: 0,
            ullAvailVirtual: 0,
            ullAvailExtendedVirtual: 0,
        };

        let mut total = 0;
        let mut avail = 0;
        unsafe {
            if GlobalMemoryStatusEx(&mut mem) != 0 {
                total = mem.ullTotalPhys / (1024 * 1024);
                avail = mem.ullAvailPhys / (1024 * 1024);
            }
        }

        let mut rss = 0;
        unsafe {
            let mut pmc = PROCESS_MEMORY_COUNTERS {
                cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                PageFaultCount: 0,
                PeakWorkingSetSize: 0,
                WorkingSetSize: 0,
                QuotaPeakPagedPoolUsage: 0,
                QuotaPagedPoolUsage: 0,
                QuotaPeakNonPagedPoolUsage: 0,
                QuotaNonPagedPoolUsage: 0,
                PagefileUsage: 0,
                PeakPagefileUsage: 0,
            };
            let proc = GetCurrentProcess();
            if K32GetProcessMemoryInfo(proc, &mut pmc, pmc.cb) != 0 {
                rss = (pmc.WorkingSetSize as u64) / (1024 * 1024);
            }
        }

        let uptime = unsafe { GetTickCount64() / 1000 };

        (total, avail, rss, uptime)
    }

    #[cfg(not(target_os = "windows"))]
    fn collect_unix() -> (u64, u64, u64, u64) {
        use std::fs;

        let mut total = 0;
        let mut avail = 0;

        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(val) = extract_kb(line) {
                        total = val / 1024;
                    }
                } else if line.starts_with("MemAvailable:") {
                    if let Some(val) = extract_kb(line) {
                        avail = val / 1024;
                    }
                }
            }
        }

        let mut rss = 0;
        if let Ok(content) = fs::read_to_string("/proc/self/statm") {
            let parts: Vec<&str> = content.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(pages) = parts[1].parse::<u64>() {
                    // Page size is typically 4KB
                    rss = (pages * 4) / 1024;
                }
            }
        }

        let mut uptime = 0;
        if let Ok(content) = fs::read_to_string("/proc/uptime") {
            if let Some(first) = content.split_whitespace().next() {
                if let Ok(secs) = first.parse::<f64>() {
                    uptime = secs as u64;
                }
            }
        }

        (total, avail, rss, uptime)
    }
}

#[cfg(not(target_os = "windows"))]
fn extract_kb(line: &str) -> Option<u64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse::<u64>().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_host_metrics() {
        let m = HostInspector::collect();
        assert!(m.cpu_cores >= 1);
        assert!(!m.os_name.is_empty());
        assert!(m.total_memory_mb > 0);
    }
}
