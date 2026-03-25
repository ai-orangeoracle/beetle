//! Linux network operations for AP/STA（数据面均经 rtnetlink，无 `ip` 命令依赖）。
//! Linux network operations for AP/STA (data plane via rtnetlink, no `ip` dependency).

use crate::error::Result;

pub fn setup_ap_address(iface: &str, cidr: &str) -> Result<()> {
    super::net_rt::setup_ap_address(iface, cidr)
}

pub fn read_sta_ip(iface: &str) -> Result<Option<String>> {
    super::net_rt::read_sta_ip(iface)
}

pub fn ensure_root_or_cap_net_admin() -> Result<()> {
    super::net_rt::ensure_netlink_access()
}
