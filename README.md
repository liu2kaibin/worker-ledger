# 🐷 打工人小账本 (Worker Ledger & Freedom Dashboard)

> **把钱和时间都算明白** —— 专为打工人设计的真实时薪核算、工时换算记账与自由基金规划工作台。  
> 视觉采用精致可爱的奶油薄荷手账风，**同时支持「ORA-SQLLite (Rust 原生嵌入式)」、「本地 SQLite 极简服务」与「腾讯 WorkBuddy 资料库」三大引擎**，实现多端设备在线双向实时同步。

---

## 📸 运行效果截图

| ORA-SQLLite (Rust 实时同步) | 桌面端（悬浮手账侧栏 + 驾驶舱） | 移动端（底部便携 Tab + 触控手账） |
| :--- | :--- | :--- |
| ![ORA-SQLLite截图](assets/screenshot-ora-dashboard.png) | ![桌面端截图](assets/screenshot-dashboard.png) | ![移动端截图](assets/screenshot-mobile.png) |

---

## 🌟 三大引擎架构自由切换

本项目前端页面（`index.html`）具备**智能环境嗅探**能力，一份前端代码完全兼容以下三种存储引擎：

```
                              ┌──────────────────────────────────────────────┐
                              │           打工人小账本 (index.html)            │
                              │        奶油薄荷手账风 + 全套内联矢量SVG        │
                              └──────┬──────────────┬──────────────┬─────────┘
                                     │              │              │
                ┌────────────────────▼────┐  ┌──────▼────────────┐ ┌▼────────────────────────┐
                │ 引擎一：ORA-SQLLite     │  │ 引擎二：本地 SQLite │ │ 引擎三：WorkBuddy 资料库│
                │ 纯 Rust Oracle 兼容子集  │  │ Python / Node.js   │ │ 腾讯云平台在线托管发布   │
                └────────────┬────────────┘  └──────┬────────────┘ └───────────┬─────────────┘
                             │                      │                          │
                   ┌─────────▼─────────┐  ┌─────────▼─────────┐      ┌─────────▼─────────┐
                   │ 本地文件: ledger.ora│  │ 本地文件: ledger.db│      │ 云端空间: 3张数据表 │
                   │ 单文件+WAL, NUMBER  │  │ 0依赖, 局域网多端同步│      │ 公网免登录短链发布 │
                   └───────────────────┘  └───────────────────┘      └───────────────────┘
```

