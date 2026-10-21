use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use ora_sqllite::{Connection, Database, Decimal, Options, Parameters, Value};
use serde_json::json;

const PORT: u16 = 8080;
const DB_FILE: &str = "ledger.ora";

fn decimal_to_f64(d: &Decimal) -> f64 {
    let mag = d.magnitude() as f64;
    let scale = d.scale() as i32;
    let factor = 10f64.powi(-scale);
    let val = mag * factor;
    if d.negative() { -val } else { val }
}

fn url_decode(s: &str) -> String {
    let mut bytes = s.bytes();
    let mut decoded = Vec::new();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let h1 = bytes.next().unwrap_or(b'0');
            let h2 = bytes.next().unwrap_or(b'0');
            if let Ok(hex_str) = std::str::from_utf8(&[h1, h2]) {
                if let Ok(val) = u8::from_str_radix(hex_str, 16) {
                    decoded.push(val);
                    continue;
                }
            }
        } else if b == b'+' {
            decoded.push(b' ');
        } else {
            decoded.push(b);
        }
    }
    String::from_utf8_lossy(&decoded).to_string()
}

fn get_local_ip() -> String {
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

struct AppState {
    conn: Mutex<Connection>,
    db_path: PathBuf,
}

fn init_db(path: &Path) -> Result<(Database, Connection), Box<dyn std::error::Error>> {
    let exists = path.exists();
    let (db, conn) = if exists {
        let db = Database::open(path, Options::default())?;
        let conn = db.connect()?;
        (db, conn)
    } else {
        let db = Database::create(path, Options::default())?;
        let mut conn = db.connect()?;

        // 初始化 Oracle 兼容数据表
        let ddl_expenses = conn.prepare(
            "CREATE TABLE EXPENSES (\
                ID VARCHAR2(64) PRIMARY KEY, \
                EXP_DATE VARCHAR2(32) NOT NULL, \
                CATEGORY VARCHAR2(32) NOT NULL, \
                AMOUNT NUMBER(12, 2) NOT NULL, \
                NOTE VARCHAR2(500), \
                WORK_HOURS NUMBER(8, 2), \
                CREATED_AT VARCHAR2(64) NOT NULL\
            )",
        )?;
        conn.execute(&ddl_expenses, Parameters::none())?;

        let ddl_summaries = conn.prepare(
            "CREATE TABLE SUMMARIES (\
                MONTH_KEY VARCHAR2(16) PRIMARY KEY, \
                INCOME NUMBER(12, 2) NOT NULL, \
                FIXED_COST NUMBER(12, 2) NOT NULL, \
                FLEXIBLE_COST NUMBER(12, 2) NOT NULL, \
                BALANCE NUMBER(12, 2) NOT NULL, \
                UPDATED_AT VARCHAR2(64) NOT NULL\
            )",
        )?;
        conn.execute(&ddl_summaries, Parameters::none())?;

        let ddl_settings = conn.prepare(
            "CREATE TABLE SETTINGS (\
                CONFIG_KEY VARCHAR2(32) PRIMARY KEY, \
                CONFIG_JSON VARCHAR2(4000) NOT NULL, \
                UPDATED_AT VARCHAR2(64) NOT NULL\
            )",
        )?;
        conn.execute(&ddl_settings, Parameters::none())?;

        (db, conn)
    };

    Ok((db, conn))
}

fn handle_connection(mut stream: TcpStream, state: &Arc<AppState>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
        return;
    }

    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let raw_path = parts[1];
    let path = url_decode(raw_path.split('?').next().unwrap_or("/"));

    let mut content_length: usize = 0;
    loop {
        let mut header_line = String::new();
        if reader.read_line(&mut header_line).is_err() || header_line.trim().is_empty() {
            break;
        }
        let lower = header_line.to_lowercase();
        if lower.starts_with("content-length:") {
            if let Some(val) = lower.split(':').nth(1) {
                content_length = val.trim().parse().unwrap_or(0);
            }
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        let _ = reader.read_exact(&mut body);
    }

    // CORS preflight
    if method == "OPTIONS" {
        send_response(&mut stream, 200, "text/plain", b"OK");
        return;
    }

    // API: GET /api/status
    if method == "GET" && path == "/api/status" {
        let mut conn = state.conn.lock().unwrap();
        let exp_stmt = conn.prepare("SELECT COUNT(*) FROM EXPENSES").unwrap();
        let mut rows = conn.query(&exp_stmt, Parameters::none()).unwrap();
        let exp_count = if let Some(r) = rows.next().unwrap() {
            match r.values()[0] {
                Value::Int64(n) => n,
                Value::Decimal(d) => d.magnitude() as i64,
                _ => 0,
            }
        } else {
            0
        };

        let sum_stmt = conn.prepare("SELECT COUNT(*) FROM SUMMARIES").unwrap();
        let mut rows = conn.query(&sum_stmt, Parameters::none()).unwrap();
        let sum_count = if let Some(r) = rows.next().unwrap() {
            match r.values()[0] {
                Value::Int64(n) => n,
                Value::Decimal(d) => d.magnitude() as i64,
                _ => 0,
            }
        } else {
            0
        };

        let has_settings = {
            let set_stmt = conn
                .prepare("SELECT CONFIG_KEY FROM SETTINGS WHERE CONFIG_KEY = 'user_settings'")
                .unwrap();
            let mut rows = conn.query(&set_stmt, Parameters::none()).unwrap();
            rows.next().unwrap().is_some()
        };

        let resp = json!({
            "ok": true,
            "engine": "ORA-SQLLite (Rust Embedded)",
            "dbPath": state.db_path.to_string_lossy(),
            "counts": {
                "expenses": exp_count,
                "summaries": sum_count,
                "hasSettings": has_settings
            }
        });
        send_json(&mut stream, 200, &resp);
        return;
    }

    // API: GET /api/all
    if method == "GET" && path == "/api/all" {
        let mut conn = state.conn.lock().unwrap();

        // 1. Query Expenses
        let mut expenses = Vec::new();
        let exp_stmt = conn
            .prepare("SELECT ID, EXP_DATE, CATEGORY, AMOUNT, NOTE, WORK_HOURS, CREATED_AT FROM EXPENSES ORDER BY EXP_DATE DESC, CREATED_AT DESC")
            .unwrap();
        let mut rows = conn.query(&exp_stmt, Parameters::none()).unwrap();
        while let Some(r) = rows.next().unwrap() {
            let vals = r.values();
            let id = if let Value::Text(s) = &vals[0] { s.clone() } else { "".into() };
            let date = if let Value::Text(s) = &vals[1] { s.clone() } else { "".into() };
            let cat = if let Value::Text(s) = &vals[2] { s.clone() } else { "其他".into() };
            let amt = if let Value::Decimal(d) = &vals[3] { decimal_to_f64(d) } else { 0.0 };
            let note = if let Value::Text(s) = &vals[4] { s.clone() } else { "".into() };
            let wh = if let Value::Decimal(d) = &vals[5] { decimal_to_f64(d) } else { 0.0 };
            let ca = if let Value::Text(s) = &vals[6] { s.clone() } else { "".into() };

            expenses.push(json!({
                "id": id,
                "date": date,
                "category": cat,
                "amount": amt,
                "note": note,
                "work_hours": wh,
                "created_at": ca
            }));
        }

        // 2. Query Summaries
        let mut summaries = Vec::new();
        let sum_stmt = conn
            .prepare("SELECT MONTH_KEY, INCOME, FIXED_COST, FLEXIBLE_COST, BALANCE, UPDATED_AT FROM SUMMARIES ORDER BY MONTH_KEY DESC")
            .unwrap();
        let mut rows = conn.query(&sum_stmt, Parameters::none()).unwrap();
        while let Some(r) = rows.next().unwrap() {
            let vals = r.values();
            let m = if let Value::Text(s) = &vals[0] { s.clone() } else { "".into() };
            let inc = if let Value::Decimal(d) = &vals[1] { decimal_to_f64(d) } else { 0.0 };
            let fix = if let Value::Decimal(d) = &vals[2] { decimal_to_f64(d) } else { 0.0 };
            let flx = if let Value::Decimal(d) = &vals[3] { decimal_to_f64(d) } else { 0.0 };
            let bal = if let Value::Decimal(d) = &vals[4] { decimal_to_f64(d) } else { 0.0 };
            let ua = if let Value::Text(s) = &vals[5] { s.clone() } else { "".into() };

            summaries.push(json!({
                "month": m,
                "income": inc,
                "fixed": fix,
                "flexible": flx,
                "balance": bal,
                "updated_at": ua
            }));
        }

        // 3. Query Settings
        let mut settings_json = None;
        let set_stmt = conn
            .prepare("SELECT CONFIG_KEY, CONFIG_JSON, UPDATED_AT FROM SETTINGS WHERE CONFIG_KEY = 'user_settings'")
            .unwrap();
        let mut rows = conn.query(&set_stmt, Parameters::none()).unwrap();
        if let Some(r) = rows.next().unwrap() {
            let vals = r.values();
            if let Value::Text(cfg) = &vals[1] {
                settings_json = Some(json!({ "config_json": cfg }));
            }
        }

        let resp = json!({
            "ok": true,
            "expenses": expenses,
            "summaries": summaries,
            "settings": settings_json
        });
        send_json(&mut stream, 200, &resp);
        return;
    }

    // API: POST /api/expenses
    if method == "POST" && path == "/api/expenses" {
        if let Ok(req) = serde_json::from_slice::<serde_json::Value>(&body) {
            let id = req["id"].as_str().unwrap_or("").to_string();
            let date = req["date"].as_str().unwrap_or("").to_string();
            let category = req["category"].as_str().unwrap_or("其他").to_string();
            let amount = req["amount"].as_f64().unwrap_or(0.0);
            let note = req["note"].as_str().unwrap_or("").to_string();
            let work_hours = req["work_hours"].as_f64().unwrap_or(0.0);
            let created_at = req["created_at"].as_str().unwrap_or("").to_string();

            let amount_str = format!("{:.2}", amount);
            let wh_str = format!("{:.2}", work_hours);

            let mut conn = state.conn.lock().unwrap();

            // 检查记录是否已存在
            let check_stmt = conn.prepare("SELECT ID FROM EXPENSES WHERE ID = :1").unwrap();
            let check_param = [Value::Text(id.clone()).into()];
            let exists = {
                let mut rows = conn.query(&check_stmt, Parameters::positional(&check_param)).unwrap();
                rows.next().unwrap().is_some()
            };

            if exists {
                let update_stmt = conn.prepare(
                    "UPDATE EXPENSES SET EXP_DATE = :1, CATEGORY = :2, AMOUNT = :3, NOTE = :4, WORK_HOURS = :5 WHERE ID = :6"
                ).unwrap();
                let params = [
                    Value::Text(date).into(),
                    Value::Text(category).into(),
                    Value::Text(amount_str).into(),
                    Value::Text(note).into(),
                    Value::Text(wh_str).into(),
                    Value::Text(id.clone()).into(),
                ];
                let _ = conn.execute(&update_stmt, Parameters::positional(&params));
            } else {
                let insert_stmt = conn.prepare("INSERT INTO EXPENSES VALUES (:1, :2, :3, :4, :5, :6, :7)").unwrap();
                let params = [
                    Value::Text(id.clone()).into(),
                    Value::Text(date).into(),
                    Value::Text(category).into(),
                    Value::Text(amount_str).into(),
                    Value::Text(note).into(),
                    Value::Text(wh_str).into(),
                    Value::Text(created_at).into(),
                ];
                let _ = conn.execute(&insert_stmt, Parameters::positional(&params));
            }

            send_json(&mut stream, 200, &json!({ "ok": true, "id": id }));
            return;
        }
    }

    // API: DELETE /api/expenses/:id
    if method == "DELETE" && path.starts_with("/api/expenses/") {
        let id = path.trim_start_matches("/api/expenses/");
        let mut conn = state.conn.lock().unwrap();
        let del_stmt = conn.prepare("DELETE FROM EXPENSES WHERE ID = :1").unwrap();
        let params = [Value::Text(id.to_string()).into()];
        let _ = conn.execute(&del_stmt, Parameters::positional(&params));
        send_json(&mut stream, 200, &json!({ "ok": true, "deleted": id }));
        return;
    }

    // API: POST /api/summaries
    if method == "POST" && path == "/api/summaries" {
        if let Ok(req) = serde_json::from_slice::<serde_json::Value>(&body) {
            let month = req["month"].as_str().unwrap_or("").to_string();
            if !month.is_empty() {
                let income = req["income"].as_f64().unwrap_or(0.0);
                let fixed = req["fixed"].as_f64().unwrap_or(0.0);
                let flexible = req["flexible"].as_f64().unwrap_or(0.0);
                let balance = income - fixed - flexible;
                let updated_at = chrono_iso_now();

                let inc_str = format!("{:.2}", income);
                let fix_str = format!("{:.2}", fixed);
                let flx_str = format!("{:.2}", flexible);
                let bal_str = format!("{:.2}", balance);

                let mut conn = state.conn.lock().unwrap();
                let check_stmt = conn.prepare("SELECT MONTH_KEY FROM SUMMARIES WHERE MONTH_KEY = :1").unwrap();
                let check_param = [Value::Text(month.clone()).into()];
                let exists = {
                    let mut rows = conn.query(&check_stmt, Parameters::positional(&check_param)).unwrap();
                    rows.next().unwrap().is_some()
                };

                if exists {
                    let update_stmt = conn.prepare(
                        "UPDATE SUMMARIES SET INCOME = :1, FIXED_COST = :2, FLEXIBLE_COST = :3, BALANCE = :4, UPDATED_AT = :5 WHERE MONTH_KEY = :6"
                    ).unwrap();
                    let params = [
                        Value::Text(inc_str).into(),
                        Value::Text(fix_str).into(),
                        Value::Text(flx_str).into(),
                        Value::Text(bal_str).into(),
                        Value::Text(updated_at).into(),
                        Value::Text(month.clone()).into(),
                    ];
                    let _ = conn.execute(&update_stmt, Parameters::positional(&params));
                } else {
                    let insert_stmt = conn.prepare("INSERT INTO SUMMARIES VALUES (:1, :2, :3, :4, :5, :6)").unwrap();
                    let params = [
                        Value::Text(month.clone()).into(),
                        Value::Text(inc_str).into(),
                        Value::Text(fix_str).into(),
                        Value::Text(flx_str).into(),
                        Value::Text(bal_str).into(),
                        Value::Text(updated_at).into(),
                    ];
                    let _ = conn.execute(&insert_stmt, Parameters::positional(&params));
                }

                send_json(&mut stream, 200, &json!({ "ok": true, "month": month, "balance": balance }));
                return;
            }
        }
    }

    // API: DELETE /api/summaries/:month
    if method == "DELETE" && path.starts_with("/api/summaries/") {
        let month = path.trim_start_matches("/api/summaries/");
        let mut conn = state.conn.lock().unwrap();
        let del_stmt = conn.prepare("DELETE FROM SUMMARIES WHERE MONTH_KEY = :1").unwrap();
        let params = [Value::Text(month.to_string()).into()];
        let _ = conn.execute(&del_stmt, Parameters::positional(&params));
        send_json(&mut stream, 200, &json!({ "ok": true, "deleted": month }));
        return;
    }

    // API: POST /api/settings
    if method == "POST" && path == "/api/settings" {
        let config_str = String::from_utf8_lossy(&body).to_string();
        let updated_at = chrono_iso_now();

        let mut conn = state.conn.lock().unwrap();
        let check_stmt = conn.prepare("SELECT CONFIG_KEY FROM SETTINGS WHERE CONFIG_KEY = 'user_settings'").unwrap();
        let exists = {
            let mut rows = conn.query(&check_stmt, Parameters::none()).unwrap();
            rows.next().unwrap().is_some()
        };

        if exists {
            let update_stmt = conn.prepare("UPDATE SETTINGS SET CONFIG_JSON = :1, UPDATED_AT = :2 WHERE CONFIG_KEY = 'user_settings'").unwrap();
            let params = [Value::Text(config_str).into(), Value::Text(updated_at).into()];
            let _ = conn.execute(&update_stmt, Parameters::positional(&params));
        } else {
            let insert_stmt = conn.prepare("INSERT INTO SETTINGS VALUES ('user_settings', :1, :2)").unwrap();
            let params = [Value::Text(config_str).into(), Value::Text(updated_at).into()];
            let _ = conn.execute(&insert_stmt, Parameters::positional(&params));
        }

        send_json(&mut stream, 200, &json!({ "ok": true }));
        return;
    }

    // 静态文件服务
    if method == "GET" {
        let rel_path = if path == "/" || path == "/index.html" {
            "index.html"
        } else {
            path.trim_start_matches('/')
        };

        if let Ok(mut f) = File::open(rel_path) {
            let mut content = Vec::new();
            if f.read_to_end(&mut content).is_ok() {
                let mime = if rel_path.ends_with(".html") {
                    "text/html; charset=utf-8"
                } else if rel_path.ends_with(".css") {
                    "text/css; charset=utf-8"
                } else if rel_path.ends_with(".js") {
                    "application/javascript; charset=utf-8"
                } else if rel_path.ends_with(".png") {
                    "image/png"
                } else if rel_path.ends_with(".svg") {
                    "image/svg+xml"
                } else {
                    "application/octet-stream"
                };
                send_response(&mut stream, 200, mime, &content);
                return;
            }
        }
    }

    send_response(&mut stream, 404, "text/plain", b"Not Found");
}

fn chrono_iso_now() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", now)
}

