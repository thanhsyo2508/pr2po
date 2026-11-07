use calamine::{open_workbook_auto, Data, ExcelDateTime, ExcelDateTimeType, Reader};
use rusqlite::{backup::Backup, params, params_from_iter, types::Value, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs::{self, File}, io::Write, path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, time::Duration};
use tauri::{Manager, State};
use tauri::Emitter;
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

struct AppState(Mutex<Connection>, Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>);

struct ImportJobGuard<'a> { jobs: &'a Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>, id: String }
impl Drop for ImportJobGuard<'_> { fn drop(&mut self) { if let Ok(mut jobs)=self.jobs.lock() { jobs.remove(&self.id); } } }

fn begin_import_job<'a>(state: &'a AppState, id: String) -> Result<(Arc<AtomicBool>, ImportJobGuard<'a>), String> {
    if id.trim().is_empty() { return Err("Thiếu mã tác vụ nhập".into()); }
    let cancel=Arc::new(AtomicBool::new(false));
    state.1.lock().map_err(|_| "Không khởi tạo được tác vụ")?.insert(id.clone(),cancel.clone());
    Ok((cancel,ImportJobGuard{jobs:&state.1,id}))
}

fn report_import_progress(app:&tauri::AppHandle,id:&str,done:usize,total:usize) {
    let percent=if total==0 {100} else {(done.saturating_mul(100)/total).min(100)};
    let _=app.emit("pr2po-import-progress",serde_json::json!({"jobId":id,"done":done,"total":total,"percent":percent}));
}

