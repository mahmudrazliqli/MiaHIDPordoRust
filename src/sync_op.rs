#![allow(dead_code)]
use hidapi::HidDevice;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy)]
pub enum SyncKind {
    Output,
    FeatureSet,
    FeatureGet,
}

pub struct SyncRequest {
    pub kind: SyncKind,
    pub report_id: u8,
    pub has_report_id: bool,
    pub payload: Vec<u8>,
    pub read_len: usize, // فقط برای FeatureGet
    // برای برگشت به UI:
    pub rid_key: u8,
}

pub struct SyncResult {
    pub rid_key: u8,
    pub kind: SyncKind,
    pub ok: bool,
    pub bytes: usize,
    pub payload: Vec<u8>, // داده‌ی برگشتی (برای FeatureGet)
    pub err: String,
}

/// در thread جداگانه اجرا می‌شود. روی `device` قفل می‌گیرد (که UI هم
/// گاهی نگه می‌دارد، برای disconnect).
pub fn execute(device: &Arc<Mutex<Option<HidDevice>>>, req: &SyncRequest) -> SyncResult {
    let mut result = SyncResult {
        rid_key: req.rid_key,
        kind: req.kind,
        ok: false,
        bytes: 0,
        payload: Vec::new(),
        err: String::new(),
    };

    let guard = match device.lock() {
        Ok(g) => g,
        Err(_) => {
            result.err = "device lock poisoned".into();
            return result;
        }
    };

    let Some(dev) = guard.as_ref() else {
        result.err = "device not available".into();
        return result;
    };

    match req.kind {
        SyncKind::Output => {
            // اولین بایت = report ID (یا 0 اگر ندارد)
            let mut buf = Vec::with_capacity(req.payload.len() + 1);
            buf.push(if req.has_report_id { req.report_id } else { 0 });
            buf.extend_from_slice(&req.payload);

            match dev.write(&buf) {
                Ok(n) => {
                    result.ok = true;
                    result.bytes = n;
                }
                Err(e) => result.err = format!("{}", e),
            }
        }
        SyncKind::FeatureSet => {
            let mut buf = Vec::with_capacity(req.payload.len() + 1);
            buf.push(if req.has_report_id { req.report_id } else { 0 });
            buf.extend_from_slice(&req.payload);

            match dev.send_feature_report(&buf) {
                Ok(()) => {
                    result.ok = true;
                    result.bytes = buf.len();
                }
                Err(e) => result.err = format!("{}", e),
            }
        }
        SyncKind::FeatureGet => {
            let total = req.read_len.max(1);
            let mut buf = vec![0u8; total];
            buf[0] = if req.has_report_id { req.report_id } else { 0 };

            match dev.get_feature_report(&mut buf) {
                Ok(n) => {
                    result.ok = true;
                    result.bytes = n;
                    // بایت اول report id است، بقیه payload
                    if n > 1 {
                        result.payload = buf[1..n].to_vec();
                    }
                }
                Err(e) => result.err = format!("{}", e),
            }
        }
    }
    result
}
