//! Linux 网络栈：STA IPv4 读取与 AP 地址配置，均经 rtnetlink（NETLINK_ROUTE），无 `ip` 子进程。
//! Linux networking: STA IPv4 read and AP address setup via rtnetlink (NETLINK_ROUTE), no `ip` subprocess.

use crate::error::{Error, Result};
use futures::stream::TryStreamExt;
use rtnetlink::packet_route::{
    address::{AddressAttribute, AddressMessage},
    AddressFamily,
};
use rtnetlink::{new_connection, Handle, LinkUnspec};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::OnceLock;
use tokio::runtime::Runtime;

static RTNETLINK_RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn rtnetlink_runtime() -> Result<&'static Runtime> {
    RTNETLINK_RUNTIME.get_or_try_init(|| {
        // `Runtime::new()` 需 `rt-multi-thread`；嵌入式用 current_thread 即可驱动 rtnetlink + spawn(connection)。
        // `Runtime::new()` needs `rt-multi-thread`; current_thread is enough for rtnetlink + spawn(connection).
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::Other {
                source: Box::new(e),
                stage: "rtnetlink_runtime_init",
            })
    })
}

fn map_rt_stage(e: rtnetlink::Error, stage: &'static str) -> Error {
    Error::Other {
        source: Box::new(e),
        stage,
    }
}

/// 与 `ip -4 -o addr show dev <iface>` 首条 IPv4 语义对齐：按 netlink 返回顺序取首个可用 IPv4。
/// Aligns with the first IPv4 line from `ip -4 -o addr show dev <iface>`.
pub fn read_sta_ip(iface: &str) -> Result<Option<String>> {
    rtnetlink_runtime()?.block_on(read_sta_ip_async(iface))
}

async fn read_sta_ip_async(iface: &str) -> Result<Option<String>> {
    let (connection, handle, _) = new_connection().map_err(|e| Error::io("wifi_sta_ip", e))?;
    tokio::spawn(connection);

    let mut links = handle.link().get().match_name(iface.to_string()).execute();
    let Some(link) = links.try_next().await.map_err(|e| map_rt_stage(e, "wifi_sta_ip"))? else {
        return Ok(None);
    };

    let mut addresses = handle
        .address()
        .get()
        .set_link_index_filter(link.header.index)
        .execute();

    while let Some(msg) = addresses
        .try_next()
        .await
        .map_err(|e| map_rt_stage(e, "wifi_sta_ip"))?
    {
        if msg.header.family != AddressFamily::Inet {
            continue;
        }
        if let Some(v4) = first_ipv4_from_message(&msg) {
            return Ok(Some(v4.to_string()));
        }
    }
    Ok(None)
}

/// 等价于 `ip addr flush dev` + `ip addr add CIDR dev` + `ip link set dev up`（IPv4 AP 段）。
/// Equivalent to `ip addr flush dev` + `ip addr add CIDR dev` + `ip link set dev up` for IPv4 AP.
pub fn setup_ap_address(iface: &str, cidr: &str) -> Result<()> {
    rtnetlink_runtime()?.block_on(setup_ap_address_async(iface, cidr))
}

async fn setup_ap_address_async(iface: &str, cidr: &str) -> Result<()> {
    let (ipv4, prefix) = parse_ipv4_cidr(cidr)?;
    let (connection, handle, _) = new_connection().map_err(|e| Error::io("wifi_ap_ip_flush", e))?;
    tokio::spawn(connection);

    let mut links = handle.link().get().match_name(iface.to_string()).execute();
    let Some(link) = links
        .try_next()
        .await
        .map_err(|e| map_rt_stage(e, "wifi_ap_ip_flush"))?
    else {
        return Err(Error::config("wifi_ap_ip_flush", "interface not found"));
    };
    let index = link.header.index;

    flush_iface_addresses(&handle, index).await?;

    handle
        .address()
        .add(index, IpAddr::V4(ipv4), prefix)
        .execute()
        .await
        .map_err(|e| map_rt_stage(e, "wifi_ap_ip_add"))?;

    handle
        .link()
        .set(LinkUnspec::new_with_index(index).up().build())
        .execute()
        .await
        .map_err(|e| map_rt_stage(e, "wifi_ap_link_up"))?;

    Ok(())
}

async fn flush_iface_addresses(handle: &Handle, index: u32) -> Result<()> {
    let mut addresses = handle
        .address()
        .get()
        .set_link_index_filter(index)
        .execute();
    while let Some(addr) = addresses
        .try_next()
        .await
        .map_err(|e| map_rt_stage(e, "wifi_ap_ip_flush"))?
    {
        handle
            .address()
            .del(addr)
            .execute()
            .await
            .map_err(|e| map_rt_stage(e, "wifi_ap_ip_flush"))?;
    }
    Ok(())
}

fn parse_ipv4_cidr(s: &str) -> Result<(Ipv4Addr, u8)> {
    let mut parts = s.split('/');
    let addr_s = parts
        .next()
        .ok_or_else(|| Error::config("wifi_ap_ip", "invalid CIDR"))?;
    let prefix_s = parts
        .next()
        .ok_or_else(|| Error::config("wifi_ap_ip", "invalid CIDR"))?;
    if parts.next().is_some() {
        return Err(Error::config("wifi_ap_ip", "invalid CIDR"));
    }
    let addr: Ipv4Addr = addr_s
        .parse()
        .map_err(|_| Error::config("wifi_ap_ip", "invalid IPv4"))?;
    let prefix: u8 = prefix_s
        .parse()
        .map_err(|_| Error::config("wifi_ap_ip", "invalid prefix"))?;
    if prefix > 32 {
        return Err(Error::config("wifi_ap_ip", "invalid prefix"));
    }
    Ok((addr, prefix))
}

/// 与 `ip link show` 等价权限探测：能打开 rtnetlink 并 dump 一条链路即视为具备网络配置能力。
/// Permission probe equivalent to `ip link show`: open rtnetlink and dump one link.
pub fn ensure_netlink_access() -> Result<()> {
    rtnetlink_runtime().block_on(ensure_netlink_access_async())
}

async fn ensure_netlink_access_async() -> Result<()> {
    let (connection, handle, _) = new_connection().map_err(|e| Error::io("wifi_permission", e))?;
    tokio::spawn(connection);
    let mut links = handle.link().get().execute();
    let _ = links
        .try_next()
        .await
        .map_err(|e| map_rt_stage(e, "wifi_permission"))?;
    Ok(())
}

fn first_ipv4_from_message(msg: &AddressMessage) -> Option<Ipv4Addr> {
    let mut locals: Vec<Ipv4Addr> = Vec::new();
    let mut addrs: Vec<Ipv4Addr> = Vec::new();
    for attr in &msg.attributes {
        match attr {
            AddressAttribute::Local(IpAddr::V4(ip)) => locals.push(*ip),
            AddressAttribute::Address(IpAddr::V4(ip)) => addrs.push(*ip),
            _ => {}
        }
    }
    for v in locals.iter().chain(addrs.iter()) {
        if !v.is_loopback() {
            return Some(*v);
        }
    }
    locals.first().or(addrs.first()).copied()
}