#[tauri::command]
fn cancel_import(job_id:String,state:State<'_,AppState>)->Result<bool,String>{
    let jobs=state.1.lock().map_err(|_|"Không thể truy cập tác vụ nhập")?;
    if let Some(flag)=jobs.get(&job_id){flag.store(true,Ordering::Relaxed);Ok(true)}else{Ok(false)}
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbookPreview {
    sheets: Vec<String>,
    sheet: String,
    columns: usize,
    first_row: usize,
    rows: Vec<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrColumnMap {
    project: Option<usize>,
    pr: usize,
    code: usize,
    name: Option<usize>,
    spec: Option<usize>,
    unit: Option<usize>,
    quantity: usize,
    due_date: Option<usize>,
    first_data_row: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryColumnMap {
    project: Option<usize>, pr: Option<usize>, po: Option<usize>, code: usize,
    name: Option<usize>, supplier: usize, quantity: Option<usize>, unit_price: usize,
    amount: Option<usize>, first_data_row: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuoteColumnMap {
    code: usize, name: Option<usize>, spec: Option<usize>, unit: Option<usize>,
    quantity: Option<usize>, unit_price: usize, quote_number: Option<String>,
    valid_until: Option<usize>, first_data_row: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PdfQuoteRow { page: usize, line_number: usize, item_code: String, quantity: String, unit: String, unit_price: String }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportSummary { batch_id: i64, imported: usize, skipped: usize, unchanged: usize }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportSource { kind: String, id: i64, source_path: String, sheet_name: String, supplier: String, quote_number: String, imported_at: String, row_count: usize }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QuotePreviewLine { source_row: i64, item_code: String, item_name: String, spec: String, unit: String, quantity: f64, unit_price: i64, valid_until: String }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QuoteBatchPreview { source_path: String, sheet_name: String, supplier: String, quote_number: String, imported_at: String, row_count: usize, total: i64, rows: Vec<QuotePreviewLine> }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryPreviewLine { source_row: i64, source_path: String, imported_at: String, project: String, pr_number: String, po_number: String, supplier: String, quantity: f64, unit_price: i64, amount: i64 }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryPreviewPage { item_code: String, total: i64, rows: Vec<HistoryPreviewLine> }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClearSourcesResult { pr_removed: usize, pr_kept_for_history: usize, quotes_removed: usize, history_removed: usize }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Offer { supplier: String, unit_price: i64, quote_line_id: i64 }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComparisonRow {
    id: i64, project: String, pr: String, code: String, name: String, spec: String,
    unit: String, quantity: f64, history_price: Option<i64>, offers: Vec<Offer>,
    suggested_supplier: Option<String>, suggested_price: Option<i64>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AllocationInput { pr_line_id: i64, supplier: String, quantity: f64, unit_price: i64, quote_line_id: Option<i64>, #[serde(default)] selection_reason: String }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportReview { total_rows: usize, new_rows: usize, changed_rows: usize, unchanged_rows: usize, duplicate_rows: usize, error_rows: usize, examples: Vec<String> }

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupSource { original: String, archive: String }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PoSummary { id: i64, supplier: String, po_number: String, lines: usize, total: i64 }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkBatchSummary { id: i64, name: String, created_at: String, lines: usize, allocated_lines: usize, no_quote_lines: usize, po_count: usize, total_amount: i64 }

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryDraftPayload { work_batch_id: Option<i64>, work_name: String, pr_ids: Vec<i64>, allocations: Vec<AllocationInput> }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryDraftRecord { work_batch_id: Option<i64>, work_name: String, pr_ids: Vec<i64>, allocations: Vec<AllocationInput>, updated_at: String }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DataHealth { last_backup_at: Option<String>, missing_sources: Vec<String>, duplicate_quote_lines: usize }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PoReviewLine { pr_line_id: i64, project: String, pr_number: String, item_code: String, item_name: String, spec: String, unit: String, source_row: i64, requested: f64, allocated: f64, amount: i64, suppliers: String, issue: String }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PoReview { total_lines: usize, full_lines: usize, partial_lines: usize, unallocated_lines: usize, no_quote_lines: usize, issue_total: usize, total_amount: i64, rows: Vec<PoReviewLine> }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkReportSummary { total_lines: usize, allocated_lines: usize, full_lines: usize, partial_lines: usize, unallocated_lines: usize, no_quote_lines: usize, over_allocated_lines: usize, expired_quote_lines: usize, stale_quote_lines: usize, manual_price_lines: usize, anomaly_lines: usize, total_amount: i64 }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkReportLine { pr_line_id: i64, project: String, pr_number: String, item_code: String, item_name: String, requested: f64, allocated: f64, amount: i64, suppliers: String, status: String, best_supplier: String, best_price: Option<i64>, selected_price: Option<i64>, history_price: Option<i64>, selection_delta_percent: Option<f64>, history_delta_percent: Option<f64>, quote_spread_percent: Option<f64>, selection_reason: String, quote_numbers: String, quote_sources: String, quote_sheets: String, quote_rows: String, expired_quotes: usize, stale_quotes: usize, manual_price: bool, anomaly: bool }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkReport { work_batch_id: i64, work_batch_name: String, summary: WorkReportSummary, total_rows: usize, rows: Vec<WorkReportLine>, supplier_totals: Vec<WorkReportSupplier> }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkReportSupplier { supplier: String, lines: usize, amount: i64, share_percent: f64 }

#[derive(Clone)]
struct ReportQuote { supplier: String, price: i64, valid_until: String, age_days: i64 }

#[derive(Clone,Default)]
struct ReportAllocation { quantity: f64, amount: i64, suppliers: String, reason: String, quote_numbers: String, quote_sources: String, quote_sheets: String, quote_rows: String, manual_price: bool }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SupplierPriority { supplier: String, priority: i64 }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportResult {
    batch_id: i64,
    imported: usize,
    skipped: usize,
    unchanged: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrRow {
    id: String, project: String, pr: String, code: String, name: String, spec: String,
    unit: String, qty: f64, ordered: f64, due: String, status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrPage { rows: Vec<PrRow>, total: i64, distinct_prs: i64, remaining_units: f64 }

fn db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("pr2po.sqlite3"))
}

fn ensure_work_batch_scope_columns(db:&Connection)->Result<(),String>{
    for (column,definition) in [("project","TEXT NOT NULL DEFAULT ''"),("pr_number","TEXT NOT NULL DEFAULT ''"),("item_code","TEXT NOT NULL DEFAULT ''"),("item_name","TEXT NOT NULL DEFAULT ''"),("spec","TEXT NOT NULL DEFAULT ''"),("unit","TEXT NOT NULL DEFAULT ''"),("quantity","REAL NOT NULL DEFAULT 0"),("ordered","REAL NOT NULL DEFAULT 0"),("source_row","INTEGER NOT NULL DEFAULT 0")] {
        let exists:bool=db.prepare("PRAGMA table_info(work_batch_scope)").map_err(|e|e.to_string())?.query_map([],|r|r.get::<_,String>(1)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?.iter().any(|name|name==column);
        if !exists{db.execute(&format!("ALTER TABLE work_batch_scope ADD COLUMN {column} {definition}"),[]).map_err(|e|e.to_string())?;}
    }
    db.execute("UPDATE work_batch_scope SET project=COALESCE((SELECT project FROM pr_line WHERE id=work_batch_scope.pr_line_id),project),pr_number=COALESCE((SELECT pr_number FROM pr_line WHERE id=work_batch_scope.pr_line_id),pr_number),item_code=COALESCE((SELECT item_code FROM pr_line WHERE id=work_batch_scope.pr_line_id),item_code),item_name=COALESCE((SELECT item_name FROM pr_line WHERE id=work_batch_scope.pr_line_id),item_name),spec=COALESCE((SELECT spec FROM pr_line WHERE id=work_batch_scope.pr_line_id),spec),unit=COALESCE((SELECT unit FROM pr_line WHERE id=work_batch_scope.pr_line_id),unit),quantity=COALESCE((SELECT quantity FROM pr_line WHERE id=work_batch_scope.pr_line_id),quantity),ordered=COALESCE((SELECT ordered FROM pr_line WHERE id=work_batch_scope.pr_line_id),ordered),source_row=COALESCE((SELECT source_row FROM pr_line WHERE id=work_batch_scope.pr_line_id),source_row) WHERE pr_number=''",[]).map_err(|e|e.to_string())?;
    Ok(())
}

fn ensure_allocation_report_columns(db:&Connection)->Result<(),String>{
    let exists:bool=db.prepare("PRAGMA table_info(allocation)").map_err(|e|e.to_string())?.query_map([],|r|r.get::<_,String>(1)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?.iter().any(|name|name=="selection_reason");
    if !exists{db.execute("ALTER TABLE allocation ADD COLUMN selection_reason TEXT NOT NULL DEFAULT ''",[]).map_err(|e|e.to_string())?;}
    Ok(())
}

fn ensure_quote_provenance_columns(db:&Connection)->Result<(),String>{
    let batch_has_sheet:bool=db.prepare("PRAGMA table_info(quote_batch)").map_err(|e|e.to_string())?.query_map([],|r|r.get::<_,String>(1)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?.iter().any(|name|name=="sheet_name");
    if !batch_has_sheet{db.execute("ALTER TABLE quote_batch ADD COLUMN sheet_name TEXT NOT NULL DEFAULT ''",[]).map_err(|e|e.to_string())?;}
    let line_has_page:bool=db.prepare("PRAGMA table_info(quote_line)").map_err(|e|e.to_string())?.query_map([],|r|r.get::<_,String>(1)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?.iter().any(|name|name=="source_page");
    if !line_has_page{db.execute("ALTER TABLE quote_line ADD COLUMN source_page INTEGER NOT NULL DEFAULT 0",[]).map_err(|e|e.to_string())?;}
    Ok(())
}

fn init_db(path: PathBuf) -> Result<Connection, String> {
    let db = Connection::open(path).map_err(|e| e.to_string())?;
    db.pragma_update(None, "journal_mode", "WAL").map_err(|e| e.to_string())?;
    db.pragma_update(None, "synchronous", "NORMAL").map_err(|e| e.to_string())?;
    db.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS import_batch (
           id INTEGER PRIMARY KEY, source_path TEXT NOT NULL, sheet_name TEXT NOT NULL, file_hash TEXT NOT NULL DEFAULT '',
           imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, row_count INTEGER NOT NULL DEFAULT 0,
           is_active INTEGER NOT NULL DEFAULT 1
         );
         CREATE TABLE IF NOT EXISTS pr_line (
           id INTEGER PRIMARY KEY, batch_id INTEGER NOT NULL REFERENCES import_batch(id),
           source_row INTEGER NOT NULL, project TEXT NOT NULL DEFAULT '', pr_number TEXT NOT NULL,
           item_code TEXT NOT NULL, item_name TEXT NOT NULL DEFAULT '', spec TEXT NOT NULL DEFAULT '',
           unit TEXT NOT NULL DEFAULT '', quantity REAL NOT NULL, due_date TEXT NOT NULL DEFAULT '',
           ordered REAL NOT NULL DEFAULT 0, status TEXT NOT NULL DEFAULT 'Chưa xử lý'
         );
         CREATE TABLE IF NOT EXISTS history_batch (
           id INTEGER PRIMARY KEY, source_path TEXT NOT NULL, file_hash TEXT NOT NULL UNIQUE,
           imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, row_count INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS purchase_history (
           id INTEGER PRIMARY KEY, batch_id INTEGER NOT NULL REFERENCES history_batch(id), source_row INTEGER NOT NULL,
           source_path TEXT NOT NULL DEFAULT '',
           project TEXT NOT NULL DEFAULT '', pr_number TEXT NOT NULL DEFAULT '', po_number TEXT NOT NULL DEFAULT '',
           item_code TEXT NOT NULL, item_name TEXT NOT NULL DEFAULT '', supplier TEXT NOT NULL,
           quantity REAL NOT NULL DEFAULT 0, unit_price INTEGER NOT NULL DEFAULT 0, amount INTEGER NOT NULL DEFAULT 0
         );
         CREATE INDEX IF NOT EXISTS idx_history_item_supplier ON purchase_history(item_code, supplier, batch_id);
         CREATE TABLE IF NOT EXISTS quote_batch (
           id INTEGER PRIMARY KEY, source_path TEXT NOT NULL, file_hash TEXT NOT NULL, supplier TEXT NOT NULL,
           quote_number TEXT NOT NULL DEFAULT '', imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           row_count INTEGER NOT NULL DEFAULT 0
         );
         CREATE INDEX IF NOT EXISTS idx_quote_supplier_hash ON quote_batch(supplier, file_hash);
         CREATE TABLE IF NOT EXISTS quote_line (
           id INTEGER PRIMARY KEY, batch_id INTEGER NOT NULL REFERENCES quote_batch(id), source_row INTEGER NOT NULL,
           item_code TEXT NOT NULL, item_name TEXT NOT NULL DEFAULT '', spec TEXT NOT NULL DEFAULT '', unit TEXT NOT NULL DEFAULT '',
           quantity REAL NOT NULL DEFAULT 0, unit_price INTEGER NOT NULL, valid_until TEXT NOT NULL DEFAULT ''
         );
         CREATE INDEX IF NOT EXISTS idx_quote_batch_code ON quote_line(batch_id, item_code);
         CREATE TABLE IF NOT EXISTS supplier_priority (supplier TEXT PRIMARY KEY, priority INTEGER NOT NULL DEFAULT 100);
         CREATE TABLE IF NOT EXISTS work_batch (id INTEGER PRIMARY KEY, name TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
         CREATE TABLE IF NOT EXISTS work_batch_no_quote (
           work_batch_id INTEGER NOT NULL REFERENCES work_batch(id) ON DELETE CASCADE, pr_line_id INTEGER NOT NULL,
           project TEXT NOT NULL DEFAULT '', pr_number TEXT NOT NULL, item_code TEXT NOT NULL, item_name TEXT NOT NULL DEFAULT '',
           spec TEXT NOT NULL DEFAULT '', unit TEXT NOT NULL DEFAULT '', quantity REAL NOT NULL DEFAULT 0,
           source_path TEXT NOT NULL DEFAULT '', source_row INTEGER NOT NULL DEFAULT 0,
           PRIMARY KEY(work_batch_id, pr_line_id)
         );
         CREATE TABLE IF NOT EXISTS work_batch_scope (
           work_batch_id INTEGER NOT NULL REFERENCES work_batch(id) ON DELETE CASCADE,
           pr_line_id INTEGER NOT NULL, project TEXT NOT NULL DEFAULT '', pr_number TEXT NOT NULL DEFAULT '',
           item_code TEXT NOT NULL DEFAULT '', item_name TEXT NOT NULL DEFAULT '', spec TEXT NOT NULL DEFAULT '', unit TEXT NOT NULL DEFAULT '',
           quantity REAL NOT NULL DEFAULT 0, ordered REAL NOT NULL DEFAULT 0, source_row INTEGER NOT NULL DEFAULT 0,
           PRIMARY KEY(work_batch_id, pr_line_id)
         );
         CREATE TABLE IF NOT EXISTS audit_event (id INTEGER PRIMARY KEY, event_type TEXT NOT NULL, entity_type TEXT NOT NULL, entity_id INTEGER NOT NULL DEFAULT 0, details TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
         CREATE TABLE IF NOT EXISTS recovery_draft (id INTEGER PRIMARY KEY CHECK(id=1), payload TEXT NOT NULL, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
         CREATE TABLE IF NOT EXISTS allocation (
           id INTEGER PRIMARY KEY, work_batch_id INTEGER NOT NULL REFERENCES work_batch(id), pr_line_id INTEGER NOT NULL REFERENCES pr_line(id),
           supplier TEXT NOT NULL, quantity REAL NOT NULL, unit_price INTEGER NOT NULL, quote_line_id INTEGER REFERENCES quote_line(id)
         );
         CREATE INDEX IF NOT EXISTS idx_allocation_batch ON allocation(work_batch_id, supplier);
         CREATE TABLE IF NOT EXISTS po_header (
           id INTEGER PRIMARY KEY, work_batch_id INTEGER NOT NULL REFERENCES work_batch(id), supplier TEXT NOT NULL,
           po_number TEXT NOT NULL, total INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           UNIQUE(work_batch_id, supplier)
         );
         CREATE TABLE IF NOT EXISTS po_line (
           id INTEGER PRIMARY KEY, po_id INTEGER NOT NULL REFERENCES po_header(id) ON DELETE CASCADE,
           pr_number TEXT NOT NULL, item_code TEXT NOT NULL, item_name TEXT NOT NULL DEFAULT '', spec TEXT NOT NULL DEFAULT '',
           unit TEXT NOT NULL DEFAULT '', quantity REAL NOT NULL, unit_price INTEGER NOT NULL, amount INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_pr_batch_project ON pr_line(batch_id, project, pr_number);
         CREATE INDEX IF NOT EXISTS idx_pr_batch_pr ON pr_line(batch_id, pr_number);
         CREATE INDEX IF NOT EXISTS idx_pr_batch_code ON pr_line(batch_id, item_code);"
    ).map_err(|e| e.to_string())?;
    ensure_work_batch_scope_columns(&db)?;
    ensure_allocation_report_columns(&db)?;
    ensure_quote_provenance_columns(&db)?;
    let has_source_path: bool = db.prepare("PRAGMA table_info(purchase_history)").map_err(|e| e.to_string())?
        .query_map([], |r| r.get::<_, String>(1)).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?.iter().any(|n| n == "source_path");
    if !has_source_path { db.execute("ALTER TABLE purchase_history ADD COLUMN source_path TEXT NOT NULL DEFAULT ''", []).map_err(|e| e.to_string())?; }
    db.execute("UPDATE purchase_history SET source_path=(SELECT source_path FROM history_batch WHERE history_batch.id=purchase_history.batch_id) WHERE source_path=''", []).map_err(|e| e.to_string())?;
    db.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_history_source_row ON purchase_history(source_path,source_row)", []).map_err(|e| e.to_string())?;
    let has_hash: bool = db.prepare("PRAGMA table_info(import_batch)").map_err(|e| e.to_string())?
        .query_map([], |r| r.get::<_, String>(1)).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?.iter().any(|n| n == "file_hash");
    if !has_hash { db.execute("ALTER TABLE import_batch ADD COLUMN file_hash TEXT NOT NULL DEFAULT ''", []).map_err(|e| e.to_string())?; }
    let has_is_active: bool = db.prepare("PRAGMA table_info(import_batch)").map_err(|e| e.to_string())?
        .query_map([], |r| r.get::<_, String>(1)).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?.iter().any(|n| n == "is_active");
    if !has_is_active { db.execute("ALTER TABLE import_batch ADD COLUMN is_active INTEGER NOT NULL DEFAULT 1", []).map_err(|e| e.to_string())?; }
    db.execute("CREATE TABLE IF NOT EXISTS app_state (key TEXT PRIMARY KEY, value TEXT NOT NULL)", []).map_err(|e| e.to_string())?;
    db.execute("INSERT OR IGNORE INTO app_state(key,value) VALUES ('source_data_cleared','0')", []).map_err(|e| e.to_string())?;
    db.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_import_file_hash ON import_batch(file_hash) WHERE file_hash <> ''", []).map_err(|e| e.to_string())?;
    // Some PR exports omit a project column but encode it in DES-<project>-<...>.
    // Backfill old imports so project filtering also works for previously loaded files.
    db.execute("UPDATE pr_line SET project=substr(pr_number,5,instr(substr(pr_number,5),'-')-1) WHERE project='' AND pr_number LIKE 'DES-%' AND instr(substr(pr_number,5),'-')>1", []).map_err(|e| e.to_string())?;
    Ok(db)
}

fn file_hash(path: &str) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Không đọc được file: {e}"))?;
    Ok(Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect())
}

fn money(text: String) -> Option<i64> {
    let text=text.trim().replace(',',"");
    if let Ok(value)=text.parse::<f64>(){return Some(value.round() as i64)}
    let digits=text.chars().filter(|c|c.is_ascii_digit()||*c=='-').collect::<String>();
    digits.parse::<i64>().ok()
}

fn cell_text(row: &[calamine::Data], index: Option<usize>) -> String {
    index.and_then(|i| row.get(i)).map(cell_value_text).unwrap_or_default()
}

fn cell_value_text(cell: &Data) -> String {
    match cell {
        Data::DateTime(value) => format_excel_datetime(*value),
        Data::DateTimeIso(value) => {
            normalize_iso_date(value)
        }
        Data::String(value) => normalize_iso_date(value),
        _ => cell.to_string().trim().to_string(),
    }
}

fn format_excel_datetime(value: ExcelDateTime) -> String {
    let (year, month, day, hour, minute, _, _) = value.to_ymd_hms_milli();
    if hour == 0 && minute == 0 { format!("{day:02}/{month:02}/{year:04}") }
    else { format!("{day:02}/{month:02}/{year:04} {hour:02}:{minute:02}") }
}

fn normalize_iso_date(value: &str) -> String {
    let date = value.split('T').next().unwrap_or(value);
    let mut parts = date.split('-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(year), Some(month), Some(day)) if year.len() == 4 && month.len() <= 2 && day.len() <= 2 =>
            match (year.parse::<u16>(), month.parse::<u8>(), day.parse::<u8>()) {
                (Ok(y), Ok(m), Ok(d)) if m > 0 && m <= 12 && d > 0 && d <= 31 => format!("{d:02}/{m:02}/{y:04}"),
                _ => value.to_string(),
            },
        _ => value.to_string(),
    }
}

fn format_excel_serial_date(value: f64) -> Option<String> {
    if !value.is_finite() || !(1.0..2_958_466.0).contains(&value) { return None; }
    Some(format_excel_datetime(ExcelDateTime::new(value, ExcelDateTimeType::DateTime, false)))
}

fn display_date_text(value: String) -> String {
    value.parse::<f64>().ok().and_then(format_excel_serial_date).unwrap_or_else(|| normalize_iso_date(&value))
}

fn date_cell_text(row: &[Data], index: Option<usize>) -> String {
    match index.and_then(|i| row.get(i)) {
        Some(Data::Int(value)) => format_excel_serial_date(*value as f64).unwrap_or_else(|| value.to_string()),
        Some(Data::Float(value)) => format_excel_serial_date(*value).unwrap_or_else(|| value.to_string()),
        Some(value) => cell_value_text(value),
        None => String::new(),
    }
}

fn project_from_pr(pr: &str) -> String {
    let parts: Vec<&str> = pr.split('-').collect();
    let Some(first) = parts.first() else { return String::new() };
    match first.to_ascii_uppercase().as_str() {
        "COMMON" | "GA" => return first.to_ascii_uppercase(),
        sales if sales.starts_with("SALES") => return "SALES".to_string(),
        "DES" | "AGV" => return parts.get(1).copied().unwrap_or(first).to_string(),
        _ => {}
    }
    let base = if parts.last().is_some_and(|last| last.len() == 6 && last.bytes().all(|b| b.is_ascii_digit())) {
        pr.rsplit_once('-').map(|(prefix, _)| prefix).unwrap_or(pr)
    } else { pr };
    let base_first = base.split('-').next().unwrap_or(base);
    if base_first.bytes().any(|b| b.is_ascii_digit()) { base_first.to_string() } else { base.to_string() }
}

const PR_PROJECT_EXPR_SQL: &str = "CASE WHEN INSTR(pr_number,'-')=0 AND UPPER(pr_number) LIKE 'SALES%' THEN 'SALES' WHEN INSTR(pr_number,'-')=0 THEN pr_number WHEN UPPER(SUBSTR(pr_number,1,INSTR(pr_number,'-')-1))='COMMON' THEN 'COMMON' WHEN UPPER(SUBSTR(pr_number,1,INSTR(pr_number,'-')-1))='GA' THEN 'GA' WHEN UPPER(SUBSTR(pr_number,1,INSTR(pr_number,'-')-1)) LIKE 'SALES%' THEN 'SALES' WHEN UPPER(SUBSTR(pr_number,1,INSTR(pr_number,'-')-1)) IN ('DES','AGV') THEN SUBSTR(SUBSTR(pr_number,INSTR(pr_number,'-')+1),1,CASE WHEN INSTR(SUBSTR(pr_number,INSTR(pr_number,'-')+1),'-')>0 THEN INSTR(SUBSTR(pr_number,INSTR(pr_number,'-')+1),'-')-1 ELSE LENGTH(pr_number) END) ELSE CASE WHEN SUBSTR(pr_number,LENGTH(pr_number)-6,1)='-' AND SUBSTR(pr_number,LENGTH(pr_number)-5,6) GLOB '[0-9][0-9][0-9][0-9][0-9][0-9]' THEN CASE WHEN SUBSTR(pr_number,1,INSTR(pr_number,'-')-1) GLOB '*[0-9]*' THEN SUBSTR(pr_number,1,INSTR(pr_number,'-')-1) ELSE SUBSTR(pr_number,1,LENGTH(pr_number)-7) END WHEN SUBSTR(pr_number,1,INSTR(pr_number,'-')-1) GLOB '*[0-9]*' THEN SUBSTR(pr_number,1,INSTR(pr_number,'-')-1) ELSE pr_number END END";

fn stored_project_expr_sql() -> String {
    format!("CASE WHEN UPPER(pr_number) LIKE 'COMMON-%' OR UPPER(SUBSTR(pr_number,1,INSTR(pr_number,'-')-1))='GA' OR UPPER(SUBSTR(pr_number,1,INSTR(pr_number,'-')-1)) LIKE 'SALES%' THEN ({PR_PROJECT_EXPR_SQL}) ELSE COALESCE(NULLIF(project,''),({PR_PROJECT_EXPR_SQL})) END")
}

#[tauri::command]
fn get_import_sources(state: State<'_, AppState>) -> Result<Vec<ImportSource>, String> {
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    let mut stmt = db.prepare(
        "SELECT kind,id,source_path,sheet_name,supplier,quote_number,imported_at,row_count FROM (
           SELECT 'pr' AS kind,id,source_path,sheet_name,'' AS supplier,'' AS quote_number,imported_at,row_count FROM import_batch WHERE is_active=1
           UNION ALL
           SELECT 'history',id,source_path,'' AS sheet_name,'' AS supplier,'' AS quote_number,imported_at,row_count FROM history_batch
           UNION ALL
           SELECT 'quote',id,source_path,sheet_name,supplier,quote_number,imported_at,row_count FROM quote_batch
         ) ORDER BY imported_at DESC,id DESC LIMIT 1000"
    ).map_err(|e| e.to_string())?;
    stmt.query_map([], |r| Ok(ImportSource {
        kind: r.get(0)?, id: r.get(1)?, source_path: r.get(2)?, sheet_name: r.get(3)?,
        supplier: r.get(4)?, quote_number: r.get(5)?, imported_at: r.get(6)?,
        row_count: r.get::<_, i64>(7)?.max(0) as usize,
    })).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_recovery_draft(state:State<'_,AppState>)->Result<Option<RecoveryDraftRecord>,String>{
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let record=db.query_row("SELECT payload,updated_at FROM recovery_draft WHERE id=1",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(|e|e.to_string())?;
    record.map(|(payload,updated_at)|{let draft:RecoveryDraftPayload=serde_json::from_str(&payload).map_err(|e|e.to_string())?;Ok(RecoveryDraftRecord{work_batch_id:draft.work_batch_id,work_name:draft.work_name,pr_ids:draft.pr_ids,allocations:draft.allocations,updated_at})}).transpose()
}

#[tauri::command]
fn save_recovery_draft(draft:RecoveryDraftPayload,state:State<'_,AppState>)->Result<(),String>{
    if draft.pr_ids.len()>300_000||draft.allocations.len()>500_000{return Err("Bản nháp vượt giới hạn an toàn để tự lưu".into())}
    let payload=serde_json::to_string(&draft).map_err(|e|e.to_string())?;
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    db.execute("INSERT INTO recovery_draft(id,payload,updated_at) VALUES (1,?1,datetime('now')) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,updated_at=excluded.updated_at",[payload]).map_err(|e|e.to_string())?;Ok(())
}

#[tauri::command]
fn clear_recovery_draft(state:State<'_,AppState>)->Result<(),String>{state.0.lock().map_err(|_|"Database đang bận")?.execute("DELETE FROM recovery_draft WHERE id=1",[]).map_err(|e|e.to_string())?;Ok(())}

#[tauri::command]
fn get_data_health(state:State<'_,AppState>)->Result<DataHealth,String>{
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let last_backup_at=db.query_row("SELECT value FROM app_state WHERE key='last_backup_at'",[],|r|r.get::<_,String>(0)).optional().map_err(|e|e.to_string())?;
    let sources={let mut stmt=db.prepare("SELECT source_path FROM import_batch UNION SELECT source_path FROM history_batch UNION SELECT source_path FROM quote_batch").map_err(|e|e.to_string())?;stmt.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?};
    let missing_sources=sources.into_iter().filter(|path|!Path::new(path).is_file()).collect();
    let duplicate_quote_lines:i64=db.query_row("SELECT COUNT(*) FROM (SELECT batch_id,item_code FROM quote_line GROUP BY batch_id,item_code HAVING COUNT(*)>1)",[],|r|r.get(0)).map_err(|e|e.to_string())?;
    Ok(DataHealth{last_backup_at,missing_sources,duplicate_quote_lines:duplicate_quote_lines.max(0) as usize})
}

#[tauri::command]
fn get_quote_batch_preview(quote_batch_id: i64, offset: i64, limit: i64, query: String, state: State<'_, AppState>) -> Result<QuoteBatchPreview, String> {
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    let (source_path, sheet_name, supplier, quote_number, imported_at, row_count) = db.query_row(
        "SELECT source_path,sheet_name,supplier,quote_number,imported_at,row_count FROM quote_batch WHERE id=?1",
        [quote_batch_id],
        |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,i64>(5)?)),
    ).optional().map_err(|e|e.to_string())?.ok_or("Không tìm thấy báo giá này")?;
    let query=query.trim().to_string();
    let pattern=format!("%{query}%");
    let filter="batch_id=?1 AND (?2='' OR item_code LIKE ?3 COLLATE NOCASE OR item_name LIKE ?3 COLLATE NOCASE OR spec LIKE ?3 COLLATE NOCASE)";
    let total=db.query_row(&format!("SELECT COUNT(*) FROM quote_line WHERE {filter}"),params![quote_batch_id,query,pattern],|row|row.get::<_,i64>(0)).map_err(|e|e.to_string())?;
    let mut stmt=db.prepare(&format!("SELECT source_row,item_code,item_name,spec,unit,quantity,unit_price,valid_until FROM quote_line WHERE {filter} ORDER BY source_row,id LIMIT ?4 OFFSET ?5")).map_err(|e|e.to_string())?;
    let rows=stmt.query_map(params![quote_batch_id,query,pattern,limit.clamp(1,200),offset.max(0)],|row|Ok(QuotePreviewLine{source_row:row.get(0)?,item_code:row.get(1)?,item_name:row.get(2)?,spec:row.get(3)?,unit:row.get(4)?,quantity:row.get(5)?,unit_price:row.get(6)?,valid_until:row.get(7)?})).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    Ok(QuoteBatchPreview{source_path,sheet_name,supplier,quote_number,imported_at,row_count:row_count.max(0) as usize,total,rows})
}

#[tauri::command]
fn get_item_history_preview(item_code: String, offset: i64, limit: i64, state: State<'_, AppState>) -> Result<HistoryPreviewPage, String> {
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    let total: i64 = db.query_row("SELECT COUNT(*) FROM purchase_history WHERE item_code=?1 AND unit_price>0", [&item_code], |r| r.get(0)).map_err(|e| e.to_string())?;
    let mut stmt = db.prepare("SELECT h.source_row,h.source_path,b.imported_at,h.project,h.pr_number,h.po_number,h.supplier,h.quantity,h.unit_price,h.amount FROM purchase_history h JOIN history_batch b ON b.id=h.batch_id WHERE h.item_code=?1 AND h.unit_price>0 ORDER BY h.id DESC LIMIT ?2 OFFSET ?3").map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![item_code, limit.clamp(1, 200), offset.max(0)], |r| Ok(HistoryPreviewLine {
        source_row: r.get(0)?, source_path: r.get(1)?, imported_at: r.get(2)?, project: r.get(3)?, pr_number: r.get(4)?, po_number: r.get(5)?, supplier: r.get(6)?, quantity: r.get(7)?, unit_price: r.get(8)?, amount: r.get(9)?,
    })).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(HistoryPreviewPage { item_code, total, rows })
}

#[tauri::command]
fn delete_import_source(kind: String, id: i64, state: State<'_, AppState>) -> Result<ClearSourcesResult, String> {
    let mut db = state.0.lock().map_err(|_| "Database đang bận")?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let mut result = ClearSourcesResult { pr_removed: 0, pr_kept_for_history: 0, quotes_removed: 0, history_removed: 0 };
    match kind.as_str() {
        "pr" => {
            let before: i64 = tx.query_row("SELECT COUNT(*) FROM pr_line WHERE batch_id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM pr_line WHERE batch_id=?1 AND NOT EXISTS (SELECT 1 FROM allocation WHERE allocation.pr_line_id=pr_line.id)", [id]).map_err(|e| e.to_string())?;
            let kept: i64 = tx.query_row("SELECT COUNT(*) FROM pr_line WHERE batch_id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
            tx.execute("UPDATE import_batch SET is_active=0,file_hash='',row_count=?1 WHERE id=?2", params![kept,id]).map_err(|e| e.to_string())?;
            result.pr_removed = before.saturating_sub(kept).max(0) as usize;
            result.pr_kept_for_history = kept.max(0) as usize;
        }
        "quote" => {
            result.quotes_removed = tx.query_row("SELECT COUNT(*) FROM quote_line WHERE batch_id=?1", [id], |r| r.get::<_,i64>(0)).map_err(|e| e.to_string())?.max(0) as usize;
            tx.execute("UPDATE allocation SET quote_line_id=NULL WHERE quote_line_id IN (SELECT id FROM quote_line WHERE batch_id=?1)", [id]).map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM quote_line WHERE batch_id=?1", [id]).map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM quote_batch WHERE id=?1", [id]).map_err(|e| e.to_string())?;
        }
        "history" => {
            result.history_removed = tx.query_row("SELECT COUNT(*) FROM purchase_history WHERE batch_id=?1", [id], |r| r.get::<_,i64>(0)).map_err(|e| e.to_string())?.max(0) as usize;
            tx.execute("DELETE FROM purchase_history WHERE batch_id=?1", [id]).map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM history_batch WHERE id=?1", [id]).map_err(|e| e.to_string())?;
        }
        _ => return Err("Loại dữ liệu cần xóa không hợp lệ".into()),
    }
    tx.execute("INSERT INTO audit_event(event_type,entity_type,entity_id,details) VALUES ('undo_import','source',?1,?2)", params![id,format!("kind={kind}; removed_pr={}; kept_pr={}; removed_quotes={}; removed_history={}",result.pr_removed,result.pr_kept_for_history,result.quotes_removed,result.history_removed)]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

#[tauri::command]
fn get_source_data_cleared(state: State<'_, AppState>) -> Result<bool, String> {
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    let value: String = db.query_row("SELECT value FROM app_state WHERE key='source_data_cleared'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    Ok(value == "1")
}

#[tauri::command]
fn clear_source_data(state: State<'_, AppState>) -> Result<ClearSourcesResult, String> {
    let mut db = state.0.lock().map_err(|_| "Database đang bận")?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let pr_before: i64 = tx.query_row("SELECT COUNT(*) FROM pr_line", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let quotes_before: i64 = tx.query_row("SELECT COUNT(*) FROM quote_line", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let history_before: i64 = tx.query_row("SELECT COUNT(*) FROM purchase_history", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    tx.execute("UPDATE allocation SET quote_line_id=NULL WHERE quote_line_id IS NOT NULL", []).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM quote_line", []).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM quote_batch", []).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM purchase_history", []).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM history_batch", []).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM pr_line WHERE NOT EXISTS (SELECT 1 FROM allocation WHERE allocation.pr_line_id=pr_line.id)", []).map_err(|e| e.to_string())?;
    let pr_kept: i64 = tx.query_row("SELECT COUNT(*) FROM pr_line", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    tx.execute("UPDATE import_batch SET is_active=0,file_hash='',row_count=(SELECT COUNT(*) FROM pr_line WHERE pr_line.batch_id=import_batch.id)", []).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM import_batch WHERE NOT EXISTS (SELECT 1 FROM pr_line WHERE pr_line.batch_id=import_batch.id)", []).map_err(|e| e.to_string())?;
    tx.execute("UPDATE app_state SET value='1' WHERE key='source_data_cleared'", []).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(ClearSourcesResult { pr_removed: pr_before.saturating_sub(pr_kept).max(0) as usize, pr_kept_for_history: pr_kept.max(0) as usize, quotes_removed: quotes_before.max(0) as usize, history_removed: history_before.max(0) as usize })
}

#[tauri::command]
fn clear_source_kind(kind: String, state: State<'_, AppState>) -> Result<ClearSourcesResult, String> {
    let mut db = state.0.lock().map_err(|_| "Database đang bận")?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let mut result = ClearSourcesResult { pr_removed: 0, pr_kept_for_history: 0, quotes_removed: 0, history_removed: 0 };
    match kind.as_str() {
        "pr" => {
            let before: i64 = tx.query_row("SELECT COUNT(*) FROM pr_line", [], |r| r.get(0)).map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM pr_line WHERE NOT EXISTS (SELECT 1 FROM allocation WHERE allocation.pr_line_id=pr_line.id)", []).map_err(|e| e.to_string())?;
            let kept: i64 = tx.query_row("SELECT COUNT(*) FROM pr_line", [], |r| r.get(0)).map_err(|e| e.to_string())?;
            tx.execute("UPDATE import_batch SET is_active=0,file_hash='',row_count=(SELECT COUNT(*) FROM pr_line WHERE pr_line.batch_id=import_batch.id)", []).map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM import_batch WHERE NOT EXISTS (SELECT 1 FROM pr_line WHERE pr_line.batch_id=import_batch.id)", []).map_err(|e| e.to_string())?;
            tx.execute("UPDATE app_state SET value='1' WHERE key='source_data_cleared'", []).map_err(|e| e.to_string())?;
            result.pr_removed = before.saturating_sub(kept).max(0) as usize;
            result.pr_kept_for_history = kept.max(0) as usize;
        }
        "quote" => {
            let count: i64 = tx.query_row("SELECT COUNT(*) FROM quote_line", [], |r| r.get(0)).map_err(|e| e.to_string())?;
            tx.execute("UPDATE allocation SET quote_line_id=NULL WHERE quote_line_id IS NOT NULL", []).map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM quote_line", []).map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM quote_batch", []).map_err(|e| e.to_string())?;
            result.quotes_removed = count.max(0) as usize;
        }
        "history" => {
            let count: i64 = tx.query_row("SELECT COUNT(*) FROM purchase_history", [], |r| r.get(0)).map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM purchase_history", []).map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM history_batch", []).map_err(|e| e.to_string())?;
            result.history_removed = count.max(0) as usize;
        }
        _ => return Err("Loại dữ liệu cần xóa không hợp lệ".into()),
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

#[tauri::command]
fn preview_workbook(path: String, sheet: Option<String>) -> Result<WorkbookPreview, String> {
    let mut workbook = open_workbook_auto(&path).map_err(|e| format!("Không mở được Excel: {e}"))?;
    let sheets = workbook.sheet_names().to_vec();
    let selected = sheet.filter(|s| sheets.contains(s)).or_else(|| sheets.first().cloned()).ok_or("Workbook không có sheet")?;
    let range = workbook.worksheet_range(&selected).map_err(|e| format!("Không đọc được sheet: {e}"))?;
    let columns = range.width();
    let first_row=range.start().map(|(r,_)|r as usize+1).unwrap_or(1);
    let rows = range.rows().take(31).map(|row| {
        (0..columns).map(|i| cell_text(row, Some(i))).collect()
    }).collect();
    Ok(WorkbookPreview { sheets, sheet: selected, columns, first_row, rows })
}

#[tauri::command]
fn review_workbook(path:String,sheet:String,kind:String,mapping:serde_json::Value,supplier:Option<String>,app:tauri::AppHandle,job_id:String,state:State<'_,AppState>)->Result<ImportReview,String>{
    let (cancel,_job_guard)=begin_import_job(&state,job_id.clone())?;
    if !["pr","history","quote"].contains(&kind.as_str()){return Err("Loại dữ liệu không hợp lệ".into())}
    let column=|name:&str|mapping.get(name).and_then(|v|v.as_u64()).map(|n|n as usize);
    let first=column("firstDataRow").unwrap_or(1).max(1);
    let mut workbook=open_workbook_auto(&path).map_err(|e|format!("Không mở được Excel: {e}"))?;
    let range=workbook.worksheet_range(&sheet).map_err(|e|format!("Không đọc được sheet: {e}"))?;
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let mut review=ImportReview{total_rows:0,new_rows:0,changed_rows:0,unchanged_rows:0,duplicate_rows:0,error_rows:0,examples:Vec::new()};
    let mut seen=std::collections::HashSet::new();
    let first_row=range.start().map(|(r,_)|r as usize+1).unwrap_or(1);
    let mut pr_check=db.prepare("SELECT quantity,item_name,spec,unit,due_date FROM pr_line WHERE pr_number=?1 AND item_code=?2 ORDER BY id DESC LIMIT 1").map_err(|e|e.to_string())?;
    let mut quote_check=db.prepare("SELECT q.unit_price FROM quote_line q JOIN quote_batch b ON b.id=q.batch_id WHERE b.supplier=?1 AND q.item_code=?2 ORDER BY b.id DESC LIMIT 1").map_err(|e|e.to_string())?;
    let mut history_check=db.prepare("SELECT unit_price FROM purchase_history WHERE supplier=?1 AND item_code=?2 ORDER BY id DESC LIMIT 1").map_err(|e|e.to_string())?;
    let total_rows=range.height();
    for (offset,row) in range.rows().enumerate(){
        if offset%512==0{if cancel.load(Ordering::Relaxed){return Err("Đã hủy rà soát dữ liệu.".into())}report_import_progress(&app,&job_id,offset,total_rows)}
        let line=offset+first_row;
        if line<first{continue}
        if row.iter().all(|cell|cell_value_text(cell).trim().is_empty()){continue}
        review.total_rows+=1;
        let code=cell_text(row,column("code"));
        let key=match kind.as_str(){"pr"=>format!("{}|{}|{}",cell_text(row,column("pr")),code,cell_text(row,column("quantity"))),"history"=>format!("{}|{}|{}|{}|{}",cell_text(row,column("supplier")),cell_text(row,column("pr")),cell_text(row,column("po")),code,cell_text(row,column("unitPrice"))),_=>format!("{}|{}|{}|{}|{}",supplier.as_deref().unwrap_or_default(),code,cell_text(row,column("unit")),cell_text(row,column("quantity")),cell_text(row,column("unitPrice")))};
        let price=money(cell_text(row,column("unitPrice")));
        let qty=cell_text(row,column("quantity"));
        let parsed_qty=qty.replace(',', "").parse::<f64>().ok();
        let missing=if code.is_empty(){Some("thiếu mã hàng")}else if kind=="pr"&&(cell_text(row,column("pr")).is_empty()||parsed_qty.is_none_or(|q|!q.is_finite()||q<=0.0)){Some("thiếu PR hoặc số lượng hợp lệ")}else if kind=="history"&&cell_text(row,column("supplier")).is_empty(){Some("thiếu nhà cung cấp")}else if kind!="pr"&&price.filter(|p|*p>0).is_none(){Some("thiếu đơn giá hợp lệ")}else{None};
        if let Some(reason)=missing{review.error_rows+=1;if review.examples.len()<12{review.examples.push(format!("Dòng {line}: {reason}"))}continue}
        if !seen.insert(key){review.duplicate_rows+=1;if review.examples.len()<12{review.examples.push(format!("Dòng {line}: có mã PR/mã hàng hoặc NCC/mã hàng lặp trong file"))}continue}
        let status=match kind.as_str(){
            "pr"=>{let pr=cell_text(row,column("pr"));let old=pr_check.query_row(params![pr,code],|r|Ok((r.get::<_,f64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?))).optional().map_err(|e|e.to_string())?;match old{None=>0,Some((old_qty,old_name,old_spec,old_unit,old_due))=>{let new_qty=qty.replace(',', "").parse::<f64>().unwrap_or_default();let changed=(old_qty-new_qty).abs()>0.000001||old_name!=cell_text(row,column("name"))||old_spec!=cell_text(row,column("spec"))||old_unit!=cell_text(row,column("unit"))||old_due!=date_cell_text(row,column("validUntil"));if changed{1}else{2}}}},
            "history"=>{let old=history_check.query_row(params![cell_text(row,column("supplier")),code],|r|r.get::<_,i64>(0)).optional().map_err(|e|e.to_string())?;match old{None=>0,Some(v)if Some(v)==price=>2,Some(_)=>1}},
            _=>{let old=quote_check.query_row(params![supplier.as_deref().unwrap_or_default(),code],|r|r.get::<_,i64>(0)).optional().map_err(|e|e.to_string())?;match old{None=>0,Some(v)if Some(v)==price=>2,Some(_)=>1}}
        };
        match status{0=>review.new_rows+=1,1=>review.changed_rows+=1,_=>review.unchanged_rows+=1}
    }
    report_import_progress(&app,&job_id,total_rows,total_rows);Ok(review)
}

#[tauri::command]
fn read_pdf_bytes(path: String) -> Result<Vec<u8>, String> {
    if !path.to_ascii_lowercase().ends_with(".pdf") { return Err("Chỉ có thể đọc tệp PDF ở thao tác này".into()); }
    fs::read(path).map_err(|e| format!("Không đọc được PDF: {e}"))
}

#[tauri::command]
fn import_pr_workbook(path: String, sheet: String, mapping: PrColumnMap, clear_quotes: bool, app: tauri::AppHandle, job_id: String, state: State<'_, AppState>) -> Result<ImportResult, String> {
    let (cancel,_job_guard)=begin_import_job(&state,job_id.clone())?;
    let file_hash=file_hash(&path)?;let hash=format!("{file_hash}:{sheet}:{}:{:?}",mapping.first_data_row,(mapping.project,mapping.pr,mapping.code,mapping.name,mapping.spec,mapping.unit,mapping.quantity,mapping.due_date));
    let mut workbook = open_workbook_auto(&path).map_err(|e| format!("Không mở được Excel: {e}"))?;
    let range = workbook.worksheet_range(&sheet).map_err(|e| format!("Không đọc được sheet: {e}"))?;
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    if !clear_quotes { if let Some((batch_id, row_count)) = db.query_row("SELECT id, row_count FROM import_batch WHERE is_active=1 AND file_hash = ?1", [&hash], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))).optional().map_err(|e| e.to_string())? {
        return Ok(ImportResult { batch_id, imported: 0, skipped: 0, unchanged: row_count.max(0) as usize });
    }}
    let tx = db.unchecked_transaction().map_err(|e| e.to_string())?;
    if clear_quotes {
        tx.execute("UPDATE allocation SET quote_line_id=NULL WHERE quote_line_id IS NOT NULL", []).map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM quote_line", []).map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM quote_batch", []).map_err(|e| e.to_string())?;
    }
    tx.execute("DELETE FROM pr_line WHERE NOT EXISTS (SELECT 1 FROM allocation WHERE allocation.pr_line_id=pr_line.id)", []).map_err(|e| e.to_string())?;
    tx.execute("UPDATE import_batch SET is_active=0,file_hash='',row_count=(SELECT COUNT(*) FROM pr_line WHERE pr_line.batch_id=import_batch.id)", []).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM import_batch WHERE NOT EXISTS (SELECT 1 FROM pr_line WHERE pr_line.batch_id=import_batch.id)", []).map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO import_batch(source_path, sheet_name, file_hash) VALUES (?1, ?2, ?3)", params![path, sheet, hash]).map_err(|e| e.to_string())?;
    let batch_id = tx.last_insert_rowid();
    let mut stmt = tx.prepare_cached(
        "INSERT INTO pr_line(batch_id, source_row, project, pr_number, item_code, item_name, spec, unit, quantity, due_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"
    ).map_err(|e| e.to_string())?;
    let mut imported = 0;
    let mut skipped = 0;
    let first_row=range.start().map(|(r,_)|r as usize+1).unwrap_or(1);
    let total_rows=range.height();
    for (offset, row) in range.rows().enumerate() {
        if offset % 512 == 0 { if cancel.load(Ordering::Relaxed) { return Err("Đã hủy nhập; dữ liệu cũ được giữ nguyên.".into()); } report_import_progress(&app,&job_id,offset,total_rows); }
        let zero_row=offset+first_row-1;
        if zero_row+1 < mapping.first_data_row {continue}
        let pr = cell_text(row, Some(mapping.pr));
        let code = cell_text(row, Some(mapping.code));
        let qty_text = cell_text(row, Some(mapping.quantity));
        let qty = qty_text.replace(',', "").parse::<f64>().ok();
        if pr.is_empty() || code.is_empty() || qty.is_none_or(|value|!value.is_finite()||value<=0.0) {
            skipped += 1;
            continue;
        }
        let mut project = cell_text(row, mapping.project);
        if project.is_empty() {
            project = project_from_pr(&pr);
        }
        stmt.execute(params![
            batch_id, (zero_row + 1) as i64, project, pr, code,
            cell_text(row, mapping.name), cell_text(row, mapping.spec), cell_text(row, mapping.unit),
            qty.unwrap_or_default(), date_cell_text(row, mapping.due_date)
        ]).map_err(|e| e.to_string())?;
        imported += 1;
    }
    drop(stmt);
    if imported == 0 { return Err("Không tìm thấy dòng PR hợp lệ. Dữ liệu cũ vẫn được giữ nguyên; hãy kiểm tra dòng bắt đầu và các cột đã ghép.".into()); }
    tx.execute("UPDATE import_batch SET row_count = ?1 WHERE id = ?2", params![imported as i64, batch_id]).map_err(|e| e.to_string())?;
    tx.execute("UPDATE app_state SET value='0' WHERE key='source_data_cleared'", []).map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO audit_event(event_type,entity_type,entity_id,details) VALUES ('import','pr',?1,?2)",params![batch_id,format!("rows={imported}; skipped={skipped}")]).map_err(|e|e.to_string())?;
    report_import_progress(&app,&job_id,total_rows,total_rows);
    tx.commit().map_err(|e| e.to_string())?;
    Ok(ImportResult { batch_id, imported, skipped, unchanged: 0 })
}

#[tauri::command]
fn import_history_workbook(path: String, sheet: String, mapping: HistoryColumnMap, app: tauri::AppHandle, job_id: String, state: State<'_, AppState>) -> Result<ImportSummary, String> {
    let (cancel,_job_guard)=begin_import_job(&state,job_id.clone())?;
    let file_hash=file_hash(&path)?;let hash=format!("{file_hash}:{sheet}:{}:{:?}",mapping.first_data_row,(mapping.project,mapping.pr,mapping.po,mapping.code,mapping.name,mapping.supplier,mapping.quantity,mapping.unit_price,mapping.amount));
    let mut workbook = open_workbook_auto(&path).map_err(|e| format!("Không mở được Excel: {e}"))?;
    let range = workbook.worksheet_range(&sheet).map_err(|e| format!("Không đọc được sheet: {e}"))?;
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    if let Some((id, rows)) = db.query_row("SELECT id,row_count FROM history_batch WHERE file_hash=?1", [&hash], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?))).optional().map_err(|e| e.to_string())? {
        return Ok(ImportSummary { batch_id: id, imported: 0, skipped: 0, unchanged: rows.max(0) as usize });
    }
    let tx = db.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO history_batch(source_path,file_hash) VALUES (?1,?2)", params![path,hash]).map_err(|e|e.to_string())?;
    let batch_id=tx.last_insert_rowid();
    let mut stmt=tx.prepare_cached("INSERT INTO purchase_history(batch_id,source_row,source_path,project,pr_number,po_number,item_code,item_name,supplier,quantity,unit_price,amount) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(source_path,source_row) DO UPDATE SET batch_id=excluded.batch_id,project=excluded.project,pr_number=excluded.pr_number,po_number=excluded.po_number,item_code=excluded.item_code,item_name=excluded.item_name,supplier=excluded.supplier,quantity=excluded.quantity,unit_price=excluded.unit_price,amount=excluded.amount WHERE purchase_history.project<>excluded.project OR purchase_history.pr_number<>excluded.pr_number OR purchase_history.po_number<>excluded.po_number OR purchase_history.item_code<>excluded.item_code OR purchase_history.item_name<>excluded.item_name OR purchase_history.supplier<>excluded.supplier OR purchase_history.quantity<>excluded.quantity OR purchase_history.unit_price<>excluded.unit_price OR purchase_history.amount<>excluded.amount").map_err(|e|e.to_string())?;
    let mut imported=0; let mut skipped=0; let mut unchanged=0;
    let first_row=range.start().map(|(r,_)|r as usize+1).unwrap_or(1);
    let total_rows=range.height();
    for (offset,row) in range.rows().enumerate() {
        if offset % 512 == 0 { if cancel.load(Ordering::Relaxed) { return Err("Đã hủy nhập; dữ liệu chưa được ghi.".into()); } report_import_progress(&app,&job_id,offset,total_rows); }
        let zero_row=offset+first_row-1;if zero_row+1<mapping.first_data_row{continue}
        let code=cell_text(row,Some(mapping.code)); let supplier=cell_text(row,Some(mapping.supplier));
        let price=money(cell_text(row,Some(mapping.unit_price)));
        if code.is_empty() || supplier.is_empty() || price.is_none() || price.unwrap_or_default()<=0 { skipped+=1; continue; }
        let qty=cell_text(row,mapping.quantity).replace(',',"").parse::<f64>().unwrap_or(0.0);
        let amount=money(cell_text(row,mapping.amount)).unwrap_or(0);
        let changed=stmt.execute(params![batch_id,(zero_row+1) as i64,path,cell_text(row,mapping.project),cell_text(row,mapping.pr),cell_text(row,mapping.po),code,cell_text(row,mapping.name),supplier,qty,price.unwrap_or_default(),amount]).map_err(|e|e.to_string())?;
        if changed>0 { imported+=1; } else { unchanged+=1; }
    }
    drop(stmt); if imported+unchanged==0 { return Err("Không tìm thấy dòng hợp lệ. Kiểm tra dòng bắt đầu và cột mã hàng, nhà cung cấp, đơn giá trước khi nhập.".into()); }
    tx.execute("UPDATE history_batch SET row_count=?1 WHERE id=?2",params![(imported+unchanged) as i64,batch_id]).map_err(|e|e.to_string())?;tx.execute("INSERT INTO audit_event(event_type,entity_type,entity_id,details) VALUES ('import','history',?1,?2)",params![batch_id,format!("rows={imported}; unchanged={unchanged}; skipped={skipped}")]).map_err(|e|e.to_string())?;report_import_progress(&app,&job_id,total_rows,total_rows); tx.commit().map_err(|e|e.to_string())?;
    Ok(ImportSummary{batch_id,imported,skipped,unchanged})
}

#[tauri::command]
fn import_quote_workbook(path: String, sheet: String, supplier: String, mapping: QuoteColumnMap, app: tauri::AppHandle, job_id: String, state: State<'_, AppState>) -> Result<ImportSummary,String> {
    let (cancel,_job_guard)=begin_import_job(&state,job_id.clone())?;
    let supplier=supplier.trim().to_string(); if supplier.is_empty(){return Err("Nhập tên nhà cung cấp trước khi nhập báo giá".into())}
    let file_hash=file_hash(&path)?;let hash=format!("{file_hash}:{sheet}:{}:{:?}",mapping.first_data_row,(mapping.code,mapping.name,mapping.spec,mapping.unit,mapping.quantity,mapping.unit_price,mapping.valid_until));let mut workbook=open_workbook_auto(&path).map_err(|e|format!("Không mở được Excel: {e}"))?;
    let range=workbook.worksheet_range(&sheet).map_err(|e|format!("Không đọc được sheet: {e}"))?;
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    if let Some((id,rows))=db.query_row("SELECT id,row_count FROM quote_batch WHERE file_hash=?1 AND supplier=?2",params![hash,supplier],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?))).optional().map_err(|e|e.to_string())? {return Ok(ImportSummary{batch_id:id,imported:0,skipped:0,unchanged:rows.max(0) as usize})}
    let tx=db.unchecked_transaction().map_err(|e|e.to_string())?;
    tx.execute("INSERT INTO quote_batch(source_path,file_hash,supplier,quote_number,sheet_name) VALUES (?1,?2,?3,?4,?5)",params![path,hash,supplier,mapping.quote_number.clone().unwrap_or_default(),sheet]).map_err(|e|e.to_string())?;
    let batch_id=tx.last_insert_rowid();
    let mut stmt=tx.prepare_cached("INSERT INTO quote_line(batch_id,source_row,item_code,item_name,spec,unit,quantity,unit_price,valid_until) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)").map_err(|e|e.to_string())?;
    let mut imported=0;let mut skipped=0;
    let first_row=range.start().map(|(r,_)|r as usize+1).unwrap_or(1);
    let total_rows=range.height();
    for (offset,row) in range.rows().enumerate(){
        if offset % 512 == 0 { if cancel.load(Ordering::Relaxed) { return Err("Đã hủy nhập; dữ liệu chưa được ghi.".into()); } report_import_progress(&app,&job_id,offset,total_rows); }
        let zero_row=offset+first_row-1;if zero_row+1<mapping.first_data_row{continue}
        let code=cell_text(row,Some(mapping.code));let price=money(cell_text(row,Some(mapping.unit_price)));
        if code.is_empty()||price.is_none()||price.unwrap_or_default()<=0{skipped+=1;continue}
        let qty=cell_text(row,mapping.quantity).replace(',',"").parse::<f64>().unwrap_or(0.0);
        stmt.execute(params![batch_id,(zero_row+1) as i64,code,cell_text(row,mapping.name),cell_text(row,mapping.spec),cell_text(row,mapping.unit),qty,price.unwrap_or_default(),date_cell_text(row,mapping.valid_until)]).map_err(|e|e.to_string())?;imported+=1;
    }
    drop(stmt);tx.execute("UPDATE quote_batch SET row_count=?1 WHERE id=?2",params![imported as i64,batch_id]).map_err(|e|e.to_string())?;tx.execute("INSERT INTO audit_event(event_type,entity_type,entity_id,details) VALUES ('import','quote',?1,?2)",params![batch_id,format!("supplier={supplier}; rows={imported}; skipped={skipped}")]).map_err(|e|e.to_string())?;report_import_progress(&app,&job_id,total_rows,total_rows);tx.commit().map_err(|e|e.to_string())?;
    Ok(ImportSummary{batch_id,imported,skipped,unchanged:0})
}

#[tauri::command]
fn import_quote_pdf(path: String, supplier: String, quote_number: Option<String>, rows: Vec<PdfQuoteRow>, app: tauri::AppHandle, job_id: String, state: State<'_, AppState>) -> Result<ImportSummary,String> {
    let (cancel,_job_guard)=begin_import_job(&state,job_id.clone())?;
    let supplier=supplier.trim().to_string(); if supplier.is_empty(){return Err("Nhập tên nhà cung cấp trước khi nhập báo giá".into())}
    if rows.is_empty(){return Err("PDF không có dòng hàng để nhập".into())}
    let file_hash=file_hash(&path)?;let hash=format!("{file_hash}:PDF:{}",quote_number.as_deref().unwrap_or_default());
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    if let Some((id,count))=db.query_row("SELECT id,row_count FROM quote_batch WHERE file_hash=?1 AND supplier=?2",params![hash,supplier],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?))).optional().map_err(|e|e.to_string())?{return Ok(ImportSummary{batch_id:id,imported:0,skipped:0,unchanged:count.max(0) as usize})}
    let tx=db.unchecked_transaction().map_err(|e|e.to_string())?;
    tx.execute("INSERT INTO quote_batch(source_path,file_hash,supplier,quote_number,sheet_name) VALUES (?1,?2,?3,?4,'PDF')",params![path,hash,supplier,quote_number.unwrap_or_default()]).map_err(|e|e.to_string())?;
    let batch_id=tx.last_insert_rowid();
    let mut stmt=tx.prepare_cached("INSERT INTO quote_line(batch_id,source_row,source_page,item_code,item_name,spec,unit,quantity,unit_price,valid_until) VALUES (?1,?2,?3,?4,'','',?5,?6,?7,'')").map_err(|e|e.to_string())?;
    let mut imported=0;let mut skipped=0;
    let total_rows=rows.len();
    for (index,row) in rows.iter().enumerate() {
        if index % 128 == 0 { if cancel.load(Ordering::Relaxed) { return Err("Đã hủy nhập; dữ liệu chưa được ghi.".into()); } report_import_progress(&app,&job_id,index,total_rows); }
        let code=row.item_code.trim();let price=money(row.unit_price.clone());let quantity=row.quantity.trim().replace(',',"").parse::<f64>().unwrap_or(0.0);
        if code.is_empty()||price.is_none()||price.unwrap_or_default()<=0||quantity<=0.0{skipped+=1;continue}
        stmt.execute(params![batch_id,row.line_number as i64,row.page as i64,code,row.unit,quantity,price.unwrap_or_default()]).map_err(|e|e.to_string())?;imported+=1;
    }
    drop(stmt);if imported==0{return Err("Không tìm thấy dòng báo giá có đơn giá hợp lệ. Hãy kiểm tra bảng xem trước.".into())}
    tx.execute("UPDATE quote_batch SET row_count=?1 WHERE id=?2",params![imported as i64,batch_id]).map_err(|e|e.to_string())?;tx.execute("INSERT INTO audit_event(event_type,entity_type,entity_id,details) VALUES ('import','quote_pdf',?1,?2)",params![batch_id,format!("supplier={supplier}; rows={imported}; skipped={skipped}")]).map_err(|e|e.to_string())?;report_import_progress(&app,&job_id,total_rows,total_rows);tx.commit().map_err(|e|e.to_string())?;
    Ok(ImportSummary{batch_id,imported,skipped,unchanged:0})
}

#[tauri::command]
fn get_pr_page(offset: i64, limit: i64, query: String, project: String, pr_numbers: Vec<String>, month: String, sort_by: String, sort_desc: bool, state: State<'_, AppState>) -> Result<PrPage, String> {
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    let batch: Option<i64> = db.query_row("SELECT MAX(id) FROM import_batch WHERE is_active=1", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let Some(batch) = batch else { return Ok(PrPage { rows: Vec::new(), total: 0, distinct_prs: 0, remaining_units: 0.0 }); };
    let (where_sql, mut base_params) = pr_filter_sql(&query,&project,&pr_numbers,&month);
    base_params.insert(0,Value::Integer(batch));
    let (total,distinct_prs,remaining_units):(i64,i64,f64)=db.query_row(&format!("SELECT COUNT(*),COUNT(DISTINCT pr_number),COALESCE(SUM(quantity-ordered),0) FROM pr_line WHERE {where_sql}"),params_from_iter(base_params.iter()),|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;
    let mut page_params = base_params;
    page_params.push(Value::Integer(limit.clamp(1, 500)));
    page_params.push(Value::Integer(offset.max(0)));
    let limit_index = page_params.len() - 1;
    let offset_index = page_params.len();
    let project_expr=stored_project_expr_sql();
    let sort_column=match sort_by.as_str(){
        "project"=>format!("({project_expr}) COLLATE NOCASE"),
        "pr"=>"pr_number COLLATE NOCASE".into(),
        "code"=>"item_code COLLATE NOCASE".into(),
        "name"=>"item_name COLLATE NOCASE".into(),
        "spec"=>"spec COLLATE NOCASE".into(),
        "unit"=>"unit COLLATE NOCASE".into(),
        "qty"=>"quantity".into(),
        "ordered"=>"ordered".into(),
        "remaining"=>"(quantity - ordered)".into(),
        "due"=>"CASE WHEN length(due_date)>=10 AND substr(due_date,3,1)='/' AND substr(due_date,6,1)='/' THEN substr(due_date,7,4)||substr(due_date,4,2)||substr(due_date,1,2) ELSE due_date END".into(),
        _=>"source_row".into(),
    };
    let direction=if sort_desc{"DESC"}else{"ASC"};
    let sql = format!("SELECT id, {project_expr}, pr_number, item_code, item_name, spec, unit, quantity, ordered, due_date FROM pr_line WHERE {where_sql} ORDER BY {sort_column} {direction}, source_row ASC, id ASC LIMIT ?{limit_index} OFFSET ?{offset_index}");
    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params_from_iter(page_params.iter()), |r| Ok(PrRow {
        id: r.get::<_, i64>(0)?.to_string(), project: r.get(1)?, pr: r.get(2)?, code: r.get(3)?,
        name: r.get(4)?, spec: r.get(5)?, unit: r.get(6)?, qty: r.get(7)?, ordered: r.get(8)?,
        due: display_date_text(r.get(9)?), status: "Chưa xử lý".into(),
    })).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(PrPage { rows, total, distinct_prs, remaining_units })
}

fn pr_filter_sql(query:&str,project:&str,pr_numbers:&[String],month:&str)->(String,Vec<Value>){
    let query=query.trim();let prefix=format!("{query}%");
    let project_filter=if project=="Tất cả dự án"{String::new()}else{project.to_string()};
    let project_expr=stored_project_expr_sql();
    let month_pattern=month.split_once('-').and_then(|(year,month)|{
        if year.len()==4&&month.len()==2&&month.parse::<u8>().is_ok_and(|m|(1..=12).contains(&m)){Some(format!("%-{}{}%",&year[2..],month))}else{None}
    }).unwrap_or_default();
    let selected=serde_json::to_string(pr_numbers).unwrap_or_else(|_|"[]".to_string());
    let where_sql=format!("batch_id=?1 AND (?2='' OR {project_expr}=?2) AND (?3='' OR pr_number LIKE ?4 OR item_code LIKE ?4 OR item_name LIKE ?4) AND (?5='' OR pr_number LIKE ?6) AND (?7='[]' OR pr_number IN (SELECT value FROM json_each(?7)))");
    (where_sql,vec![project_filter.into(),query.to_string().into(),prefix.into(),month.to_string().into(),month_pattern.into(),selected.into()])
}

#[tauri::command]
fn get_pr_ids(query:String,project:String,pr_numbers:Vec<String>,month:String,state:State<'_,AppState>)->Result<Vec<i64>,String>{
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let batch:Option<i64>=db.query_row("SELECT MAX(id) FROM import_batch WHERE is_active=1",[],|r|r.get(0)).map_err(|e|e.to_string())?;
    let Some(batch)=batch else{return Ok(Vec::new())};
    let (where_sql,mut values)=pr_filter_sql(&query,&project,&pr_numbers,&month);values.insert(0,Value::Integer(batch));
    let mut stmt=db.prepare(&format!("SELECT id FROM pr_line WHERE {where_sql} ORDER BY source_row")).map_err(|e|e.to_string())?;
    stmt.query_map(params_from_iter(values.iter()),|r|r.get(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}

#[tauri::command]
fn get_pr_months(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    let mut stmt = db.prepare("SELECT DISTINCT pr_number FROM pr_line WHERE batch_id=(SELECT MAX(id) FROM import_batch WHERE is_active=1)").map_err(|e| e.to_string())?;
    let mut months = std::collections::BTreeSet::new();
    for value in stmt.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())? {
        let pr = value.map_err(|e| e.to_string())?;
        let digits = pr.chars().rev().take(6).collect::<String>().chars().rev().collect::<String>();
        if digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_digit()) {
            let month = digits[2..4].parse::<u8>().unwrap_or(0);
            if (1..=12).contains(&month) { months.insert(format!("20{}-{:02}", &digits[..2], month)); }
        }
    }
    Ok(months.into_iter().rev().collect())
}

#[tauri::command]
fn get_pr_projects(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    let project_expr = stored_project_expr_sql();
    let mut stmt = db.prepare(&format!("SELECT DISTINCT {project_expr} FROM pr_line WHERE batch_id = (SELECT MAX(id) FROM import_batch WHERE is_active=1) ORDER BY 1")).map_err(|e| e.to_string())?;
    stmt.query_map([], |r| r.get(0)).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_suppliers(state: State<'_,AppState>) -> Result<Vec<String>,String> {
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let mut stmt=db.prepare("SELECT supplier FROM quote_batch GROUP BY supplier ORDER BY supplier").map_err(|e|e.to_string())?;
    stmt.query_map([],|r|r.get(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}

#[tauri::command]
fn set_supplier_priority(supplier:String,priority:i64,state:State<'_,AppState>)->Result<(),String>{
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    db.execute("INSERT INTO supplier_priority(supplier,priority) VALUES (?1,?2) ON CONFLICT(supplier) DO UPDATE SET priority=excluded.priority",params![supplier.trim(),priority.clamp(1,999)]).map_err(|e|e.to_string())?;Ok(())
}

#[tauri::command]
fn get_supplier_priorities(state: State<'_, AppState>) -> Result<Vec<SupplierPriority>, String> {
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    let mut stmt = db.prepare("SELECT s.supplier,COALESCE(p.priority,100) FROM (SELECT DISTINCT supplier FROM quote_batch) s LEFT JOIN supplier_priority p ON p.supplier=s.supplier ORDER BY s.supplier").map_err(|e| e.to_string())?;
    stmt.query_map([], |r| Ok(SupplierPriority { supplier: r.get(0)?, priority: r.get(1)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_comparison_rows(pr_ids:Vec<i64>,price_rule:String,state:State<'_,AppState>)->Result<Vec<ComparisonRow>,String>{
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let mut rules=std::collections::HashMap::<String,i64>::new();
    {let mut s=db.prepare("SELECT supplier,priority FROM supplier_priority").map_err(|e|e.to_string())?;for r in s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).map_err(|e|e.to_string())?{let (n,p)=r.map_err(|e|e.to_string())?;rules.insert(n,p);}}
    let mut rows=Vec::new();
    let mut base_stmt=db.prepare_cached("SELECT id,project,pr_number,item_code,item_name,spec,unit,quantity FROM pr_line WHERE id=?1").map_err(|e|e.to_string())?;
    let mut history_stmt=db.prepare_cached("SELECT unit_price FROM purchase_history WHERE item_code=?1 AND unit_price>0 ORDER BY id DESC LIMIT 1").map_err(|e|e.to_string())?;
    let mut offers_stmt=db.prepare_cached("SELECT q.id,b.supplier,q.unit_price FROM quote_line q JOIN quote_batch b ON b.id=q.batch_id WHERE q.item_code=?1 AND q.unit_price>0 AND b.id=(SELECT MAX(b2.id) FROM quote_batch b2 WHERE b2.supplier=b.supplier) ORDER BY b.supplier,q.source_row").map_err(|e|e.to_string())?;
    for id in pr_ids{
        let base=base_stmt.query_row([id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,f64>(7)?))).optional().map_err(|e|e.to_string())?;
        let Some((id,project,pr,code,name,spec,unit,quantity))=base else{continue};
        let history=history_stmt.query_row([&code],|r|r.get::<_,i64>(0)).optional().map_err(|e|e.to_string())?;
        let mut offers=Vec::new();let mut seen=std::collections::HashSet::new();
        for item in offers_stmt.query_map([&code],|r|Ok(Offer{quote_line_id:r.get(0)?,supplier:r.get(1)?,unit_price:r.get(2)?})).map_err(|e|e.to_string())?{let o=item.map_err(|e|e.to_string())?;if seen.insert(o.supplier.clone()){offers.push(o)}}
        offers.sort_by(|a,b|match price_rule.as_str(){"max"=>b.unit_price.cmp(&a.unit_price).then_with(||rules.get(&a.supplier).unwrap_or(&100).cmp(rules.get(&b.supplier).unwrap_or(&100))),"priority"=>rules.get(&a.supplier).unwrap_or(&100).cmp(rules.get(&b.supplier).unwrap_or(&100)).then_with(||a.unit_price.cmp(&b.unit_price)),_=>a.unit_price.cmp(&b.unit_price).then_with(||rules.get(&a.supplier).unwrap_or(&100).cmp(rules.get(&b.supplier).unwrap_or(&100)))});
        let suggested_supplier=offers.first().map(|o|o.supplier.clone());let suggested_price=offers.first().map(|o|o.unit_price);
        rows.push(ComparisonRow{id,project,pr,code,name,spec,unit,quantity,history_price:history,offers,suggested_supplier,suggested_price});
    }
    Ok(rows)
}

#[tauri::command]
fn save_allocations(name:String,allocations:Vec<AllocationInput>,no_quote_pr_ids:Vec<i64>,scope_pr_ids:Vec<i64>,state:State<'_,AppState>)->Result<i64,String>{
    if allocations.is_empty(){return Err("Chưa có dòng phân bổ".into())}
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let mut sums=std::collections::HashMap::<i64,f64>::new();
    for a in &allocations{
        if a.supplier.trim().is_empty()||a.quantity<=0.0||a.unit_price<=0{return Err("Nhà cung cấp, số lượng và giá phải hợp lệ".into())}
        *sums.entry(a.pr_line_id).or_default()+=a.quantity;
    }
    for (id,qty) in sums{
        let requested:f64=db.query_row("SELECT quantity-ordered FROM pr_line WHERE id=?1",[id],|r|r.get(0)).optional().map_err(|e|e.to_string())?.ok_or("Dòng PR không còn tồn tại")?;
        if qty-requested>0.000001{return Err(format!("Tổng phân bổ vượt số lượng còn lại ở dòng PR #{id}"))}
    }
    let tx=db.unchecked_transaction().map_err(|e|e.to_string())?;
    tx.execute("INSERT INTO work_batch(name) VALUES (?1)",[name.trim()]).map_err(|e|e.to_string())?;
    let batch_id=tx.last_insert_rowid();
    replace_no_quote_snapshot(&tx,batch_id,&no_quote_pr_ids)?;
    replace_scope_snapshot(&tx,batch_id,&scope_pr_ids)?;
    let mut stmt=tx.prepare_cached("INSERT INTO allocation(work_batch_id,pr_line_id,supplier,quantity,unit_price,quote_line_id,selection_reason) VALUES (?1,?2,?3,?4,?5,?6,?7)").map_err(|e|e.to_string())?;
    for a in allocations{stmt.execute(params![batch_id,a.pr_line_id,a.supplier.trim(),a.quantity,a.unit_price,a.quote_line_id,a.selection_reason]).map_err(|e|e.to_string())?;}
    drop(stmt);tx.execute("INSERT INTO audit_event(event_type,entity_type,entity_id,details) VALUES ('save','work_batch',?1,?2)",params![batch_id,format!("allocations={}",tx.query_row("SELECT COUNT(*) FROM allocation WHERE work_batch_id=?1",[batch_id],|r|r.get::<_,i64>(0)).unwrap_or(0))]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(batch_id)
}

#[tauri::command]
fn get_work_batches(state: State<'_, AppState>) -> Result<Vec<WorkBatchSummary>, String> {
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    let mut stmt = db.prepare("SELECT w.id,w.name,w.created_at,CASE WHEN EXISTS(SELECT 1 FROM work_batch_scope s WHERE s.work_batch_id=w.id) THEN (SELECT COUNT(*) FROM work_batch_scope s WHERE s.work_batch_id=w.id) ELSE (SELECT COUNT(DISTINCT pr_line_id) FROM allocation a WHERE a.work_batch_id=w.id)+(SELECT COUNT(DISTINCT pr_line_id) FROM work_batch_no_quote n WHERE n.work_batch_id=w.id AND NOT EXISTS(SELECT 1 FROM allocation a WHERE a.work_batch_id=w.id AND a.pr_line_id=n.pr_line_id)) END,(SELECT COUNT(DISTINCT pr_line_id) FROM allocation a WHERE a.work_batch_id=w.id),(SELECT COUNT(*) FROM work_batch_no_quote n WHERE n.work_batch_id=w.id),(SELECT COUNT(*) FROM po_header h WHERE h.work_batch_id=w.id),COALESCE((SELECT SUM(total) FROM po_header h WHERE h.work_batch_id=w.id),0) FROM work_batch w ORDER BY w.id DESC LIMIT 100").map_err(|e| e.to_string())?;
    stmt.query_map([], |r| Ok(WorkBatchSummary { id:r.get(0)?,name:r.get(1)?,created_at:r.get(2)?,lines:r.get::<_,i64>(3)?.max(0) as usize,allocated_lines:r.get::<_,i64>(4)?.max(0) as usize,no_quote_lines:r.get::<_,i64>(5)?.max(0) as usize,po_count:r.get::<_,i64>(6)?.max(0) as usize,total_amount:r.get(7)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_work_batch_allocations(work_batch_id: i64, state: State<'_, AppState>) -> Result<Vec<AllocationInput>, String> {
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    let mut stmt = db.prepare("SELECT pr_line_id,supplier,quantity,unit_price,quote_line_id,selection_reason FROM allocation WHERE work_batch_id=?1 ORDER BY id").map_err(|e| e.to_string())?;
    stmt.query_map([work_batch_id], |r| Ok(AllocationInput { pr_line_id:r.get(0)?, supplier:r.get(1)?, quantity:r.get(2)?, unit_price:r.get(3)?, quote_line_id:r.get(4)?, selection_reason:r.get(5)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_work_batch_no_quote_ids(work_batch_id:i64,state:State<'_,AppState>)->Result<Vec<i64>,String>{
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let mut stmt=db.prepare("SELECT pr_line_id FROM work_batch_no_quote WHERE work_batch_id=?1 ORDER BY pr_line_id").map_err(|e|e.to_string())?;
    stmt.query_map([work_batch_id],|r|r.get(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}

#[tauri::command]
fn get_work_batch_scope_ids(work_batch_id:i64,state:State<'_,AppState>)->Result<Vec<i64>,String>{
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let mut stmt=db.prepare("SELECT pr_line_id FROM work_batch_scope WHERE work_batch_id=?1 UNION SELECT pr_line_id FROM allocation WHERE work_batch_id=?1 UNION SELECT pr_line_id FROM work_batch_no_quote WHERE work_batch_id=?1 ORDER BY 1").map_err(|e|e.to_string())?;
    stmt.query_map([work_batch_id],|r|r.get(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}

#[tauri::command]
fn update_work_batch(work_batch_id:i64,name:String,allocations:Vec<AllocationInput>,no_quote_pr_ids:Vec<i64>,scope_pr_ids:Vec<i64>,state:State<'_,AppState>)->Result<(),String>{
    if allocations.is_empty(){return Err("Chưa có dòng phân bổ".into())}
    let mut db=state.0.lock().map_err(|_|"Database đang bận")?;
    let mut sums=std::collections::HashMap::<i64,f64>::new();
    for a in &allocations{if a.supplier.trim().is_empty()||a.quantity<=0.0||a.unit_price<=0{return Err("Nhà cung cấp, số lượng và giá phải hợp lệ".into())}*sums.entry(a.pr_line_id).or_default()+=a.quantity;}
    for(id,qty)in sums{let requested:f64=db.query_row("SELECT quantity-ordered FROM pr_line WHERE id=?1",[id],|r|r.get(0)).optional().map_err(|e|e.to_string())?.ok_or("Dòng PR không còn tồn tại")?;if qty-requested>0.000001{return Err(format!("Tổng phân bổ vượt số lượng còn lại ở dòng PR #{id}"))}}
    let tx=db.transaction().map_err(|e|e.to_string())?;
    let exists:i64=tx.query_row("SELECT COUNT(*) FROM work_batch WHERE id=?1",[work_batch_id],|r|r.get(0)).map_err(|e|e.to_string())?;if exists==0{return Err("Đợt xử lý không còn tồn tại".into())}
    tx.execute("UPDATE work_batch SET name=?1 WHERE id=?2",params![name.trim(),work_batch_id]).map_err(|e|e.to_string())?;
    tx.execute("DELETE FROM po_header WHERE work_batch_id=?1",[work_batch_id]).map_err(|e|e.to_string())?;
    tx.execute("DELETE FROM allocation WHERE work_batch_id=?1",[work_batch_id]).map_err(|e|e.to_string())?;
    replace_no_quote_snapshot(&tx,work_batch_id,&no_quote_pr_ids)?;
    replace_scope_snapshot(&tx,work_batch_id,&scope_pr_ids)?;
    let mut stmt=tx.prepare_cached("INSERT INTO allocation(work_batch_id,pr_line_id,supplier,quantity,unit_price,quote_line_id,selection_reason) VALUES (?1,?2,?3,?4,?5,?6,?7)").map_err(|e|e.to_string())?;
    for a in allocations{stmt.execute(params![work_batch_id,a.pr_line_id,a.supplier.trim(),a.quantity,a.unit_price,a.quote_line_id,a.selection_reason]).map_err(|e|e.to_string())?;}
    drop(stmt);tx.execute("INSERT INTO audit_event(event_type,entity_type,entity_id,details) VALUES ('update','work_batch',?1,?2)",params![work_batch_id,format!("{} allocations saved",tx.query_row("SELECT COUNT(*) FROM allocation WHERE work_batch_id=?1",[work_batch_id],|r|r.get::<_,i64>(0)).unwrap_or(0))]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())
}

#[tauri::command]
fn get_po_summaries(work_batch_id: i64, state: State<'_, AppState>) -> Result<Vec<PoSummary>, String> {
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    let mut stmt = db.prepare("SELECT h.id,h.supplier,h.po_number,COUNT(l.id),h.total FROM po_header h LEFT JOIN po_line l ON l.po_id=h.id WHERE h.work_batch_id=?1 GROUP BY h.id ORDER BY h.supplier").map_err(|e| e.to_string())?;
    stmt.query_map([work_batch_id], |r| Ok(PoSummary { id: r.get(0)?, supplier: r.get(1)?, po_number: r.get(2)?, lines: r.get::<_, i64>(3)?.max(0) as usize, total: r.get(4)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_po_preflight_review(work_batch_id:i64,offset:i64,limit:i64,state:State<'_,AppState>)->Result<PoReview,String>{
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let scoped_count:i64=db.query_row("SELECT COUNT(*) FROM work_batch_scope WHERE work_batch_id=?1",[work_batch_id],|r|r.get(0)).map_err(|e|e.to_string())?;
    let mut stmt=db.prepare(po_preflight_sql(scoped_count>0)).map_err(|e|e.to_string())?;
    let mut rows=Vec::new();let offset=offset.max(0) as usize;let limit=limit.clamp(1,100) as usize;let mut total_lines=0;let mut full_lines=0;let mut partial_lines=0;let mut unallocated_lines=0;let mut no_quote_lines=0;let mut issue_total=0;let mut total_amount=0;
    for item in stmt.query_map([work_batch_id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,i64>(7)?,r.get::<_,f64>(8)?,r.get::<_,f64>(9)?,r.get::<_,i64>(10)?,r.get::<_,String>(11)?,r.get::<_,bool>(12)?))).map_err(|e|e.to_string())? {
        let (pr_line_id,project,pr_number,item_code,item_name,spec,unit,source_row,requested,allocated,amount,suppliers,no_quote)=item.map_err(|e|e.to_string())?;
        let issue=po_review_issue(no_quote,requested,allocated).to_string();
        if issue.is_empty(){full_lines+=1}else{let issue_index=issue_total;issue_total+=1;if issue_index>=offset&&rows.len()<limit{rows.push(PoReviewLine{pr_line_id,project,pr_number,item_code,item_name,spec,unit,source_row,requested,allocated,amount,suppliers,issue:issue.clone()})}}
        total_lines+=1;total_amount+=amount;
        match issue.as_str(){"Chưa có báo giá"=>no_quote_lines+=1,"Chưa phân bổ"=>unallocated_lines+=1,"Phân bổ chưa đủ"=>partial_lines+=1,_=>{}}
    }
    Ok(PoReview{total_lines,full_lines,partial_lines,unallocated_lines,no_quote_lines,issue_total,total_amount,rows})
}

fn po_review_issue(no_quote:bool,requested:f64,allocated:f64)->&'static str{if no_quote{"Chưa có báo giá"}else if allocated<=0.0{"Chưa phân bổ"}else if allocated+0.000001<requested{"Phân bổ chưa đủ"}else{""}}

fn report_date_key(value:&str)->Option<i32>{
    let value=value.trim();
    let (year,month,day)=if value.len()>=10&&value.as_bytes().get(4)==Some(&b'-'){
        (value.get(0..4)?.parse::<i32>().ok()?,value.get(5..7)?.parse::<i32>().ok()?,value.get(8..10)?.parse::<i32>().ok()?)
    }else if value.len()>=10&&value.as_bytes().get(2)==Some(&b'/'){
        (value.get(6..10)?.parse::<i32>().ok()?,value.get(3..5)?.parse::<i32>().ok()?,value.get(0..2)?.parse::<i32>().ok()?)
    }else{return None};
    if !(1..=12).contains(&month)||!(1..=31).contains(&day){return None}Some(year*10_000+month*100+day)
}

fn report_price_delta(price:Option<i64>,reference:Option<i64>)->Option<f64>{price.zip(reference).filter(|(_,base)|*base>0).map(|(value,base)|((value as f64-base as f64).abs()/base as f64)*100.0)}
fn report_signed_price_delta(price:Option<i64>,reference:Option<i64>)->Option<f64>{price.zip(reference).filter(|(_,base)|*base>0).map(|(value,base)|(value as f64-base as f64)/base as f64*100.0)}

#[tauri::command]
fn get_work_report(work_batch_id:Option<i64>,filter:String,threshold_percent:f64,stale_days:i64,offset:i64,limit:i64,state:State<'_,AppState>)->Result<WorkReport,String>{
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    build_work_report(&db,work_batch_id,filter,threshold_percent,stale_days,offset,limit)
}

fn build_work_report(db:&Connection,work_batch_id:Option<i64>,filter:String,threshold_percent:f64,stale_days:i64,offset:i64,limit:i64)->Result<WorkReport,String>{
    let (batch_id,batch_name)=match work_batch_id{
        Some(id)=>db.query_row("SELECT id,name FROM work_batch WHERE id=?1",[id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).optional().map_err(|e|e.to_string())?.ok_or("Không tìm thấy đợt làm việc")?,
        None=>db.query_row("SELECT id,name FROM work_batch ORDER BY id DESC LIMIT 1",[],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).optional().map_err(|e|e.to_string())?.ok_or("Chưa có đợt làm việc đã lưu")?,
    };
    let threshold=if threshold_percent.is_finite(){threshold_percent.clamp(1.0,1000.0)}else{30.0};let stale_days=stale_days.clamp(1,3650);
    let today_key=db.query_row("SELECT CAST(strftime('%Y%m%d','now') AS INTEGER)",[],|r|r.get::<_,i32>(0)).map_err(|e|e.to_string())?;
    let has_scope:i64=db.query_row("SELECT COUNT(*) FROM work_batch_scope WHERE work_batch_id=?1",[batch_id],|r|r.get(0)).map_err(|e|e.to_string())?;
    let mut scope=Vec::<(i64,String,String,String,String,f64,f64)>::new();
    if has_scope>0{
        let mut stmt=db.prepare("SELECT pr_line_id,project,pr_number,item_code,item_name,quantity,ordered FROM work_batch_scope WHERE work_batch_id=?1 ORDER BY pr_number,item_code,source_row").map_err(|e|e.to_string())?;
        for row in stmt.query_map([batch_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).map_err(|e|e.to_string())?{scope.push(row.map_err(|e|e.to_string())?)}
    }else{
        let mut stmt=db.prepare("SELECT p.id,COALESCE(p.project,''),p.pr_number,p.item_code,p.item_name,p.quantity,p.ordered FROM (SELECT pr_line_id FROM allocation WHERE work_batch_id=?1 UNION SELECT pr_line_id FROM work_batch_no_quote WHERE work_batch_id=?1) s JOIN pr_line p ON p.id=s.pr_line_id ORDER BY p.pr_number,p.item_code,p.source_row").map_err(|e|e.to_string())?;
        for row in stmt.query_map([batch_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).map_err(|e|e.to_string())?{scope.push(row.map_err(|e|e.to_string())?)}
    }
    let mut allocations=std::collections::HashMap::<i64,ReportAllocation>::new();
    {let mut stmt=db.prepare("SELECT a.pr_line_id,SUM(a.quantity),SUM(CAST(ROUND(a.quantity*a.unit_price) AS INTEGER)),GROUP_CONCAT(DISTINCT a.supplier),GROUP_CONCAT(DISTINCT a.selection_reason),GROUP_CONCAT(DISTINCT COALESCE(qb.quote_number,'')),GROUP_CONCAT(DISTINCT COALESCE(qb.source_path,'')),GROUP_CONCAT(DISTINCT COALESCE(qb.sheet_name,'')),GROUP_CONCAT(DISTINCT CASE WHEN q.source_page>0 THEN 'Trang '||q.source_page||' · STT '||q.source_row ELSE 'Dòng '||q.source_row END),MAX(CASE WHEN a.quote_line_id IS NULL THEN 1 ELSE 0 END) FROM allocation a LEFT JOIN quote_line q ON q.id=a.quote_line_id LEFT JOIN quote_batch qb ON qb.id=q.batch_id WHERE a.work_batch_id=?1 GROUP BY a.pr_line_id").map_err(|e|e.to_string())?;
        for row in stmt.query_map([batch_id],|r|Ok((r.get::<_,i64>(0)?,ReportAllocation{quantity:r.get(1)?,amount:r.get(2)?,suppliers:r.get::<_,Option<String>>(3)?.unwrap_or_default(),reason:r.get::<_,Option<String>>(4)?.unwrap_or_default(),quote_numbers:r.get::<_,Option<String>>(5)?.unwrap_or_default(),quote_sources:r.get::<_,Option<String>>(6)?.unwrap_or_default(),quote_sheets:r.get::<_,Option<String>>(7)?.unwrap_or_default(),quote_rows:r.get::<_,Option<String>>(8)?.unwrap_or_default(),manual_price:r.get::<_,i64>(9)?>0}))).map_err(|e|e.to_string())?{let (id,details)=row.map_err(|e|e.to_string())?;allocations.insert(id,details);}}
    let mut offers=std::collections::HashMap::<String,Vec<ReportQuote>>::new();
    {let mut stmt=db.prepare("WITH ranked AS (SELECT q.item_code,b.supplier,q.unit_price,q.valid_until,MAX(0,CAST(julianday('now')-julianday(b.imported_at) AS INTEGER)) age_days,ROW_NUMBER() OVER(PARTITION BY b.supplier,q.item_code ORDER BY b.id DESC,q.source_row,q.id) rn FROM quote_line q JOIN quote_batch b ON b.id=q.batch_id WHERE q.unit_price>0 AND b.id=(SELECT MAX(b2.id) FROM quote_batch b2 WHERE b2.supplier=b.supplier)) SELECT item_code,supplier,unit_price,valid_until,age_days FROM ranked WHERE rn=1").map_err(|e|e.to_string())?;
        for row in stmt.query_map([],|r|Ok((r.get::<_,String>(0)?,ReportQuote{supplier:r.get(1)?,price:r.get(2)?,valid_until:r.get(3)?,age_days:r.get::<_,Option<i64>>(4)?.unwrap_or(0)}))).map_err(|e|e.to_string())?{let (code,quote)=row.map_err(|e|e.to_string())?;offers.entry(code).or_default().push(quote)}}
    let mut history=std::collections::HashMap::<String,i64>::new();
    {let mut stmt=db.prepare("SELECT item_code,unit_price FROM (SELECT item_code,unit_price,ROW_NUMBER() OVER(PARTITION BY item_code ORDER BY id DESC) rn FROM purchase_history WHERE unit_price>0) WHERE rn=1").map_err(|e|e.to_string())?;
        for row in stmt.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).map_err(|e|e.to_string())?{let (code,price)=row.map_err(|e|e.to_string())?;history.insert(code,price);}}
    let filter=filter.as_str();let offset=offset.max(0) as usize;let limit=limit.clamp(1,200) as usize;let mut matched=0usize;let mut rows=Vec::new();let mut summary=WorkReportSummary{total_lines:0,allocated_lines:0,full_lines:0,partial_lines:0,unallocated_lines:0,no_quote_lines:0,over_allocated_lines:0,expired_quote_lines:0,stale_quote_lines:0,manual_price_lines:0,anomaly_lines:0,total_amount:0};
    for (id,project,pr_number,item_code,item_name,quantity,ordered) in scope{
        let requested=(quantity-ordered).max(0.0);let allocation=allocations.get(&id).cloned().unwrap_or_default();let item_offers=offers.get(&item_code).cloned().unwrap_or_default();let expired_quotes=item_offers.iter().filter(|offer|report_date_key(&offer.valid_until).is_some_and(|key|key<today_key)).count();let stale_quotes=item_offers.iter().filter(|offer|offer.age_days>stale_days).count();
        let history_price=history.get(&item_code).copied();let mut valid_offers=item_offers.iter().filter(|offer|report_date_key(&offer.valid_until).is_none_or(|key|key>=today_key)).collect::<Vec<_>>();if valid_offers.is_empty(){valid_offers=item_offers.iter().collect()};valid_offers.sort_by_key(|offer|(offer.price,offer.supplier.clone()));let best=valid_offers.first();let best_price=best.map(|offer|offer.price);let selected_price=(allocation.quantity>0.0).then(||(allocation.amount as f64/allocation.quantity).round() as i64);let selection_delta=report_signed_price_delta(selected_price,best_price);let history_delta=report_price_delta(selected_price,history_price);let low=item_offers.iter().map(|offer|offer.price).min();let high=item_offers.iter().map(|offer|offer.price).max();let spread=low.zip(high).filter(|(low,_)|*low>0).map(|(low,high)|((high-low) as f64/low as f64)*100.0);let anomaly=history_delta.is_some_and(|delta|delta>=threshold)||spread.is_some_and(|delta|delta>=threshold);
        let over=allocation.quantity-requested>0.000001;let full=allocation.quantity>0.0&&!over&&allocation.quantity+0.000001>=requested;let partial=allocation.quantity>0.0&&!over&&!full;let unallocated=allocation.quantity<=0.0;let no_quote=item_offers.is_empty();summary.total_lines+=1;summary.total_amount+=allocation.amount;if allocation.quantity>0.0{summary.allocated_lines+=1}if full{summary.full_lines+=1}if partial{summary.partial_lines+=1}if unallocated{summary.unallocated_lines+=1}if no_quote{summary.no_quote_lines+=1}if over{summary.over_allocated_lines+=1}if expired_quotes>0{summary.expired_quote_lines+=1}if stale_quotes>0{summary.stale_quote_lines+=1}if allocation.manual_price{summary.manual_price_lines+=1}if anomaly{summary.anomaly_lines+=1}
        let mut statuses=Vec::new();if no_quote{statuses.push("Thiếu báo giá")}if unallocated{statuses.push("Chưa phân bổ")}else if partial{statuses.push("Phân bổ chưa đủ")}if over{statuses.push("Vượt số lượng")}if expired_quotes>0{statuses.push("Có báo giá quá hạn")}if stale_quotes>0{statuses.push("Báo giá cũ")}if allocation.manual_price{statuses.push("Giá nhập tay")}if anomaly{statuses.push("Giá bất thường")}
        let status=if statuses.is_empty(){"Đủ điều kiện".to_string()}else{statuses.join(" · ")};let matches=match filter{"issues"=>!statuses.is_empty(),"no-quote"=>no_quote,"allocation"=>unallocated||partial||over,"expired"=>expired_quotes>0,"stale"=>stale_quotes>0,"manual-price"=>allocation.manual_price,"anomaly"=>anomaly,"decision"=>allocation.quantity>0.0,_=>true};if !matches{continue}matched+=1;if matched<=offset||rows.len()>=limit{continue}
        rows.push(WorkReportLine{pr_line_id:id,project,pr_number,item_code,item_name,requested,allocated:allocation.quantity,amount:allocation.amount,suppliers:allocation.suppliers,status,best_supplier:best.map(|offer|offer.supplier.clone()).unwrap_or_default(),best_price,selected_price,history_price,selection_delta_percent:selection_delta,history_delta_percent:history_delta,quote_spread_percent:spread,selection_reason:if allocation.reason.is_empty(){"Chưa lưu lý do".to_string()}else{allocation.reason},quote_numbers:allocation.quote_numbers,quote_sources:allocation.quote_sources,quote_sheets:allocation.quote_sheets,quote_rows:allocation.quote_rows,expired_quotes,stale_quotes,manual_price:allocation.manual_price,anomaly});
    }
    let supplier_totals={let mut stmt=db.prepare("SELECT supplier,COUNT(DISTINCT pr_line_id),SUM(CAST(ROUND(quantity*unit_price) AS INTEGER)) FROM allocation WHERE work_batch_id=?1 GROUP BY supplier ORDER BY 3 DESC").map_err(|e|e.to_string())?;
        let totals=stmt.query_map([batch_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;let sum=totals.iter().map(|(_,_,amount)|*amount).sum::<i64>();totals.into_iter().map(|(supplier,lines,amount)|WorkReportSupplier{supplier,lines:lines.max(0) as usize,amount,share_percent:if sum>0{amount as f64*100.0/sum as f64}else{0.0}}).collect()};
    Ok(WorkReport{work_batch_id:batch_id,work_batch_name:batch_name,summary,total_rows:matched,rows,supplier_totals})
}

fn po_preflight_sql(scoped:bool)->&'static str{if scoped{"SELECT s.pr_line_id,s.project,s.pr_number,s.item_code,s.item_name,s.spec,s.unit,s.source_row,MAX(s.quantity-s.ordered,0),COALESCE(SUM(a.quantity),0),COALESCE(SUM(CAST(ROUND(a.quantity*a.unit_price) AS INTEGER)),0),COALESCE(GROUP_CONCAT(DISTINCT a.supplier),''),EXISTS(SELECT 1 FROM work_batch_no_quote n WHERE n.work_batch_id=?1 AND n.pr_line_id=s.pr_line_id) FROM work_batch_scope s LEFT JOIN allocation a ON a.work_batch_id=s.work_batch_id AND a.pr_line_id=s.pr_line_id WHERE s.work_batch_id=?1 GROUP BY s.pr_line_id ORDER BY s.pr_number,s.item_code"}else{"WITH scope AS (SELECT pr_line_id FROM allocation WHERE work_batch_id=?1 UNION SELECT pr_line_id FROM work_batch_no_quote WHERE work_batch_id=?1) SELECT p.id,COALESCE(NULLIF(p.project,''),''),p.pr_number,p.item_code,p.item_name,p.spec,p.unit,p.source_row,MAX(p.quantity-p.ordered,0),COALESCE(SUM(a.quantity),0),COALESCE(SUM(CAST(ROUND(a.quantity*a.unit_price) AS INTEGER)),0),COALESCE(GROUP_CONCAT(DISTINCT a.supplier),''),EXISTS(SELECT 1 FROM work_batch_no_quote n WHERE n.work_batch_id=?1 AND n.pr_line_id=p.id) FROM scope s JOIN pr_line p ON p.id=s.pr_line_id LEFT JOIN allocation a ON a.work_batch_id=?1 AND a.pr_line_id=p.id GROUP BY p.id ORDER BY p.pr_number,p.item_code"}}

#[tauri::command]
fn rename_work_batch(work_batch_id: i64, name: String, state: State<'_, AppState>) -> Result<(), String> {
    let clean = name.trim();
    if clean.is_empty() { return Err("Tên đợt không được để trống".into()); }
    let db = state.0.lock().map_err(|_| "Database đang bận")?;
    db.execute("UPDATE work_batch SET name=?1 WHERE id=?2", params![clean, work_batch_id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn generate_purchase_orders(work_batch_id:i64,state:State<'_,AppState>)->Result<Vec<PoSummary>,String>{
    let mut db=state.0.lock().map_err(|_|"Database đang bận")?;
    let tx=db.transaction().map_err(|e|e.to_string())?;
    let groups={let mut s=tx.prepare("SELECT supplier,COUNT(*),CAST(SUM(ROUND(quantity*unit_price)) AS INTEGER) FROM allocation WHERE work_batch_id=?1 GROUP BY supplier ORDER BY supplier").map_err(|e|e.to_string())?;s.query_map([work_batch_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?};
    if groups.is_empty(){return Err("Đợt xử lý chưa có phân bổ".into())}
    let mut result=Vec::new();
    for (supplier,lines,total) in groups{
        let po_number=format!("PO-{work_batch_id}-{}",supplier.chars().filter(|c|c.is_ascii_alphanumeric()).take(12).collect::<String>().to_uppercase());
        tx.execute("INSERT INTO po_header(work_batch_id,supplier,po_number,total) VALUES (?1,?2,?3,?4) ON CONFLICT(work_batch_id,supplier) DO UPDATE SET total=excluded.total",params![work_batch_id,supplier,po_number,total]).map_err(|e|e.to_string())?;
        let po_id:i64=tx.query_row("SELECT id FROM po_header WHERE work_batch_id=?1 AND supplier=?2",params![work_batch_id,supplier],|r|r.get(0)).map_err(|e|e.to_string())?;
        tx.execute("DELETE FROM po_line WHERE po_id=?1",[po_id]).map_err(|e|e.to_string())?;
        let mut s=tx.prepare("SELECT p.pr_number,p.item_code,p.item_name,p.spec,p.unit,a.quantity,a.unit_price FROM allocation a JOIN pr_line p ON p.id=a.pr_line_id WHERE a.work_batch_id=?1 AND a.supplier=?2 ORDER BY p.pr_number,p.item_code").map_err(|e|e.to_string())?;
        let items=s.query_map(params![work_batch_id,supplier],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,f64>(5)?,r.get::<_,i64>(6)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        drop(s);let mut line_stmt=tx.prepare_cached("INSERT INTO po_line(po_id,pr_number,item_code,item_name,spec,unit,quantity,unit_price,amount) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)").map_err(|e|e.to_string())?;
        for (pr,code,name,spec,unit,qty,price) in items{let amount=(qty*price as f64).round() as i64;line_stmt.execute(params![po_id,pr,code,name,spec,unit,qty,price,amount]).map_err(|e|e.to_string())?;}
        drop(line_stmt);result.push(PoSummary{id:po_id,supplier,po_number,lines:lines.max(0) as usize,total});
    }
    tx.commit().map_err(|e|e.to_string())?;Ok(result)
}

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
struct ExportLine{supplier:String,po_number:String,pr:String,code:String,name:String,spec:String,unit:String,quantity:f64,unit_price:i64,amount:i64,history_price:Option<i64>}

struct ProvenanceLine{supplier:String,po_number:String,pr:String,code:String,quantity:f64,unit_price:i64,pr_source:String,pr_row:i64,quote_source:String,quote_row:Option<i64>}

struct NoQuoteReportLine{project:String,pr:String,code:String,name:String,spec:String,unit:String,quantity:f64,source_path:String,source_row:i64}

fn replace_no_quote_snapshot(db:&Connection,work_batch_id:i64,pr_line_ids:&[i64])->Result<(),String>{
    db.execute("DELETE FROM work_batch_no_quote WHERE work_batch_id=?1",[work_batch_id]).map_err(|e|e.to_string())?;
    let mut stmt=db.prepare_cached("INSERT INTO work_batch_no_quote(work_batch_id,pr_line_id,project,pr_number,item_code,item_name,spec,unit,quantity,source_path,source_row) SELECT ?1,p.id,p.project,p.pr_number,p.item_code,p.item_name,p.spec,p.unit,p.quantity,COALESCE(b.source_path,''),p.source_row FROM pr_line p LEFT JOIN import_batch b ON b.id=p.batch_id WHERE p.id=?2").map_err(|e|e.to_string())?;
    for id in pr_line_ids{if stmt.execute(params![work_batch_id,id]).map_err(|e|e.to_string())?==0{return Err(format!("Dòng PR #{id} không còn tồn tại để lưu báo cáo thiếu báo giá"))}}
    Ok(())
}

fn replace_scope_snapshot(db:&Connection,work_batch_id:i64,pr_line_ids:&[i64])->Result<(),String>{
    db.execute("DELETE FROM work_batch_scope WHERE work_batch_id=?1",[work_batch_id]).map_err(|e|e.to_string())?;
    let mut stmt=db.prepare_cached("INSERT INTO work_batch_scope(work_batch_id,pr_line_id,project,pr_number,item_code,item_name,spec,unit,quantity,ordered,source_row) SELECT ?1,id,project,pr_number,item_code,item_name,spec,unit,quantity,ordered,source_row FROM pr_line WHERE id=?2").map_err(|e|e.to_string())?;
    for id in pr_line_ids{if stmt.execute(params![work_batch_id,id]).map_err(|e|e.to_string())?==0{return Err(format!("Dòng PR #{id} không còn tồn tại để lưu phạm vi đợt xử lý"))}}
    Ok(())
}

#[tauri::command]
fn export_po_workbook(path:String,work_batch_id:i64,state:State<'_,AppState>)->Result<usize,String>{
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let mut stmt=db.prepare("SELECT h.supplier,h.po_number,l.pr_number,l.item_code,l.item_name,l.spec,l.unit,l.quantity,l.unit_price,l.amount,(SELECT ph.unit_price FROM purchase_history ph WHERE ph.item_code=l.item_code ORDER BY ph.id DESC LIMIT 1) FROM po_line l JOIN po_header h ON h.id=l.po_id WHERE h.work_batch_id=?1 ORDER BY h.supplier,l.pr_number,l.item_code").map_err(|e|e.to_string())?;
    let rows=stmt.query_map([work_batch_id],|r|Ok(ExportLine{supplier:r.get(0)?,po_number:r.get(1)?,pr:r.get(2)?,code:r.get(3)?,name:r.get(4)?,spec:r.get(5)?,unit:r.get(6)?,quantity:r.get(7)?,unit_price:r.get(8)?,amount:r.get(9)?,history_price:r.get(10)?})).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    drop(stmt);if rows.is_empty(){return Err("Chưa tạo PO cho đợt này".into())}
    let mut source_stmt=db.prepare("SELECT a.supplier,h.po_number,p.pr_number,p.item_code,a.quantity,a.unit_price,b.source_path,p.source_row,COALESCE(qb.source_path,''),q.source_row FROM allocation a JOIN pr_line p ON p.id=a.pr_line_id JOIN import_batch b ON b.id=p.batch_id JOIN po_header h ON h.work_batch_id=a.work_batch_id AND h.supplier=a.supplier LEFT JOIN quote_line q ON q.id=a.quote_line_id LEFT JOIN quote_batch qb ON qb.id=q.batch_id WHERE a.work_batch_id=?1 ORDER BY a.supplier,p.pr_number,p.item_code,a.id").map_err(|e|e.to_string())?;
    let provenance=source_stmt.query_map([work_batch_id],|r|Ok(ProvenanceLine{supplier:r.get(0)?,po_number:r.get(1)?,pr:r.get(2)?,code:r.get(3)?,quantity:r.get(4)?,unit_price:r.get(5)?,pr_source:r.get(6)?,pr_row:r.get(7)?,quote_source:r.get(8)?,quote_row:r.get(9)?})).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;drop(source_stmt);
    let mut no_quote_stmt=db.prepare("SELECT project,pr_number,item_code,item_name,spec,unit,quantity,source_path,source_row FROM work_batch_no_quote WHERE work_batch_id=?1 ORDER BY project,pr_number,item_code").map_err(|e|e.to_string())?;
    let no_quote_rows=no_quote_stmt.query_map([work_batch_id],|r|Ok(NoQuoteReportLine{project:r.get(0)?,pr:r.get(1)?,code:r.get(2)?,name:r.get(3)?,spec:r.get(4)?,unit:r.get(5)?,quantity:r.get(6)?,source_path:r.get(7)?,source_row:r.get(8)?})).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;drop(no_quote_stmt);
    let no_quote_item_count=no_quote_rows.iter().map(|row|row.code.as_str()).collect::<std::collections::HashSet<_>>().len();
    let title=Format::new().set_bold().set_font_size(16).set_font_color(Color::RGB(0x173B44));
    let subtitle=Format::new().set_font_color(Color::RGB(0x53666A));
    let header=Format::new().set_bold().set_font_color(Color::White).set_background_color(Color::RGB(0x155761)).set_align(FormatAlign::Center).set_text_wrap().set_border(FormatBorder::Thin).set_border_color(Color::RGB(0xD5E1E2));
    let body=Format::new().set_border(FormatBorder::Thin).set_border_color(Color::RGB(0xE3EAEB));
    let alternate=Format::new().set_background_color(Color::RGB(0xF2F7F7)).set_border(FormatBorder::Thin).set_border_color(Color::RGB(0xE3EAEB));
    let money=Format::new().set_num_format("#,##0;[Red]-#,##0").set_align(FormatAlign::Right);
    let qty=Format::new().set_num_format("#,##0.###").set_align(FormatAlign::Right);
    let total_format=Format::new().set_bold().set_background_color(Color::RGB(0xDDEEEF)).set_num_format("#,##0;[Red]-#,##0");
    let mut workbook=Workbook::new();let mut summary_rows=Vec::new();
    let mut vendors=std::collections::BTreeMap::<String,Vec<&ExportLine>>::new();for row in &rows{vendors.entry(row.supplier.clone()).or_default().push(row)}
    for (supplier,items) in &vendors{let po_number=&items[0].po_number;let total:i64=items.iter().map(|r|r.amount).sum();summary_rows.push((supplier,po_number,total,items.len()));}

    let summary=workbook.add_worksheet();summary.set_name("Tổng hợp").map_err(|e|e.to_string())?;summary.set_tab_color(Color::RGB(0x155761));
    summary.merge_range(0,0,0,3,"TỔNG HỢP ĐƠN ĐẶT HÀNG",&title).map_err(|e|e.to_string())?;
    summary.merge_range(1,0,1,3,"Danh sách PO nháp theo nhà cung cấp · Đối chiếu chi tiết tại từng sheet",&subtitle).map_err(|e|e.to_string())?;
    summary.write_string_with_format(2,0,format!("Mã hàng chưa có báo giá: {no_quote_item_count} · {n} dòng PR",n=no_quote_rows.len()),&subtitle).map_err(|e|e.to_string())?;
    for(c,h)in ["Nhà cung cấp","Mã PO","Số dòng hàng","Tổng tiền (VND)"].iter().enumerate(){summary.write_string_with_format(3,c as u16,*h,&header).map_err(|e|e.to_string())?;}
    for(i,(supplier,po,total,count))in summary_rows.iter().enumerate(){let r=i as u32+4;let row_format=if i%2==0{&body}else{&alternate};summary.write_string_with_format(r,0,(*supplier).as_str(),row_format).map_err(|e|e.to_string())?;summary.write_string_with_format(r,1,(*po).as_str(),row_format).map_err(|e|e.to_string())?;summary.write_number_with_format(r,2,*count as f64,row_format).map_err(|e|e.to_string())?;summary.write_number_with_format(r,3,*total as f64,&money).map_err(|e|e.to_string())?;}
    let summary_total: i64=summary_rows.iter().map(|(_,_,total,_)|*total).sum();let summary_total_row=summary_rows.len() as u32+4;
    summary.write_string_with_format(summary_total_row,2,"TỔNG CỘNG",&total_format).map_err(|e|e.to_string())?;summary.write_number_with_format(summary_total_row,3,summary_total as f64,&total_format).map_err(|e|e.to_string())?;
    for(col,width)in[(0,28.0),(1,27.0),(2,16.0),(3,24.0)]{summary.set_column_width(col,width).map_err(|e|e.to_string())?;}summary.set_row_height(0,30.0).map_err(|e|e.to_string())?;summary.set_row_height(3,32.0).map_err(|e|e.to_string())?;summary.set_freeze_panes(4,0).map_err(|e|e.to_string())?;summary.autofilter(3,0,summary_total_row.saturating_sub(1),3).map_err(|e|e.to_string())?;

    let quotes=workbook.add_worksheet();quotes.set_name("Tổng hợp báo giá").map_err(|e|e.to_string())?;quotes.set_tab_color(Color::RGB(0x528A78));
    quotes.merge_range(0,0,0,10,"TỔNG HỢP CHI TIẾT PO TOÀN NHÀ CUNG CẤP",&title).map_err(|e|e.to_string())?;
    quotes.merge_range(1,0,1,10,"Các dòng hàng được gộp từ toàn bộ sheet PO theo nhà cung cấp",&subtitle).map_err(|e|e.to_string())?;
    let quote_heads=["Nhà cung cấp","Mã PO","Số PR","Mã hàng","Tên hàng","Quy cách","ĐVT","Số lượng","Đơn giá (VND)","Thành tiền (VND)","Giá mua cũ (VND)"];
    for(c,h)in quote_heads.iter().enumerate(){quotes.write_string_with_format(3,c as u16,*h,&header).map_err(|e|e.to_string())?;}
    for(i,r)in rows.iter().enumerate(){let n=i as u32+4;let row_format=if i%2==0{&body}else{&alternate};for(c,v)in[(0,r.supplier.as_str()),(1,r.po_number.as_str()),(2,r.pr.as_str()),(3,r.code.as_str()),(4,r.name.as_str()),(5,r.spec.as_str()),(6,r.unit.as_str())]{quotes.write_string_with_format(n,c as u16,v,row_format).map_err(|e|e.to_string())?;}quotes.write_number_with_format(n,7,r.quantity,&qty).map_err(|e|e.to_string())?;quotes.write_number_with_format(n,8,r.unit_price as f64,&money).map_err(|e|e.to_string())?;quotes.write_number_with_format(n,9,r.amount as f64,&money).map_err(|e|e.to_string())?;if let Some(price)=r.history_price{quotes.write_number_with_format(n,10,price as f64,&money).map_err(|e|e.to_string())?;}}
    let merged_total:i64=rows.iter().map(|r|r.amount).sum();let merged_total_row=rows.len() as u32+4;quotes.write_string_with_format(merged_total_row,8,"TỔNG CỘNG",&total_format).map_err(|e|e.to_string())?;quotes.write_number_with_format(merged_total_row,9,merged_total as f64,&total_format).map_err(|e|e.to_string())?;
    for(col,width)in[(0,25.0),(1,27.0),(2,30.0),(3,22.0),(4,38.0),(5,28.0),(6,10.0),(7,13.0),(8,22.0),(9,23.0),(10,22.0)]{quotes.set_column_width(col,width).map_err(|e|e.to_string())?;}quotes.set_row_height(0,30.0).map_err(|e|e.to_string())?;quotes.set_row_height(3,34.0).map_err(|e|e.to_string())?;quotes.set_freeze_panes(4,0).map_err(|e|e.to_string())?;quotes.autofilter(3,0,merged_total_row.saturating_sub(1),10).map_err(|e|e.to_string())?;

    let no_quote=workbook.add_worksheet();no_quote.set_name("Chưa có báo giá").map_err(|e|e.to_string())?;no_quote.set_tab_color(Color::RGB(0xC77836));
    no_quote.merge_range(0,0,0,8,"TỔNG HỢP MÃ HÀNG CHƯA CÓ BÁO GIÁ",&title).map_err(|e|e.to_string())?;
    let no_quote_subtitle=format!("{} mã hàng · {} dòng PR trong phạm vi so sánh không có nhà cung cấp chào giá",no_quote_item_count,no_quote_rows.len());
    no_quote.merge_range(1,0,1,8,no_quote_subtitle.as_str(),&subtitle).map_err(|e|e.to_string())?;
    let missing_heads=["Dự án","Số PR","Mã hàng","Tên hàng","Quy cách","ĐVT","Số lượng","File PR nguồn","Dòng nguồn"];
    for(c,h)in missing_heads.iter().enumerate(){no_quote.write_string_with_format(3,c as u16,*h,&header).map_err(|e|e.to_string())?;}
    for(i,r)in no_quote_rows.iter().enumerate(){let n=i as u32+4;let row_format=if i%2==0{&body}else{&alternate};let source=Path::new(&r.source_path).file_name().and_then(|v|v.to_str()).unwrap_or(&r.source_path);for(c,v)in[(0,r.project.as_str()),(1,r.pr.as_str()),(2,r.code.as_str()),(3,r.name.as_str()),(4,r.spec.as_str()),(5,r.unit.as_str()),(7,source)]{no_quote.write_string_with_format(n,c as u16,v,row_format).map_err(|e|e.to_string())?;}no_quote.write_number_with_format(n,6,r.quantity,&qty).map_err(|e|e.to_string())?;no_quote.write_number_with_format(n,8,r.source_row as f64,&body).map_err(|e|e.to_string())?;}
    if no_quote_rows.is_empty(){no_quote.merge_range(4,0,4,8,"Không có mã hàng thiếu báo giá trong đợt này.",&subtitle).map_err(|e|e.to_string())?;}
    for(col,width)in[(0,22.0),(1,30.0),(2,22.0),(3,36.0),(4,28.0),(5,10.0),(6,15.0),(7,36.0),(8,13.0)]{no_quote.set_column_width(col,width).map_err(|e|e.to_string())?;}no_quote.set_row_height(0,30.0).map_err(|e|e.to_string())?;no_quote.set_row_height(3,34.0).map_err(|e|e.to_string())?;no_quote.set_freeze_panes(4,0).map_err(|e|e.to_string())?;if !no_quote_rows.is_empty(){no_quote.autofilter(3,0,no_quote_rows.len() as u32+3,8).map_err(|e|e.to_string())?;}

    for(supplier,items)in &vendors{
        let po_number=&items[0].po_number;let total:i64=items.iter().map(|r|r.amount).sum();
        let sheet=workbook.add_worksheet();let safe_name=supplier.chars().take(25).collect::<String>();sheet.set_name(&safe_name).map_err(|e|e.to_string())?;sheet.set_tab_color(Color::RGB(0x31818A));
        sheet.merge_range(0,0,0,8,"ĐƠN ĐẶT HÀNG · BẢN DỰ THẢO",&title).map_err(|e|e.to_string())?;
        sheet.write_string_with_format(1,0,"Nhà cung cấp",&subtitle).map_err(|e|e.to_string())?;sheet.merge_range(1,1,1,3,supplier,&body).map_err(|e|e.to_string())?;
        sheet.write_string_with_format(2,0,"Mã PO",&subtitle).map_err(|e|e.to_string())?;sheet.merge_range(2,1,2,3,po_number,&body).map_err(|e|e.to_string())?;
        sheet.write_string_with_format(1,5,"Tổng tiền (VND)",&subtitle).map_err(|e|e.to_string())?;sheet.merge_range(1,6,2,8,"",&total_format).map_err(|e|e.to_string())?;sheet.write_number_with_format(1,6,total as f64,&total_format).map_err(|e|e.to_string())?;
        let heads=["Số PR","Mã hàng","Tên hàng","Quy cách","ĐVT","Số lượng","Đơn giá (VND)","Thành tiền (VND)","Giá mua cũ (VND)"];
        for(c,h)in heads.iter().enumerate(){sheet.write_string_with_format(4,c as u16,*h,&header).map_err(|e|e.to_string())?;}
        for(i,r)in items.iter().enumerate(){let n=i as u32+5;let row_format=if i%2==0{&body}else{&alternate};for(c,v)in[(0,&r.pr),(1,&r.code),(2,&r.name),(3,&r.spec),(4,&r.unit)]{sheet.write_string_with_format(n,c as u16,v,row_format).map_err(|e|e.to_string())?;}sheet.write_number_with_format(n,5,r.quantity,&qty).map_err(|e|e.to_string())?;sheet.write_number_with_format(n,6,r.unit_price as f64,&money).map_err(|e|e.to_string())?;sheet.write_number_with_format(n,7,r.amount as f64,&money).map_err(|e|e.to_string())?;if let Some(p)=r.history_price{sheet.write_number_with_format(n,8,p as f64,&money).map_err(|e|e.to_string())?;}}
        let total_row=items.len() as u32+5;sheet.write_string_with_format(total_row,6,"Tổng cộng",&total_format).map_err(|e|e.to_string())?;sheet.write_number_with_format(total_row,7,total as f64,&total_format).map_err(|e|e.to_string())?;
        for(col,width)in[(0,22.0),(1,20.0),(2,34.0),(3,25.0),(4,9.0),(5,13.0),(6,20.0),(7,22.0),(8,22.0)]{sheet.set_column_width(col,width).map_err(|e|e.to_string())?;}sheet.set_row_height(0,30.0).map_err(|e|e.to_string())?;sheet.set_row_height(4,34.0).map_err(|e|e.to_string())?;sheet.set_freeze_panes(5,0).map_err(|e|e.to_string())?;sheet.autofilter(4,0,total_row-1,8).map_err(|e|e.to_string())?;
    }
    let trace=workbook.add_worksheet();trace.set_name("Truy vết nguồn").map_err(|e|e.to_string())?;trace.set_tab_color(Color::RGB(0x84999C));trace.merge_range(0,0,0,9,"TRUY VẾT DỮ LIỆU NGUỒN",&title).map_err(|e|e.to_string())?;trace.merge_range(1,0,1,9,"Thông tin nguồn PR và báo giá dùng để tạo từng dòng phân bổ",&subtitle).map_err(|e|e.to_string())?;let heads=["Nhà cung cấp","Mã PO","Số PR","Mã hàng","Số lượng","Đơn giá (VND)","File PR","Dòng PR","File báo giá","Dòng báo giá"];
    for(c,h)in heads.iter().enumerate(){trace.write_string_with_format(2,c as u16,*h,&header).map_err(|e|e.to_string())?;}
    for(i,r)in provenance.iter().enumerate(){let n=i as u32+3;let row_format=if i%2==0{&body}else{&alternate};let pr_file=Path::new(&r.pr_source).file_name().and_then(|v|v.to_str()).unwrap_or(&r.pr_source);let quote_file=Path::new(&r.quote_source).file_name().and_then(|v|v.to_str()).unwrap_or(&r.quote_source);for(c,v)in[(0,&r.supplier),(1,&r.po_number),(2,&r.pr),(3,&r.code)]{trace.write_string_with_format(n,c as u16,v,row_format).map_err(|e|e.to_string())?;}trace.write_string_with_format(n,6,pr_file,row_format).map_err(|e|e.to_string())?;trace.write_string_with_format(n,8,quote_file,row_format).map_err(|e|e.to_string())?;trace.write_number_with_format(n,4,r.quantity,&qty).map_err(|e|e.to_string())?;trace.write_number_with_format(n,5,r.unit_price as f64,&money).map_err(|e|e.to_string())?;trace.write_number_with_format(n,7,r.pr_row as f64,&body).map_err(|e|e.to_string())?;if let Some(row)=r.quote_row{trace.write_number_with_format(n,9,row as f64,&body).map_err(|e|e.to_string())?;}}
    for(col,width)in[(0,27.0),(1,27.0),(2,30.0),(3,20.0),(4,13.0),(5,20.0),(6,36.0),(7,12.0),(8,36.0),(9,14.0)]{trace.set_column_width(col,width).map_err(|e|e.to_string())?;}trace.set_row_height(2,34.0).map_err(|e|e.to_string())?;trace.set_freeze_panes(3,0).map_err(|e|e.to_string())?;if !provenance.is_empty(){trace.autofilter(2,0,provenance.len() as u32+2,9).map_err(|e|e.to_string())?;}
    workbook.save(path).map_err(|e|e.to_string())?;Ok(vendors.len())
}

#[tauri::command]
fn backup_database(path:String,app:tauri::AppHandle,state:State<'_,AppState>)->Result<usize,String>{
    let db=state.0.lock().map_err(|_|"Database đang bận")?;
    let temp_dir=app.path().app_data_dir().map_err(|e|e.to_string())?;fs::create_dir_all(&temp_dir).map_err(|e|e.to_string())?;
    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_millis();
    let snapshot=temp_dir.join(format!("backup-snapshot-{stamp}.sqlite3"));
    {
        let mut target=Connection::open(&snapshot).map_err(|e|e.to_string())?;
        Backup::new(&db,&mut target).map_err(|e|e.to_string())?.run_to_completion(64,Duration::from_millis(10),None).map_err(|e|e.to_string())?;
    }
    let sources={let mut stmt=db.prepare("SELECT source_path FROM import_batch UNION SELECT source_path FROM history_batch UNION SELECT source_path FROM quote_batch").map_err(|e|e.to_string())?;stmt.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?};
    let output=File::create(&path).map_err(|e|e.to_string())?;
    let mut zip=ZipWriter::new(output);let options=SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("database.sqlite3",options).map_err(|e|e.to_string())?;let mut db_file=File::open(&snapshot).map_err(|e|e.to_string())?;std::io::copy(&mut db_file,&mut zip).map_err(|e|e.to_string())?;
    let source_dir=app.path().app_data_dir().map_err(|e|e.to_string())?.join("restored_sources");fs::create_dir_all(&source_dir).map_err(|e|e.to_string())?;
    let mut manifest=Vec::<BackupSource>::new();let mut seen=std::collections::HashSet::new();let mut included=0;
    for (index,original) in sources.into_iter().enumerate(){if !seen.insert(original.clone())||!Path::new(&original).is_file(){continue}let name=Path::new(&original).file_name().and_then(|n|n.to_str()).unwrap_or("source-file").chars().filter(|c|c.is_ascii_alphanumeric()||matches!(c,'.'|'_'|'-')).collect::<String>();let archive=format!("sources/{index:05}-{name}");zip.start_file(&archive,options).map_err(|e|e.to_string())?;let mut source=File::open(&original).map_err(|e|e.to_string())?;std::io::copy(&mut source,&mut zip).map_err(|e|e.to_string())?;manifest.push(BackupSource{original,archive});included+=1;}
    zip.start_file("manifest.json",options).map_err(|e|e.to_string())?;zip.write_all(&serde_json::to_vec(&manifest).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;zip.finish().map_err(|e|e.to_string())?;
    let _=fs::remove_file(snapshot);db.execute("INSERT INTO app_state(key,value) VALUES ('last_backup_at',datetime('now')) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[]).map_err(|e|e.to_string())?;Ok(included)
}

#[tauri::command]
fn restore_database(path:String,app:tauri::AppHandle,state:State<'_,AppState>)->Result<usize,String>{
    let data_dir=app.path().app_data_dir().map_err(|e|e.to_string())?;fs::create_dir_all(&data_dir).map_err(|e|e.to_string())?;
    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_millis();
    let temp_db=data_dir.join(format!("restore-{stamp}.sqlite3"));let source_root=data_dir.join(format!("restored_sources/{stamp}"));fs::create_dir_all(&source_root).map_err(|e|e.to_string())?;
    let result=(||->Result<usize,String>{
        let file=File::open(&path).map_err(|e|format!("Không mở được gói sao lưu: {e}"))?;let mut archive=ZipArchive::new(file).map_err(|e|format!("Gói sao lưu không hợp lệ: {e}"))?;
        if archive.len()>10_002{return Err("Gói sao lưu có quá nhiều tệp".into())}let mut expanded=0u64;for index in 0..archive.len(){let entry=archive.by_index(index).map_err(|e|e.to_string())?;if entry.size()>2_000_000_000{return Err("Một tệp trong gói sao lưu vượt quá giới hạn 2 GB".into())}expanded=expanded.saturating_add(entry.size());if expanded>4_000_000_000{return Err("Gói sao lưu vượt quá giới hạn tổng dung lượng giải nén".into())}}
        {let mut database=archive.by_name("database.sqlite3").map_err(|_|"Gói sao lưu thiếu database.sqlite3")?;let mut out=File::create(&temp_db).map_err(|e|e.to_string())?;std::io::copy(&mut database,&mut out).map_err(|e|e.to_string())?;}
        let manifest={let mut f=archive.by_name("manifest.json").map_err(|_|"Gói sao lưu thiếu manifest nguồn")?;serde_json::from_reader::<_,Vec<BackupSource>>(&mut f).map_err(|e|format!("Manifest không hợp lệ: {e}"))?};
        let mut restored=Vec::new();
        for (index,item) in manifest.iter().enumerate(){if !item.archive.starts_with("sources/")||item.archive.contains("..")||item.archive.contains('\\'){return Err("Đường dẫn tệp trong gói sao lưu không hợp lệ".into())}let mut entry=archive.by_name(&item.archive).map_err(|e|e.to_string())?;let name=Path::new(&item.original).file_name().and_then(|n|n.to_str()).unwrap_or("source-file");let destination=source_root.join(format!("{index:05}-{name}"));let mut out=File::create(&destination).map_err(|e|e.to_string())?;std::io::copy(&mut entry,&mut out).map_err(|e|e.to_string())?;restored.push((item.original.clone(),destination.to_string_lossy().to_string()));}
        let incoming=Connection::open(&temp_db).map_err(|e|e.to_string())?;let integrity:String=incoming.query_row("PRAGMA integrity_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;if integrity!="ok"{return Err(format!("Database sao lưu không toàn vẹn: {integrity}"))}
        for table in ["import_batch","history_batch","quote_batch"]{let exists:i64=incoming.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",[table],|r|r.get(0)).map_err(|e|e.to_string())?;if exists==0{return Err(format!("Database sao lưu thiếu bảng {table}"))}}
        for(original,new_path)in &restored{for(table,column)in [("import_batch","source_path"),("history_batch","source_path"),("quote_batch","source_path"),("purchase_history","source_path")]{let sql=format!("UPDATE {table} SET {column}=?1 WHERE {column}=?2");incoming.execute(&sql,params![new_path,original]).map_err(|e|e.to_string())?;}}
        let mut db=state.0.lock().map_err(|_|"Database đang bận")?;db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").map_err(|e|e.to_string())?;
        let safety_path=data_dir.join(format!("before-restore-{stamp}.sqlite3"));let mut safety=Connection::open(&safety_path).map_err(|e|e.to_string())?;Backup::new(&db,&mut safety).map_err(|e|e.to_string())?.run_to_completion(64,Duration::from_millis(10),None).map_err(|e|e.to_string())?;
        if let Err(error)=Backup::new(&incoming,&mut db).and_then(|backup|backup.run_to_completion(64,Duration::from_millis(10),None)){let _=Backup::new(&safety,&mut db).and_then(|backup|backup.run_to_completion(64,Duration::from_millis(10),None));return Err(format!("Phục hồi thất bại; database hiện tại đã được khôi phục lại: {error}"))}
        let _=fs::remove_file(safety_path);
        db.execute_batch("CREATE TABLE IF NOT EXISTS audit_event (id INTEGER PRIMARY KEY, event_type TEXT NOT NULL, entity_type TEXT NOT NULL, entity_id INTEGER NOT NULL DEFAULT 0, details TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)").map_err(|e|e.to_string())?;
        db.execute("INSERT INTO audit_event(event_type,entity_type,entity_id,details) VALUES ('restore','database',0,?1)",[path]).ok();
        Ok(restored.len())
    })();
    let _=fs::remove_file(&temp_db);result
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let path = db_path(&app.handle()).map_err(std::io::Error::other)?;
            let db = init_db(path).map_err(std::io::Error::other)?;
            app.manage(AppState(Mutex::new(db),Mutex::new(std::collections::HashMap::new())));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![preview_workbook, review_workbook, read_pdf_bytes, import_pr_workbook, import_history_workbook, import_quote_workbook, import_quote_pdf, cancel_import, get_import_sources, get_recovery_draft, save_recovery_draft, clear_recovery_draft, get_data_health, get_quote_batch_preview, get_item_history_preview, delete_import_source, get_source_data_cleared, clear_source_data, clear_source_kind, get_pr_page, get_pr_ids, get_pr_projects, get_pr_months, get_suppliers, get_supplier_priorities, set_supplier_priority, get_comparison_rows, save_allocations, update_work_batch, get_work_batches, get_work_batch_allocations, get_work_batch_no_quote_ids, get_work_batch_scope_ids, get_po_summaries, get_po_preflight_review, get_work_report, rename_work_batch, generate_purchase_orders, export_po_workbook, backup_database, restore_database])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}

#[cfg(test)]
mod tests {
    use super::{build_work_report, display_date_text, ensure_allocation_report_columns, ensure_quote_provenance_columns, ensure_work_batch_scope_columns, format_excel_serial_date, init_db, po_preflight_sql, po_review_issue, project_from_pr, report_date_key, report_price_delta, report_signed_price_delta, PR_PROJECT_EXPR_SQL, stored_project_expr_sql};

    #[test]
    fn po_review_flags_missing_and_partial_allocations(){
        assert_eq!(po_review_issue(true,5.0,5.0),"Chưa có báo giá");
        assert_eq!(po_review_issue(false,5.0,0.0),"Chưa phân bổ");
        assert_eq!(po_review_issue(false,5.0,4.5),"Phân bổ chưa đủ");
        assert_eq!(po_review_issue(false,5.0,5.0),"");
    }

    #[test]
    fn po_preflight_reads_full_saved_scope_and_sums_split_allocations(){
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE work_batch_scope(work_batch_id INTEGER,pr_line_id INTEGER,project TEXT,pr_number TEXT,item_code TEXT,item_name TEXT,spec TEXT,unit TEXT,quantity REAL,ordered REAL,source_row INTEGER); CREATE TABLE work_batch_no_quote(work_batch_id INTEGER,pr_line_id INTEGER); CREATE TABLE allocation(work_batch_id INTEGER,pr_line_id INTEGER,supplier TEXT,quantity REAL,unit_price INTEGER);").unwrap();
        for(id,code,qty)in[(1,"A-1",5.0),(2,"B-2",5.0),(3,"C-3",4.0)]{db.execute("INSERT INTO work_batch_scope VALUES(1,?1,'P','PR-1',?2,?2,'','PCS',?3,0,?1)",rusqlite::params![id,code,qty]).unwrap();}
        db.execute("INSERT INTO allocation VALUES(1,1,'NCC A',2,100),(1,1,'NCC B',1,150),(1,2,'NCC C',5,40)",[]).unwrap();
        db.execute("INSERT INTO work_batch_no_quote VALUES(1,3)",[]).unwrap();
        let mut stmt=db.prepare(po_preflight_sql(true)).unwrap();
        let rows=stmt.query_map([1],|r|Ok((r.get::<_,String>(3)?,r.get::<_,f64>(8)?,r.get::<_,f64>(9)?,r.get::<_,i64>(10)?,r.get::<_,String>(11)?,r.get::<_,bool>(12)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
        assert_eq!(rows.len(),3);
        assert_eq!(rows[0],("A-1".into(),5.0,3.0,350,"NCC A,NCC B".into(),false));
        assert_eq!(rows[1].2,5.0);
        assert!(rows[2].5);
    }

    #[test]
    fn upgrades_older_work_batch_scope_and_backfills_line_details(){
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE work_batch_scope(work_batch_id INTEGER,pr_line_id INTEGER,PRIMARY KEY(work_batch_id,pr_line_id)); CREATE TABLE pr_line(id INTEGER,project TEXT,pr_number TEXT,item_code TEXT,item_name TEXT,spec TEXT,unit TEXT,quantity REAL,ordered REAL,source_row INTEGER); INSERT INTO pr_line VALUES(7,'MEC','DES-MEC-01','ITEM-7','Băng tải','P2','PCS',4,1,29); INSERT INTO work_batch_scope VALUES(3,7);").unwrap();
        ensure_work_batch_scope_columns(&db).unwrap();
        let row:(String,String,String,String,f64,f64,i64)=db.query_row("SELECT project,pr_number,item_code,item_name,quantity,ordered,source_row FROM work_batch_scope WHERE work_batch_id=3",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).unwrap();
        assert_eq!(row,("MEC".into(),"DES-MEC-01".into(),"ITEM-7".into(),"Băng tải".into(),4.0,1.0,29));
    }

    #[test]
    fn upgrades_allocation_reason_and_parses_quote_expiry_dates(){
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE allocation(id INTEGER PRIMARY KEY,pr_line_id INTEGER); INSERT INTO allocation(id,pr_line_id) VALUES(1,2);").unwrap();
        ensure_allocation_report_columns(&db).unwrap();
        let reason:String=db.query_row("SELECT selection_reason FROM allocation WHERE id=1",[],|r|r.get(0)).unwrap();
        assert_eq!(reason,"");
        assert_eq!(report_date_key("30/10/2026"),Some(20261030));
        assert_eq!(report_date_key("2026-10-30"),Some(20261030));
        assert_eq!(report_date_key(""),None);
        assert_eq!(report_price_delta(Some(130),Some(100)),Some(30.0));
        assert_eq!(report_price_delta(Some(70),Some(100)),Some(30.0));
        assert_eq!(report_price_delta(Some(90),Some(0)),None);
        assert_eq!(report_signed_price_delta(Some(150),Some(120)),Some(25.0));
        assert!((report_signed_price_delta(Some(100),Some(120)).unwrap()+16.6666667).abs()<0.0001);
    }

    #[test]
    fn upgrades_quote_source_sheet_and_pdf_page_fields(){
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE quote_batch(id INTEGER PRIMARY KEY,source_path TEXT); CREATE TABLE quote_line(id INTEGER PRIMARY KEY,source_row INTEGER); INSERT INTO quote_batch(id,source_path) VALUES(1,'quote.xlsx'); INSERT INTO quote_line(id,source_row) VALUES(1,8);").unwrap();
        ensure_quote_provenance_columns(&db).unwrap();
        let sheet:String=db.query_row("SELECT sheet_name FROM quote_batch WHERE id=1",[],|r|r.get(0)).unwrap();
        let page:i64=db.query_row("SELECT source_page FROM quote_line WHERE id=1",[],|r|r.get(0)).unwrap();
        assert_eq!(sheet,"");
        assert_eq!(page,0);
    }

    #[test]
    fn work_report_summarizes_issues_prices_and_source_provenance(){
        let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path=std::env::temp_dir().join(format!("pr2po-report-{}-{stamp}.sqlite3",std::process::id()));
        let db=init_db(path.clone()).unwrap();
        db.execute("INSERT INTO import_batch(id,source_path,sheet_name,file_hash) VALUES(1,'pr.xlsx','Sheet1','pr-hash')",[]).unwrap();
        db.execute("INSERT INTO history_batch(id,source_path,file_hash) VALUES(1,'history.xlsx','history-hash')",[]).unwrap();
        db.execute("INSERT INTO purchase_history(batch_id,source_row,item_code,item_name,supplier,quantity,unit_price,amount) VALUES(1,10,'ITEM-1','Băng tải','NCC CŨ',5,100,500)",[]).unwrap();
        db.execute("INSERT INTO quote_batch(id,source_path,file_hash,supplier,quote_number,sheet_name,imported_at) VALUES(1,'low.xlsx','low-hash','NCC THẤP','BG-LOW','Bao gia',datetime('now')),(2,'chosen.pdf','pdf-hash','NCC CHỌN','BG-02','PDF',datetime('now')),(3,'expired.xlsx','expired-hash','NCC HẾT HẠN','BG-03','Sheet1','2020-01-01 00:00:00')",[]).unwrap();
        db.execute("INSERT INTO quote_line(batch_id,source_row,source_page,item_code,unit_price,valid_until) VALUES(1,4,0,'ITEM-1',120,'31/12/2099'),(2,9,2,'ITEM-1',150,'31/12/2099'),(3,12,0,'ITEM-3',50,'01/01/2020')",[]).unwrap();
        db.execute("INSERT INTO work_batch(id,name) VALUES(1,'Kiểm thử báo cáo')",[]).unwrap();
        for (id,code,qty) in [(1,"ITEM-1",5.0),(2,"ITEM-2",3.0),(3,"ITEM-3",2.0)] {
            db.execute("INSERT INTO pr_line(id,batch_id,source_row,project,pr_number,item_code,item_name,spec,unit,quantity,due_date,ordered,status) VALUES(?1,1,?1,'PROJECT','PR-1',?2,?2,'','PCS',?3,'',0,'Chưa xử lý')",rusqlite::params![id,code,qty]).unwrap();
            db.execute("INSERT INTO work_batch_scope(work_batch_id,pr_line_id,project,pr_number,item_code,item_name,spec,unit,quantity,ordered,source_row) VALUES(1,?1,'PROJECT','PR-1',?2,?2,'','PCS',?3,0,?1)",rusqlite::params![id,code,qty]).unwrap();
        }
        db.execute("INSERT INTO allocation(work_batch_id,pr_line_id,supplier,quantity,unit_price,quote_line_id,selection_reason) VALUES(1,1,'NCC CHỌN',5,150,2,'Chọn tay'),(1,2,'NCC THỦ CÔNG',1,80,NULL,'Sửa giá thủ công')",[]).unwrap();
        let report=build_work_report(&db,Some(1),"issues".into(),30.0,90,0,100).unwrap();
        assert_eq!(report.summary.total_lines,3);
        assert_eq!(report.summary.no_quote_lines,1);
        assert_eq!(report.summary.unallocated_lines,1);
        assert_eq!(report.summary.partial_lines,1);
        assert_eq!(report.summary.expired_quote_lines,1);
        assert_eq!(report.summary.stale_quote_lines,1);
        assert_eq!(report.summary.manual_price_lines,1);
        assert_eq!(report.summary.anomaly_lines,1);
        assert_eq!(report.summary.total_amount,830);
        assert_eq!(report.total_rows,3);
        let chosen=report.rows.iter().find(|row|row.item_code=="ITEM-1").unwrap();
        assert_eq!(chosen.best_supplier,"NCC THẤP");
        assert_eq!(chosen.selection_delta_percent,Some(25.0));
        assert_eq!(chosen.selection_reason,"Chọn tay");
        assert_eq!(chosen.quote_sheets,"PDF");
        assert_eq!(chosen.quote_rows,"Trang 2 · STT 9");
        let manual=build_work_report(&db,Some(1),"manual-price".into(),30.0,90,0,100).unwrap();
        assert_eq!(manual.total_rows,1);
        assert!(manual.rows[0].manual_price);
        let anomalies=build_work_report(&db,Some(1),"anomaly".into(),60.0,90,0,100).unwrap();
        assert_eq!(anomalies.total_rows,0);
        drop(db);
        for suffix in ["","-wal","-shm"]{let _=std::fs::remove_file(format!("{}{suffix}",path.display()));}
    }

    #[test]
    fn extracts_project_segment_from_pr_number() {
        assert_eq!(project_from_pr("AGV-MEC2501004-23-261001"), "MEC2501004");
        assert_eq!(project_from_pr("DES-AUTM2609F6-04-261030"), "AUTM2609F6");
        assert_eq!(project_from_pr("MEC2501004-23-261001"), "MEC2501004");
        assert_eq!(project_from_pr("COMMON-ICT-266-261006"), "COMMON");
        assert_eq!(project_from_pr("SALES2033-261007"), "SALES");
        assert_eq!(project_from_pr("GA-248-261008"), "GA");
    }

    #[test]
    fn project_filter_sql_finds_projects_from_pr_numbers() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        let sql = format!("SELECT {PR_PROJECT_EXPR_SQL} FROM (SELECT '' AS project, ?1 AS pr_number)");
        for (pr, expected) in [("AGV-MEC2501004-23-261001", "MEC2501004"), ("DES-AUTM2609F6-04-261030", "AUTM2609F6"), ("COMMON-ICT-266-261006", "COMMON"), ("SALES2033-261007", "SALES"), ("GA-248-261008", "GA")] {
            let actual: String = db.query_row(&sql, [pr], |row| row.get(0)).unwrap();
            assert_eq!(actual, expected);
        }
        let stored_sql=format!("SELECT {} FROM (SELECT ?1 AS project, ?2 AS pr_number)",stored_project_expr_sql());
        for (old_project,pr,expected) in [("ICT","COMMON-ICT-266-261006","COMMON"),("SALES2033","SALES2033-261007","SALES"),("248","GA-248-261008","GA")] {
            let actual:String=db.query_row(&stored_sql,[old_project,pr],|row|row.get(0)).unwrap();
            assert_eq!(actual,expected);
        }
    }

    #[test]
    fn pr_filter_supports_more_than_500_selected_pr_numbers() {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE pr_line(batch_id INTEGER,project TEXT,pr_number TEXT,item_code TEXT,item_name TEXT)").unwrap();
        for index in 0..750{db.execute("INSERT INTO pr_line VALUES (1,'P',?1,?2,'')",rusqlite::params![format!("PR-{index}"),format!("ITEM-{index}")]).unwrap();}
        let selected=(0..750).map(|index|format!("PR-{index}")).collect::<Vec<_>>();
        let (filter,mut values)=super::pr_filter_sql("","Tất cả dự án",&selected,"");values.insert(0,rusqlite::types::Value::Integer(1));
        let count:i64=db.query_row(&format!("SELECT COUNT(*) FROM pr_line WHERE {filter}"),rusqlite::params_from_iter(values.iter()),|row|row.get(0)).unwrap();
        assert_eq!(count,750);
    }

    #[test]
    fn formats_excel_serial_dates_for_import_and_existing_rows() {
        assert_eq!(format_excel_serial_date(45_943.5).as_deref(), Some("13/10/2025 12:00"));
        assert_eq!(display_date_text("45943".into()), "13/10/2025");
        assert_eq!(display_date_text("2026-10-01".into()), "01/10/2026");
    }
}
