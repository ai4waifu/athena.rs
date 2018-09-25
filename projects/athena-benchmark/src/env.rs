//! 运行环境元数据捕获。

use std::process::Command;

use serde::Serialize;

/// 单次基准运行的环境快照。
#[derive(Debug, Clone, Serialize)]
pub struct BenchEnv {
    /// `git rev-parse HEAD`，失败则为 `null`。
    pub commit: Option<String>,
    /// `rustc --version` 首行。
    pub rustc: Option<String>,
    /// 编译目标三元组。
    pub target_triple: String,
    /// CPU 描述（尽力而为）。
    pub cpu: Option<String>,
    /// GPU 描述；Athena 默认路径通常为 `null`。
    pub gpu: Option<String>,
    /// 可用并行度（逻辑 CPU）。
    pub threads: usize,
    /// 是否启用 `jit` feature。
    pub jit_enabled: bool,
    /// Node 版本；本 crate 不依赖 Node，恒为 `null`。
    pub node: Option<String>,
}

impl BenchEnv {
    /// 捕获当前进程可见的环境信息。
    pub fn capture(jit_enabled: bool) -> Self {
        Self {
            commit: git_head(),
            rustc: rustc_version(),
            target_triple: std::env::var("TARGET")
                .unwrap_or_else(|_| format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)),
            cpu: cpu_brand(),
            gpu: None,
            threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
            jit_enabled,
            node: None,
        }
    }
}

fn git_head() -> Option<String> {
    let out = Command::new("git").args(["rev-parse", "HEAD"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn rustc_version() -> Option<String> {
    let out = Command::new("rustc").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn cpu_brand() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let out = Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name)"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return if s.is_empty() { None } else { Some(s) };
    }
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("model name") {
                let brand = rest.trim().trim_start_matches(':').trim();
                if !brand.is_empty() {
                    return Some(brand.to_string());
                }
            }
        }
        None
    }
    #[cfg(target_os = "macos")]
    {
        let out = Command::new("sysctl").args(["-n", "machdep.cpu.brand_string"]).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        None
    }
}
