//! WSS 网关入站抽象：协议驱动 trait + 传输 trait + 统一循环。
//! 仅依赖 bus、error、ChannelHttpClient；不依赖 platform/esp_idf。
//! 扩展新通道：实现 WssGatewayDriver，在 ESP 上提供 WssConnection 实现并调用 run_wss_gateway_loop。

mod connection;
mod driver;
mod r#loop;

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
mod esp_conn;

#[allow(unused_imports)]
pub use connection::{WssConnection, WssEvent};
pub use driver::{WssGatewayDriver, WssRecvAction, WssSessionState};
pub use r#loop::run_wss_gateway_loop;

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
#[allow(unused_imports)]
pub use esp_conn::{connect_esp_wss, EspWssConnection};
