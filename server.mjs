#!/usr/bin/env node
/**
 * 打工人小账本 - Node.js SQLite 本地持久化与多端同步服务
 * 仅依赖 Node.js v22+ / v24+ 内置 node:sqlite 与 node:http，0 npm 安装！
 */

import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DatabaseSync } from 'node:sqlite';
import os from 'node:os';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const DB_PATH = path.join(__dirname, 'ledger.db');
const PORT = 8080;

const db = new DatabaseSync(DB_PATH);

// 初始化 SQLite 表结构
db.exec(`
  CREATE TABLE IF NOT EXISTS expenses (
      id TEXT PRIMARY KEY,
      date TEXT NOT NULL,
      category TEXT NOT NULL,
      amount REAL NOT NULL,
      note TEXT,
      work_hours REAL DEFAULT 0,
      created_at TEXT NOT NULL
  );
  CREATE TABLE IF NOT EXISTS summaries (
      month TEXT PRIMARY KEY,
      income REAL NOT NULL,
      fixed REAL NOT NULL,
      flexible REAL NOT NULL,
      balance REAL NOT NULL,
      updated_at TEXT NOT NULL
  );
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
`);

function getLocalIp() {
  const nets = os.networkInterfaces();
  for (const name of Object.keys(nets)) {
    for (const net of nets[name]) {
      if (net.family === 'IPv4' && !net.internal) {
        return net.address;
      }
    }
  }
  return '127.0.0.1';
}

function sendJson(res, data, status = 200) {
  res.writeHead(status, {
    'Content-Type': 'application/json; charset=utf-8',
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type, Authorization',
    'Cache-Control': 'no-cache, no-store, must-revalidate'
  });
  res.end(JSON.stringify(data));
}

