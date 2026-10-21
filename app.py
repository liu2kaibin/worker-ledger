#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
打工人小账本 - SQLite 本地持久化与局域网同步服务
仅依赖 Python 3 标准库（内置 sqlite3 与 http.server），无需额外安装任何第三方包。
"""

import os
import sys
import json
import sqlite3
import socket
from http.server import HTTPServer, SimpleHTTPRequestHandler
from urllib.parse import urlparse, parse_qs, unquote
from datetime import datetime

DB_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ledger.db")
PORT = 8080

def get_db():
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    return conn

def init_db():
    with get_db() as conn:
        cursor = conn.cursor()
        cursor.execute("""
        CREATE TABLE IF NOT EXISTS expenses (
            id TEXT PRIMARY KEY,
            date TEXT NOT NULL,
            category TEXT NOT NULL,
            amount REAL NOT NULL,
            note TEXT,
            work_hours REAL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        """)
        cursor.execute("""
        CREATE TABLE IF NOT EXISTS summaries (
            month TEXT PRIMARY KEY,
            income REAL NOT NULL,
            fixed REAL NOT NULL,
            flexible REAL NOT NULL,
            balance REAL NOT NULL,
            updated_at TEXT NOT NULL
        );
        """)
        cursor.execute("""
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            monthly_salary REAL,
            pay_months REAL,
            monthly_work_cost REAL,
            daily_office_hours REAL,
            one_way_commute_minutes REAL,
            weekly_overtime_hours REAL,
            target_amount REAL,
            current_saved REAL,
            basic_monthly_cost REAL,
            target_date TEXT,
            start_month TEXT,
            reason TEXT,
            config_json TEXT,
            updated_at TEXT NOT NULL
        );
        """)
        conn.commit()

def get_local_ip():
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.connect(("8.8.8.8", 80))
        ip = s.getsockname()[0]
        s.close()
        return ip
    except Exception:
        return "127.0.0.1"

class LedgerHandler(SimpleHTTPRequestHandler):
    def end_headers(self):
        # 允许跨域请求，支持从任何本地端口或不同主机访问
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type, Authorization")
        self.send_header("Cache-Control", "no-cache, no-store, must-revalidate")
        super().end_headers()

    def do_OPTIONS(self):
        self.send_response(200)
        self.end_headers()

    def send_json(self, data, status=200):
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.end_headers()
        self.wfile.write(json.dumps(data, ensure_ascii=False).encode("utf-8"))

    def read_json_body(self):
        content_length = int(self.headers.get("Content-Length", 0))
        if content_length <= 0:
            return {}
        body = self.rfile.read(content_length).decode("utf-8")
        try:
            return json.loads(body)
        except Exception:
            return {}

    def do_GET(self):
        parsed = urlparse(self.path)
        path = parsed.path

        if path == "/api/status":
            with get_db() as conn:
                exp_count = conn.execute("SELECT COUNT(*) FROM expenses").fetchone()[0]
                sum_count = conn.execute("SELECT COUNT(*) FROM summaries").fetchone()[0]
                has_settings = conn.execute("SELECT COUNT(*) FROM settings WHERE key='user_settings'").fetchone()[0]
            return self.send_json({
                "ok": True,
                "engine": "SQLite",
                "dbPath": DB_PATH,
                "counts": {
                    "expenses": exp_count,
                    "summaries": sum_count,
                    "hasSettings": bool(has_settings)
                }
            })

        if path == "/api/all":
            with get_db() as conn:
                expenses = [dict(r) for r in conn.execute("SELECT * FROM expenses ORDER BY date DESC, created_at DESC").fetchall()]
                summaries = [dict(r) for r in conn.execute("SELECT * FROM summaries ORDER BY month DESC").fetchall()]
                setting_row = conn.execute("SELECT * FROM settings WHERE key='user_settings'").fetchone()
                setting_data = dict(setting_row) if setting_row else None
            return self.send_json({
                "ok": True,
                "expenses": expenses,
                "summaries": summaries,
                "settings": setting_data
            })

        # 默认静态页面分发
        if path == "/" or path == "":
            self.path = "/index.html"
        return super().do_GET()

    def do_POST(self):
        parsed = urlparse(self.path)
        path = parsed.path
        body = self.read_json_body()

        if path == "/api/expenses":
            exp_id = body.get("id") or ("expense-" + str(int(datetime.now().timestamp() * 1000)))
            date = body.get("date", datetime.now().strftime("%Y-%m-%d"))
            category = body.get("category", "其他")
            amount = float(body.get("amount", 0))
            note = body.get("note", "")
            work_hours = float(body.get("work_hours", 0))
            created_at = body.get("created_at") or datetime.now().isoformat()

            with get_db() as conn:
                conn.execute("""
                INSERT INTO expenses (id, date, category, amount, note, work_hours, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    date=excluded.date,
                    category=excluded.category,
                    amount=excluded.amount,
                    note=excluded.note,
                    work_hours=excluded.work_hours
                """, (exp_id, date, category, amount, note, work_hours, created_at))
                conn.commit()
            return self.send_json({"ok": True, "id": exp_id})

        if path == "/api/summaries":
            month = body.get("month")
            if not month:
                return self.send_json({"ok": False, "error": "月份不能为空"}, 400)
            income = float(body.get("income", 0))
            fixed = float(body.get("fixed", 0))
            flexible = float(body.get("flexible", 0))
            balance = income - fixed - flexible
            updated_at = datetime.now().isoformat()

            with get_db() as conn:
                conn.execute("""
                INSERT INTO summaries (month, income, fixed, flexible, balance, updated_at)
                VALUES (?, ?, ?, ?, ?, ?)
                ON CONFLICT(month) DO UPDATE SET
                    income=excluded.income,
                    fixed=excluded.fixed,
                    flexible=excluded.flexible,
                    balance=excluded.balance,
                    updated_at=excluded.updated_at
                """, (month, income, fixed, flexible, balance, updated_at))
                conn.commit()
            return self.send_json({"ok": True, "month": month, "balance": balance})

        if path == "/api/settings":
            key = "user_settings"
            profile = body.get("profile", {})
            freedom = body.get("freedom", {})
            config_json = json.dumps(body, ensure_ascii=False)
            updated_at = datetime.now().isoformat()

            with get_db() as conn:
                conn.execute("""
                INSERT INTO settings (
                    key, monthly_salary, pay_months, monthly_work_cost,
                    daily_office_hours, one_way_commute_minutes, weekly_overtime_hours,
                    target_amount, current_saved, basic_monthly_cost,
                    target_date, start_month, reason, config_json, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(key) DO UPDATE SET
                    monthly_salary=excluded.monthly_salary,
                    pay_months=excluded.pay_months,
                    monthly_work_cost=excluded.monthly_work_cost,
                    daily_office_hours=excluded.daily_office_hours,
                    one_way_commute_minutes=excluded.one_way_commute_minutes,
                    weekly_overtime_hours=excluded.weekly_overtime_hours,
                    target_amount=excluded.target_amount,
                    current_saved=excluded.current_saved,
                    basic_monthly_cost=excluded.basic_monthly_cost,
                    target_date=excluded.target_date,
                    start_month=excluded.start_month,
                    reason=excluded.reason,
                    config_json=excluded.config_json,
                    updated_at=excluded.updated_at
                """, (
                    key,
                    float(profile.get("monthlySalary", 0)),
                    float(profile.get("payMonths", 12)),
                    float(profile.get("monthlyWorkCost", 0)),
                    float(profile.get("dailyOfficeHours", 8)),
                    float(profile.get("oneWayCommuteMinutes", 0)),
                    float(profile.get("weeklyOvertimeHours", 0)),
                    float(freedom.get("targetAmount", 0)),
                    float(freedom.get("currentSaved", 0)),
                    float(freedom.get("basicMonthlyCost", 0)),
                    str(freedom.get("targetDate", "")),
                    str(freedom.get("startMonth", "")),
                    str(freedom.get("reason", "")),
                    config_json,
                    updated_at
                ))
                conn.commit()
            return self.send_json({"ok": True})

        return self.send_json({"ok": False, "error": "Not Found"}, 404)

    def do_DELETE(self):
        parsed = urlparse(self.path)
        path = unquote(parsed.path)

        if path.startswith("/api/expenses/"):
            exp_id = path[len("/api/expenses/"):]
            with get_db() as conn:
                conn.execute("DELETE FROM expenses WHERE id = ?", (exp_id,))
                conn.commit()
            return self.send_json({"ok": True, "deleted": exp_id})

        if path.startswith("/api/summaries/"):
            month = path[len("/api/summaries/"):]
            with get_db() as conn:
                conn.execute("DELETE FROM summaries WHERE month = ?", (month,))
                conn.commit()
            return self.send_json({"ok": True, "deleted": month})

        return self.send_json({"ok": False, "error": "Not Found"}, 404)

if __name__ == "__main__":
    if hasattr(sys.stdout, "reconfigure"):
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    init_db()
    local_ip = get_local_ip()
    server_address = ("0.0.0.0", PORT)
    httpd = HTTPServer(server_address, LedgerHandler)

    print("=" * 64)
    print(" [打工人小账本] SQLite 本地持久化与多端同步服务已启动！")
    print(f" * 电脑本机访问: http://localhost:{PORT}")
    print(f" * 同局域网手机: http://{local_ip}:{PORT}")
    print(f" * SQLite 数据库: {DB_PATH}")
    print(" * 数据真正存储在本地 SQLite，支持多设备实时同步，数据 100% 自主可控")
    print("=" * 64)
    print(" 按 Ctrl+C 即可退出服务。\n")

    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\n服务已平稳关闭。")
        sys.exit(0)
