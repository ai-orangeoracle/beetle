# configure-ui 状态分层约定

本约定用于约束 UI 侧状态管理，避免同类状态在多处重复维护。

## 1. 状态分层

- 持久连接状态：`DeviceContext`（`baseUrl` / `pairingCode`），允许 `localStorage` 持久化。
- 全局设备运行状态：`deviceStatusStore`（连接可达性、激活态、重启阶段）。
- 配置主数据：`ConfigContext`（`config` + `load/save`）。
- 页面可编辑副本：仅配置编辑页可保留本地 `form/useState`，用于“编辑未提交”场景。
- 远端异步数据：统一使用 `AsyncState<T>` 形态（`data/loading/error`）。
- 纯 UI 临时态：弹窗开关、输入焦点、折叠展开等，仅允许本地组件状态。

## 2. 强制规则

1. 同一业务数据只允许一个“权威源”（source of truth）。
2. 页面不得直接调用底层 `request()`；统一通过 `useDeviceApi().api.*`。
3. 配置页加载逻辑统一使用 `useConfigPageLoad`，禁止重复写 `loadAttemptedRef` 模板。
4. 保存反馈统一使用 `useSaveFeedback`，保存状态统一为 `idle/saving/ok/fail`。
5. 错误展示优先 i18n key；原始错误文案仅作为兜底。

## 3. 新页面接入清单

- 是否可归入 `ConfigContext` / `DeviceContext` / `deviceStatusStore` 之一？
- 是否复用 `useDeviceApi` 作为唯一请求入口？
- 异步请求是否使用 `AsyncState<T>`？
- 是否避免了重复的加载/保存模板逻辑？

如需新增全局状态，先补充本文件中的“状态分层”和“强制规则”，再落代码。
