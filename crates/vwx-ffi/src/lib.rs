//! C ABI of the VWX reader (`vwx.dll`).
//!
//! ```c
//! VwxDocument *doc = vwx_open("house.vwx");          // NULL when the file cannot be read
//! char *json = vwx_query(doc, "{\"query\": \"info\"}");
//! /* ... */
//! vwx_free(json);
//! vwx_close(doc);
//! ```
//!
//! `vwx_query` takes a JSON request and returns a JSON answer, `{"ok": true, "result": …}` or
//! `{"ok": false, "error": "…"}`; the caller frees it with `vwx_free`. Requests:
//! - `{"query": "info"}`: document facts;
//! - `{"query": "probe"}`: the analysis summary of the bytes (see `vwx_core::Probe`);
//! - `{"query": "document"}`: the whole neutral model (layers, classes, objects);
//! - `{"query": "objects", "kind": "wall", "limit": 10, "offset": 0}`: objects of one kind.

use std::ffi::{c_char, CStr, CString};

use serde_json::{json, Value};

/// An open file behind the C ABI: its bytes and, when they decode, the document.
pub struct VwxDocument {
    data: Vec<u8>,
    doc: Result<vwx_model::Document, String>,
}

impl VwxDocument {
    pub fn new(data: Vec<u8>) -> Self {
        let doc = vwx_read::open(&data).map_err(|e| e.to_string());
        VwxDocument { data, doc }
    }
}

/// Answers one JSON request (the safe core of `vwx_query`).
pub fn query(doc: &VwxDocument, request: &str) -> Value {
    match answer(doc, request) {
        Ok(v) => json!({"ok": true, "result": v}),
        Err(e) => json!({"ok": false, "error": e}),
    }
}

fn answer(file: &VwxDocument, request: &str) -> Result<Value, String> {
    let req: Value = serde_json::from_str(request).map_err(|e| format!("request is not JSON: {e}"))?;
    let q = req.get("query").and_then(Value::as_str).ok_or("request has no \"query\"")?;
    let to = |v: Result<Value, serde_json::Error>| v.map_err(|e| e.to_string());
    let doc = || file.doc.as_ref().map_err(Clone::clone);
    match q {
        "info" => to(serde_json::to_value(match &file.doc {
            Ok(d) => d.info.clone(),
            Err(_) => vwx_read::info(&file.data),
        })),
        "probe" => to(serde_json::to_value(vwx_core::Probe::of(&file.data))),
        "document" => to(serde_json::to_value(doc()?)),
        "objects" => {
            let kind = req.get("kind").and_then(Value::as_str);
            let limit = req.get("limit").and_then(Value::as_u64).map_or(usize::MAX, |n| n as usize);
            let offset = req.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
            let all: Vec<_> = doc()?.objects.iter().filter(|o| kind.is_none_or(|k| o.kind == k)).collect();
            Ok(json!({"kind": kind, "total": all.len(), "objects": to(serde_json::to_value(all.iter().skip(offset).take(limit).collect::<Vec<_>>()))?}))
        }
        other => Err(format!("unknown query \"{other}\"")),
    }
}

fn into_c(v: Value) -> *mut c_char {
    CString::new(v.to_string()).map_or(std::ptr::null_mut(), CString::into_raw)
}

/// Opens a file by path (UTF-8). Returns NULL when the file cannot be read; a file that reads
/// but does not decode still opens (queries then answer the error).
///
/// # Safety
/// `path` is a NUL-terminated string or NULL.
#[no_mangle]
pub unsafe extern "C" fn vwx_open(path: *const c_char) -> *mut VwxDocument {
    if path.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(path) = CStr::from_ptr(path).to_str() else { return std::ptr::null_mut() };
    match std::fs::read(path) {
        Ok(data) => Box::into_raw(Box::new(VwxDocument::new(data))),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Opens a file from memory; the bytes are copied.
///
/// # Safety
/// `data` points at `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn vwx_open_bytes(data: *const u8, len: usize) -> *mut VwxDocument {
    if data.is_null() {
        return std::ptr::null_mut();
    }
    let bytes = std::slice::from_raw_parts(data, len).to_vec();
    Box::into_raw(Box::new(VwxDocument::new(bytes)))
}

/// Answers a JSON request; free the answer with [`vwx_free`].
///
/// # Safety
/// `doc` comes from `vwx_open*` and is not closed; `request` is a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn vwx_query(doc: *const VwxDocument, request: *const c_char) -> *mut c_char {
    if doc.is_null() || request.is_null() {
        return into_c(json!({"ok": false, "error": "null argument"}));
    }
    let request = CStr::from_ptr(request).to_string_lossy();
    into_c(query(&*doc, &request))
}

/// Frees a string returned by this library.
///
/// # Safety
/// `s` comes from this library and is freed once.
#[no_mangle]
pub unsafe extern "C" fn vwx_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// Closes a document.
///
/// # Safety
/// `doc` comes from `vwx_open*` and is closed once.
#[no_mangle]
pub unsafe extern "C" fn vwx_close(doc: *mut VwxDocument) {
    if !doc.is_null() {
        drop(Box::from_raw(doc));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_info_probe_and_errors() {
        let doc = VwxDocument::new(b"not decoded yet".to_vec());
        assert_eq!(query(&doc, r#"{"query":"info"}"#)["result"]["size"], 15);
        assert_eq!(query(&doc, r#"{"query":"probe"}"#)["result"]["size"], 15);
        let a = query(&doc, r#"{"query":"document"}"#);
        assert_eq!(a["ok"], false, "{a}");
        assert_eq!(query(&doc, "nonsense")["ok"], false);
        assert_eq!(query(&doc, r#"{"query":"what"}"#)["error"], "unknown query \"what\"");
    }

    #[test]
    fn c_round_trip() {
        unsafe {
            let bytes = b"abc";
            let doc = vwx_open_bytes(bytes.as_ptr(), bytes.len());
            let req = CString::new(r#"{"query":"info"}"#).unwrap();
            let answer = vwx_query(doc, req.as_ptr());
            assert!(CStr::from_ptr(answer).to_str().unwrap().contains("\"size\":3"));
            vwx_free(answer);
            vwx_close(doc);
        }
    }
}
