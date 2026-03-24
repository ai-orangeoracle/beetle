# Linux 部署随带的 WiFi 工具（可选） / Bundled WiFi tools (optional)

甲壳虫在 Linux 上会执行 **`iw`、`hostapd`、`dnsmasq`** 完成热点与配网。若设备上**没有**这些命令（精简固件、无包管理器等情况很常见），可把**与设备匹配**的可执行文件放在本目录下对应架构子文件夹里，由 **`deploy-linux.sh`** 一并上传到设备的 **`/opt/beetle/bin/`**。运行中的甲壳虫会**优先使用该路径**，再回退到系统 `PATH`。

On Linux, beetle invokes **`iw`**, **`hostapd`**, and **`dnsmasq`**. If they are missing on the device (common on **trimmed images without a package manager**), place matching binaries under **`packaging/linux/embed-deps/<arch>/`**; **`./deploy-linux.sh`** copies them to **`/opt/beetle/bin/`**, which beetle checks **before** `PATH`.

用户向操作说明见 **[docs/zh-cn/deploy-linux.md](../../docs/zh-cn/deploy-linux.md)**（中文）与 **[docs/en-us/deploy-linux.md](../../docs/en-us/deploy-linux.md)**（英文）。

---

## 目录结构 / Layout

| 子目录 / Subdir | 对应 Rust 构建目标 / Build target |
|-----------------|-------------------------------------|
| `armv7/`        | `armv7-unknown-linux-musleabihf`    |
| `aarch64/`      | `aarch64-unknown-linux-musl`        |
| `x86_64/`       | `x86_64-unknown-linux-musl`         |

文件名建议即为 `iw`、`hostapd`、`dnsmasq`。大文件不必提交 git，可用发行包或 CI 产物。

---

## ABI 说明（重要）

甲壳虫自身多为 **musl** 静态链接；**这三个工具必须与设备 rootfs 一致**（常见为 **glibc + 设备 CPU**）。若拷贝后执行报「找不到文件」或动态链接错误，说明架构或 libc 不匹配，需从**同一套固件/SDK** 或能在该板子上运行的环境取得二进制。

---

## 获取方式示例

- 在厂商 SDK / 镜像构建系统里启用对应软件包，烧录后进板子复制 `/usr/sbin/iw` 等；或  
- 使用项目 **Release** 里提供的依赖包（若有），解压到对应 `embed-deps/<arch>/`。

---

## 维护者备注

- 与「是否 Buildroot / Yocto」**无强绑定**：只是嵌入式里常见用这类工具生成裁剪 rootfs；用户只需关心 **工具能否在目标机运行**。