fn send_json(stream: &mut TcpStream, status: u16, data: &serde_json::Value) {
    let bytes = serde_json::to_vec(data).unwrap();
    send_response(stream, status, "application/json; charset=utf-8", &bytes);
}

fn send_response(stream: &mut TcpStream, status: u16, mime: &str, body: &[u8]) {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Status",
    };
    let response = format!(
        "HTTP/1.1 {} {}\r\n\
        Content-Type: {}\r\n\
        Content-Length: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
        Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
        Cache-Control: no-cache, no-store, must-revalidate\r\n\
        Connection: close\r\n\r\n",
        status, status_text, mime, body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(body);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = PathBuf::from(DB_FILE);
    let (db, conn) = init_db(&db_path)?;
    let local_ip = get_local_ip();

    let state = Arc::new(AppState {
        conn: Mutex::new(conn),
        db_path: std::fs::canonicalize(&db_path).unwrap_or(db_path),
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], PORT));
    let listener = TcpListener::bind(addr)?;

    println!("{}", "=".repeat(66));
    println!(" [打工人小账本] ORA-SQLLite (Rust 嵌入式数据库) 服务已启动！");
    println!(" * 电脑本机访问: http://localhost:{}", PORT);
    println!(" * 同局域网手机: http://{}:{}", local_ip, PORT);
    println!(" * ORA-SQLLite 存储文件: {}", state.db_path.display());
    println!(" * 底层引擎: ORA-SQLLite v0.1.0 (纯 Rust, 单文件+WAL, Oracle兼容子集)");
    println!("{}", "=".repeat(66));
    println!(" 按 Ctrl+C 即可退出服务。\n");

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            let state_clone = Arc::clone(&state);
            std::thread::spawn(move || {
                handle_connection(stream, &state_clone);
            });
        }
    }

    drop(db);
    Ok(())
}
