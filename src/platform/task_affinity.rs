//! ESP 线程绑核执行面：统一封装 pthread cfg，避免各处直接操作底层 API。
//! Unified ESP thread affinity surface for safe/centralized pthread cfg handling.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskCore {
    Core0,
    Core1,
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
mod imp {
    use super::TaskCore;
    use esp_idf_hal::cpu::Core;
    use esp_idf_hal::task::thread::ThreadSpawnConfiguration;
    use std::io;
    use std::sync::Mutex;

    static PTHREAD_CFG_LOCK: Mutex<()> = Mutex::new(());

    fn map_core(core: TaskCore) -> Core {
        match core {
            TaskCore::Core0 => Core::Core0,
            TaskCore::Core1 => Core::Core1,
        }
    }

    fn io_other(msg: impl Into<String>) -> io::Error {
        io::Error::other(msg.into())
    }

    pub fn spawn_named_with_affinity<F>(
        name: String,
        stack_size: usize,
        core: Option<TaskCore>,
        f: F,
    ) -> io::Result<std::thread::JoinHandle<()>>
    where
        F: FnOnce() + Send + 'static,
    {
        if core.is_none() {
            return std::thread::Builder::new()
                .name(name)
                .stack_size(stack_size)
                .spawn(f);
        }

        let _guard = PTHREAD_CFG_LOCK
            .lock()
            .map_err(|e| io_other(format!("task_affinity lock poisoned: {}", e)))?;
        let mut cfg = ThreadSpawnConfiguration::get().unwrap_or_default();
        cfg.pin_to_core = core.map(map_core);
        cfg.inherit = false;
        cfg.set()
            .map_err(|e| io_other(format!("task_affinity set cfg failed: {}", e)))?;

        let spawn_result = std::thread::Builder::new()
            .name(name)
            .stack_size(stack_size)
            .spawn(f);

        let restore_result = ThreadSpawnConfiguration::default().set();
        if let Err(e) = restore_result {
            log::warn!("[task_affinity] restore pthread cfg failed: {}", e);
        }

        spawn_result
    }
}

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
mod imp {
    use super::TaskCore;
    use std::io;

    pub fn spawn_named_with_affinity<F>(
        name: String,
        stack_size: usize,
        _core: Option<TaskCore>,
        f: F,
    ) -> io::Result<std::thread::JoinHandle<()>>
    where
        F: FnOnce() + Send + 'static,
    {
        std::thread::Builder::new()
            .name(name)
            .stack_size(stack_size)
            .spawn(f)
    }
}

pub use imp::spawn_named_with_affinity;
