//! 状态根文件系统实现（按目标平台分模块）。
//! State root filesystem implementations per target.

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
mod esp32;
#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub(crate) use esp32::Esp32StateFs;
