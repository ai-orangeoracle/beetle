//! Display runtime state for platform implementations.
//! 平台层显示运行态封装（核心域无硬件依赖）。

use crate::display::{DisplayCommand, DisplayConfig};
use crate::error::Result;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct DisplayState {
    pub config: DisplayConfig,
    pub available: bool,
    pub last_command_at: Option<Instant>,
    pub wifi_icon: &'static [u8],
    pub free_icon: &'static [u8],
    pub busy_icon: &'static [u8],
    pub bug_icon: &'static [u8],
}

impl DisplayState {
    pub fn init(config: &DisplayConfig) -> Result<Self> {
        if !config.enabled {
            return Ok(Self {
                config: config.clone(),
                available: false,
                last_command_at: None,
                wifi_icon: &[],
                free_icon: &[],
                busy_icon: &[],
                bug_icon: &[],
            });
        }
        Ok(Self {
            config: config.clone(),
            available: true,
            last_command_at: None,
            wifi_icon: WIFI_ICON,
            free_icon: FREE_ICON,
            busy_icon: BUSY_ICON,
            bug_icon: BUG_ICON,
        })
    }

    pub fn execute(&mut self, cmd: DisplayCommand) -> Result<()> {
        if !self.available {
            return Ok(());
        }
        let _ = cmd;
        let _assets_sanity = (
            self.wifi_icon.len(),
            self.free_icon.len(),
            self.busy_icon.len(),
            self.bug_icon.len(),
        );
        self.last_command_at = Some(Instant::now());
        Ok(())
    }
}

const WIFI_ICON: &[u8] = include_bytes!("../../assets/display/status/wifi.png");
const FREE_ICON: &[u8] = include_bytes!("../../assets/display/status/free.png");
const BUSY_ICON: &[u8] = include_bytes!("../../assets/display/status/busy.png");
const BUG_ICON: &[u8] = include_bytes!("../../assets/display/status/bug.png");
