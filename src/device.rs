use hidapi::HidApi;

pub struct DeviceEntry {
    pub path: String,
    pub id: String,
    pub label: String,
}

pub fn enumerate(api: &HidApi, saved_id: Option<&str>) -> (Vec<DeviceEntry>, Option<usize>) {
    let mut entries = Vec::new();
    let mut new_sel: Option<usize> = None;

    for info in api.device_list() {
        let vid = info.vendor_id();
        let pid = info.product_id();
        let mfg = info.manufacturer_string().unwrap_or("-");
        let prod = info.product_string().unwrap_or("-");
        let path = info.path().to_string_lossy().to_string();

        let serial = info.serial_number();
        let id = match serial {
            Some(s) if !s.is_empty() => format!("{:04x}:{:04x}:{}", vid, pid, s),
            _ => format!("{:04x}:{:04x}", vid, pid),
        };
        let label = format!("{:04x}:{:04x}  {} {}", vid, pid, mfg, prod);

        if let Some(saved) = saved_id {
            if saved == id {
                new_sel = Some(entries.len());
            }
        }
        entries.push(DeviceEntry { path, id, label });
    }
    (entries, new_sel)
}

/// parse input like C code: hex "01 02 FF", "0x01,0x02,0xFF", or ascii text
pub fn parse_input(text: &str, max_len: usize, hex_mode: bool) -> Vec<u8> {
    if !hex_mode {
        let bytes = text.as_bytes();
        let n = bytes.len().min(max_len);
        return bytes[..n].to_vec();
    }

    let mut out: Vec<u8> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() && out.len() < max_len {
        // skip separators
        while i < bytes.len() {
            let c = bytes[i];
            if c == b' ' || c == b'\t' || c == b',' || c == b':' || c == b'\n' || c == b'\r' {
                i += 1;
            } else {
                break;
            }
        }
        if i >= bytes.len() {
            break;
        }

        // skip 0x / 0X
        if i + 1 < bytes.len()
            && bytes[i] == b'0'
            && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X')
            && i + 2 < bytes.len() && bytes[i + 2].is_ascii_hexdigit() {
                i += 2;
            }

        let mut val: u32 = 0;
        let mut digits = 0;
        while digits < 2 && i < bytes.len() && bytes[i].is_ascii_hexdigit() {
            let c = bytes[i];
            let d = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => break,
            };
            val = val * 16 + d as u32;
            i += 1;
            digits += 1;
        }
        if digits == 0 {
            break;
        }
        out.push(val as u8);
    }
    out
}

pub fn format_bytes(data: &[u8], hex_mode: bool) -> String {
    if hex_mode {
        data.iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        data.iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    (b as char).to_string()
                } else {
                    ".".to_string()
                }
            })
            .collect()
    }
}
