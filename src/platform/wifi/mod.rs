//! WiFi：ESP 上 SoftAP+STA；host 上为桩。
//! WiFi: SoftAP+STA on ESP; stubs on host.

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
mod esp;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub use esp::*;

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
mod host;
#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
pub use host::*;
