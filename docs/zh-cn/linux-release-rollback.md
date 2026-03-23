# Linux 发布目录与回滚

[English](../en-us/linux-release-rollback.md) | **中文**

本文说明在低端 Linux SBC 上部署 **beetle** 的推荐目录布局、版本切换与回滚步骤；权限与目录基线与项目迁移计划中的运维约定一致（不引用内部文档路径）。

## 1. 目录约定（推荐）

| 路径 | 用途 |
|------|------|
| `/opt/beetle/releases/<version>/` | 某一发布包解压后的内容（含 `beetle`、`beetle.service`、`README.txt` 等） |
| `/opt/beetle/current` | 指向当前运行版本的**符号链接** |
| `/var/lib/beetle` 或 `/data/beetle` | 运行时状态与配置（由 `BEETLE_STATE_ROOT` 或默认路径决定，实现见 `src/platform/state_root.rs`） |

`<version>` 与发布包名一致，例如 `v0.1.0`（与 Git tag、`Cargo.toml` 的 `package.version` 对齐）。

## 2. 首次安装

1. 校验发布页附带的 `SHA256SUMS`。
2. 创建目录：`sudo mkdir -p /opt/beetle/releases/vX.Y.Z`。
3. 将 `beetle-vX.Y.Z-linux-<arch>-musl.tar.gz` 解压到该目录（顶层目录名与压缩包 basename 一致）。
4. 设置权限：`sudo chmod 755 /opt/beetle/releases/vX.Y.Z/beetle`；配置文件与状态目录建议使用专用用户及 `0600`/`0700` 等权限。
5. 建立当前版本链接：`sudo ln -sfn /opt/beetle/releases/vX.Y.Z /opt/beetle/current`。
6. 按需安装 systemd：复制 `beetle.service` 到 `/etc/systemd/system/`，编辑 `User`/`Group`/`ReadWritePaths` 后执行 `systemctl daemon-reload && systemctl enable --now beetle`。

## 3. 升级到新版本

1. 将新版本解压到 `/opt/beetle/releases/<new>`。
2. 切换链接：`sudo ln -sfn /opt/beetle/releases/<new> /opt/beetle/current`。
3. 重启服务：`sudo systemctl restart beetle`（或 SysV：`./beetle.init restart`）。

## 4. 回滚到上一版本

1. 确认旧版本目录仍在（例如 `/opt/beetle/releases/v0.1.0`）。
2. 执行：`sudo ln -sfn /opt/beetle/releases/<previous> /opt/beetle/current`。
3. `sudo systemctl restart beetle`。
4. 验收：进程存在；若已启用配置 HTTP API，可请求 `GET http://<listen>/api/health`；日志中无持续错误。

## 5. 故障注入演练（建议发布前做一次）

- 故意将 `current` 指向错误目录，确认失败可观测；恢复软链后服务恢复。
- `kill -9` 主进程后由 systemd 拉起（若已配置 `Restart=`）。

## 6. 与 CI 的关系

回滚在**设备上**完成；GitHub Actions 发布流水线只产出 tarball、校验和与构建出处证明，不在 CI 内模拟软链。
