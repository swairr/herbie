# Linux 平台追踪能力整体降级为不可用，而非部分实现或静默缺失

herbie 增加 Linux 支持时，追踪三件套（前台窗口监控、空闲检测、电源/锁屏事件）**整体不实现**：Linux 上启动即判定为 Tracking Unavailable，UI 显式呈现（主窗口常驻横幅 + 托盘 tooltip + 设置页状态行），期间不产生任何 Activity Segment；Todo / Journal / 导出 / 托盘等核心功能完整可用。备选方案：**部分实现**——X11 侧技术上可行（`x11rb` 事件驱动前台 + XScreenSaver 空闲 + logind 电源），但一次引入两套原生依赖与平台分支，超出当前需求（验收面是自用 X11/Cinnamon 桌面之外的通用可用性不属于本次范围）；**Wayland 适配**——协议层面不存在全局前台窗口，需 per-compositor 适配，不可能一次做全；**静默不记录**——违反最小惊讶，用户会以为在记录。显式降级让“没有数据”的原因可见，且 `platform/` 门面接缝已就位，未来实现追踪只需填充 `linux/` 模块，核心状态机不动。

代价：Linux 用户暂时没有时间记录功能，该功能继续 Windows-only；未来实现时需重新引入 X11/D-Bus 依赖并做 Cinnamon 实测（含 logind `Lock`/`Unlock` 信号是否由 DE 发出的验证）。记录此决策以免后人把“Linux 没有追踪”误读为遗漏而非选择。