1. **引擎一：ORA-SQLLite 纯 Rust 嵌入式引擎（已完整实装落地）**：
   * 基于项目作者自研的 **[ORA-SQLLite](https://github.com/liu2kaibin/ORA-SQLLite)**（纯 Rust，单文件+WAL，Oracle 兼容 CRUD 子集）。
   * 数据金额与工时采用 `NUMBER(12,2)` 严格精度量化，分类与备注采用 `VARCHAR2(32)` 与 `VARCHAR2(500)`。
   * 单个二进制可执行程序即可直接拉起全功能服务，毫秒级响应。
2. **引擎二：本地 SQLite 极简服务模式（0 额外依赖）**：
   * 仅依赖 Python 3 或 Node.js 标准库，**无需 `pip install` 或 `npm install` 任何第三方包**。
   * 本地自动创建 `ledger.db` 单文件数据库，同一 Wi-Fi 下手机直接输入内网 IP 即可同步记账。
3. **引擎三：腾讯 WorkBuddy 资料库云端工作台模式**：
   * 深度适配 WorkBuddy SmartPage 资料库 SDK (`window.__SMART_PAGE__.database`)。
   * 包含 3 张结构化数据表，支持公网免登录短链发布（`https://workbuddy.link/p/...`）。

---

## 🚀 极速上手

### 选项 A：使用 ORA-SQLLite 启动（Rust 原生嵌入式）

进入 `ora-backend/` 目录运行：

```bash
cd ora-backend
cargo run --release
```

终端将打印：
```text
==================================================================
 [打工人小账本] ORA-SQLLite (Rust 嵌入式数据库) 服务已启动！
 * 电脑本机访问: http://localhost:8080
 * 同局域网手机: http://192.168.x.x:8080
 * ORA-SQLLite 存储文件: .../ledger.ora
 * 底层引擎: ORA-SQLLite v0.1.0 (纯 Rust, 单文件+WAL, Oracle兼容子集)
==================================================================
```
浏览器打开 `http://localhost:8080`，顶部状态胶囊自动亮起绿灯：**`ORA-SQLLite (Rust Embedded) 实时同步`**！

---

### 选项 B：使用本地 SQLite 启动（Python / Node.js 0 依赖）

在项目根目录运行以下任一命令：

* **使用 Python 启动**：
  ```bash
  python app.py
  # Windows 下也可以直接：
  py app.py
  ```
* **使用 Node.js 启动**：
  ```bash
  node server.mjs
  ```

---

### 选项 C：WorkBuddy 空间部署

1. **创建 3 张数据表**：参考 [DATABASE_SCHEMAS.md](DATABASE_SCHEMAS.md)，在 WorkBuddy 空间创建「支出流水」、「月度总结」、「个人设置」表。
2. **导入页面**：上传本项目的 `index.html` 为 WorkBuddy 页面。
3. **绑定表 ID**：打开页面右上角点击「资料库」胶囊，填入 3 个数据表 ID，点击保存并同步。
4. **发布**：点击右上角「发布」，获得专属公网访问链接。详见 [DEPLOYMENT_GUIDE.md](DEPLOYMENT_GUIDE.md)。

---

## 💡 核心功能清单

### 1. 真实时薪计算器 (Work Value)
* 综合核算：**到手月薪**、**发薪月数**（12~16）、**每月工作成本**（通勤包车/差旅/职业着装等）、**每日在公司在场时间**（含午休）、**单程通勤分钟**、**每周额外加班小时**。
* 名义时薪与真实时薪并列对比，标明差距百分比。
* 展开 7 步核算公式：年到手收入、年工作成本、月在场、月通勤、月加班、年总投入时间及真实时薪公式。

### 2. 10 秒即时记账与工时换算 (10-Second Expense)
* 快速录入：金额、分类（吃饭、交通、购物、娱乐、房租、其他）、备注与日期。
* 保存后即时换算为消耗的真实工作时间（例如：*“这笔 ¥45 相当于 49 分钟 的真实工作时间”*）。

### 3. 今日驾驶舱 (Dashboard)
* 核心 KPI：真实时薪、本月支出、自由基金进度、自由宣言。
* 首页最近 4 笔记录：手账便签卡片，配精致分类矢量图标、金额与等价工时。
* 快速记账卡片：首页即输即存，10 秒内收进账本。

### 4. 月度收入与固定、弹性支出总结 (Monthly Review)
* 三项极简录入：总收入、固定支出（刚性开销）、弹性支出（灵活开销）。
* 现场自动计算当月结余，滚动计算最近完整月平均净存款能力。

### 5. 自由基金目标、安全垫与原生 SVG 存款曲线 (Freedom Fund)
* 设定目标金额、达成月份、当前已攒积蓄、每月基本生活开销与自由宣言。
* **安全垫阶梯点亮**：智能核算并点亮 3 个月、6 个月、12 个月、24 个月基本生活“安全垫”。
* **原生 SVG 存款曲线**：由历史月度结余推导平滑绿色折线与数据圆点，叠加目标虚线。

### 6. 三情景模拟 (Scenario Simulation)
* 滑块调节通勤减少比例 (0~100%)、每周少加班小时 (0~20h)、涨薪比例 (0~100%)。
* 并列对比现金增益、时间省下、自由等价增益与目标月数，输出科学决策结论。

---

## 📁 项目文件结构

```text
worker-ledger/
├── ora-backend/               # 【核心新增】ORA-SQLLite 纯 Rust 嵌入式后端
│   ├── Cargo.toml             # 依赖 github.com/liu2kaibin/ORA-SQLLite
│   ├── src/main.rs            # 原生 Rust HTTP 服务器与 Oracle 语法 CRUD
│   └── README.md              # ORA-SQLLite 专属说明文档
├── app.py                     # Python 3 SQLite 极简本地服务 (0第三方依赖)
├── server.mjs                 # Node.js 24 SQLite 原生本地服务 (0 npm依赖)
├── index.html                 # 打工人小账本完整单文件前端应用 (自适应三大引擎)
├── DATABASE_SCHEMAS.md        # WorkBuddy 与 SQLite 数据表结构全字段规范
├── DEPLOYMENT_GUIDE.md        # WorkBuddy 资料库空间建表、绑定与发布详细指南
├── .gitignore                 # 忽略本地生成的 *.db / *.ora / target 缓存
├── README.md                  # 项目综合说明文档
└── assets/                    # 界面实测高清截图
    ├── screenshot-ora-dashboard.png    # ORA-SQLLite 实机运行截图
    ├── screenshot-dashboard.png        # 桌面端手账截图
    ├── screenshot-mobile.png           # 移动端手账截图
    └── screenshot-sqlite-dashboard.png # SQLite 实机运行截图
```

---

## 🔒 隐私与许可

* 采用 ORA-SQLLite 或 SQLite 模式运行时，数据 100% 保存在您本机的 `ledger.ora` / `ledger.db`，绝不上报任何外部服务器。
* 采用 WorkBuddy 模式运行时，数据仅在您授权的 WorkBuddy 空间内流转。
