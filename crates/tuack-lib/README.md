# tuack-lib

tuack-ng 的后端公共逻辑库：纯数据与 trait 定义（不触碰文件系统、配置与全局状态）。

包含评测（`data`/`test`/`dmk`）、渲染（`ren`）、导出（`dump`）等模块的数据结构与抽象 trait，
供 `tuack-utils`（具体实现）与 `tuack-ng`（CLI 前端）复用。

许可证：AGPL-3.0-or-later。