function readBody(req) {
  return new Promise((resolve) => {
    let raw = '';
    req.on('data', chunk => { raw += chunk; });
    req.on('end', () => {
      try { resolve(JSON.parse(raw)); } catch (e) { resolve({}); }
    });
  });
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`);
  const pathname = decodeURIComponent(url.pathname);

  // CORS Preflight
  if (req.method === 'OPTIONS') {
    res.writeHead(200, {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type, Authorization'
    });
    return res.end();
  }

  // API 路由
  if (pathname === '/api/status' && req.method === 'GET') {
    const expCount = db.prepare('SELECT COUNT(*) as c FROM expenses').get().c;
    const sumCount = db.prepare('SELECT COUNT(*) as c FROM summaries').get().c;
    const hasSettings = !!db.prepare("SELECT key FROM settings WHERE key='user_settings'").get();
    return sendJson(res, {
      ok: true,
      engine: 'SQLite (Node.js)',
      dbPath: DB_PATH,
      counts: { expenses: expCount, summaries: sumCount, hasSettings }
    });
  }

  if (pathname === '/api/all' && req.method === 'GET') {
    const expenses = db.prepare('SELECT * FROM expenses ORDER BY date DESC, created_at DESC').all();
    const summaries = db.prepare('SELECT * FROM summaries ORDER BY month DESC').all();
    const settings = db.prepare("SELECT * FROM settings WHERE key='user_settings'").get() || null;
    return sendJson(res, { ok: true, expenses, summaries, settings });
  }

  if (pathname === '/api/expenses' && req.method === 'POST') {
    const body = await readBody(req);
    const id = body.id || ('expense-' + Date.now());
    const date = body.date || new Date().toISOString().slice(0, 10);
    const category = body.category || '其他';
    const amount = Number(body.amount) || 0;
    const note = body.note || '';
    const workHours = Number(body.work_hours) || 0;
    const createdAt = body.created_at || new Date().toISOString();

    const stmt = db.prepare(`
      INSERT INTO expenses (id, date, category, amount, note, work_hours, created_at)
      VALUES (?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET
        date=excluded.date, category=excluded.category, amount=excluded.amount,
        note=excluded.note, work_hours=excluded.work_hours
    `);
    stmt.run(id, date, category, amount, note, workHours, createdAt);
    return sendJson(res, { ok: true, id });
  }

  if (pathname.startsWith('/api/expenses/') && req.method === 'DELETE') {
    const id = pathname.slice('/api/expenses/'.length);
    db.prepare('DELETE FROM expenses WHERE id = ?').run(id);
    return sendJson(res, { ok: true, deleted: id });
  }

  if (pathname === '/api/summaries' && req.method === 'POST') {
    const body = await readBody(req);
    const month = body.month;
    if (!month) return sendJson(res, { ok: false, error: '月份不能为空' }, 400);
    const income = Number(body.income) || 0;
    const fixed = Number(body.fixed) || 0;
    const flexible = Number(body.flexible) || 0;
    const balance = income - fixed - flexible;
    const updatedAt = new Date().toISOString();

    const stmt = db.prepare(`
      INSERT INTO summaries (month, income, fixed, flexible, balance, updated_at)
      VALUES (?, ?, ?, ?, ?, ?)
      ON CONFLICT(month) DO UPDATE SET
        income=excluded.income, fixed=excluded.fixed, flexible=excluded.flexible,
        balance=excluded.balance, updated_at=excluded.updated_at
    `);
    stmt.run(month, income, fixed, flexible, balance, updatedAt);
    return sendJson(res, { ok: true, month, balance });
  }

  if (pathname.startsWith('/api/summaries/') && req.method === 'DELETE') {
    const month = pathname.slice('/api/summaries/'.length);
    db.prepare('DELETE FROM summaries WHERE month = ?').run(month);
    return sendJson(res, { ok: true, deleted: month });
  }

  if (pathname === '/api/settings' && req.method === 'POST') {
    const body = await readBody(req);
    const key = 'user_settings';
    const profile = body.profile || {};
    const freedom = body.freedom || {};
    const configJson = JSON.stringify(body);
    const updatedAt = new Date().toISOString();

    const stmt = db.prepare(`
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
    `);
    stmt.run(
      key,
      Number(profile.monthlySalary) || 0,
      Number(profile.payMonths) || 12,
      Number(profile.monthlyWorkCost) || 0,
      Number(profile.dailyOfficeHours) || 8,
      Number(profile.oneWayCommuteMinutes) || 0,
      Number(profile.weeklyOvertimeHours) || 0,
      Number(freedom.targetAmount) || 0,
      Number(freedom.currentSaved) || 0,
      Number(freedom.basicMonthlyCost) || 0,
      String(freedom.targetDate || ''),
      String(freedom.startMonth || ''),
      String(freedom.reason || ''),
      configJson,
      updatedAt
    );
    return sendJson(res, { ok: true });
  }

  // 静态页面服务
  let filePath = path.join(__dirname, pathname === '/' ? 'index.html' : pathname);
  if (!fs.existsSync(filePath)) {
    filePath = path.join(__dirname, 'index.html');
  }

  const ext = path.extname(filePath).toLowerCase();
  const mimeMap = {
    '.html': 'text/html; charset=utf-8',
    '.css': 'text/css; charset=utf-8',
    '.js': 'application/javascript; charset=utf-8',
    '.png': 'image/png',
    '.svg': 'image/svg+xml'
  };

  try {
    const data = fs.readFileSync(filePath);
    res.writeHead(200, {
      'Content-Type': mimeMap[ext] || 'application/octet-stream',
      'Access-Control-Allow-Origin': '*'
    });
    res.end(data);
  } catch (e) {
    res.writeHead(404);
    res.end('Not Found');
  }
});

server.listen(PORT, '0.0.0.0', () => {
  const localIp = getLocalIp();
  console.log('='.repeat(64));
  console.log(' 🐷「打工人小账本」SQLite 本地持久化与多端同步服务已启动 (Node.js)');
  console.log(` * 电脑本机访问: http://localhost:${PORT}`);
  console.log(` * 同局域网手机: http://${localIp}:${PORT}`);
  console.log(` * SQLite 数据库: ${DB_PATH}`);
  console.log(' * 数据真正存储在本地 SQLite，支持多设备实时同步，数据 100% 自主可控');
  console.log('='.repeat(64));
  console.log(' 按 Ctrl+C 即可退出服务。\n');
});
