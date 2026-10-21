# ORA-SQLLite 纯 Rust 嵌入式后端服务

> 本模块为「打工人小账本」提供基于 **[ORA-SQLLite](https://github.com/liu2kaibin/ORA-SQLLite)** 的纯 Rust 原生嵌入式后端支持。  
> 采用 Oracle 兼容语法子集、单文件 + WAL 事务引擎，数据落地为 `ledger.ora`。

---

## 🌟 核心特性

* **底层引擎**：直接引入 `ora_sqllite` crate（纯 Rust 开发，单进程拥有，单文件 + WAL）。
* **零外部 Web 框架**：基于 Rust 标准库实现极速 HTTP/1.1 服务器与 RESTful API，免除复杂异步运行时依赖。
* **数据类型映射**：
  * 流水金额与工时：映射至 `NUMBER(12, 2)` 与 `NUMBER(8, 2)`。
  * 消费分类与日期：映射至 `VARCHAR2(32)`。
  * 全量配置与备份：映射至 `VARCHAR2(4000)`。
* **双向即时同步**：前端执行增删改查时，毫秒级提交至 `ledger.ora` 事务并在 WAL 落盘。

---

## 🚀 启动方式

进入当前目录，直接运行：

```bash
cargo run --release
```

启动成功后，终端将输出：
```text
==================================================================
 [打工人小账本] ORA-SQLLite (Rust 嵌入式数据库) 服务已启动！
 * 电脑本机访问: http://localhost:8080
 * 同局域网手机: http://192.168.x.x:8080
 * ORA-SQLLite 存储文件: .../ledger.ora
 * 底层引擎: ORA-SQLLite v0.1.0 (纯 Rust, 单文件+WAL, Oracle兼容子集)
==================================================================
```

* **电脑浏览器** 打开：`http://localhost:8080`
* **同局域网手机** 打开：`http://192.168.x.x:8080`
* 顶部同步状态胶囊自动亮起绿灯：**`ORA-SQLLite (Rust Embedded) 实时同步`**！
