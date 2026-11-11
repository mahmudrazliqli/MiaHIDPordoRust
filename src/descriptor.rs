use crate::app::{ReportSizes, MAX_REPORT_ID};

const MAX_GLOBAL_STACK: usize = 4;

fn bits_to_bytes(bits: u32) -> u32 {
    (bits as u64).div_ceil(8) as u32
}

pub fn parse_report_descriptor(desc: &[u8]) -> ([ReportSizes; MAX_REPORT_ID], bool) {
    #[derive(Clone, Copy, Default)]
    struct G {
        report_size: u32,
        report_count: u32,
        report_id: u8,
    }

    let mut cur = G::default();
    let mut stack = [G::default(); MAX_GLOBAL_STACK];
    let mut sp = 0;
    let mut has_report_id = false;
    let mut out = [ReportSizes::default(); MAX_REPORT_ID];
    let mut i = 0;

    while i < desc.len() {
        let prefix = desc[i];
        i += 1;

        if prefix == 0xFE {
            if i + 1 >= desc.len() {
                break;
            }
            let dlen = desc[i] as usize;
            if i + 2 + dlen > desc.len() {
                break;
            }
            i += 2 + dlen;
            continue;
        }

        let size_code = (prefix & 0x03) as usize;
        let type_ = (prefix >> 2) & 0x03;
        let tag = (prefix >> 4) & 0x0F;
        let data_size = if size_code == 3 { 4 } else { size_code };

        let mut data: u32 = 0;
        for b in 0..data_size {
            if i < desc.len() {
                data |= (desc[i] as u32) << (8 * b);
                i += 1;
            }
        }

        match type_ {
            1 => match tag {
                0x7 => cur.report_size = data,
                0x9 => cur.report_count = data,
                0x8 => {
                    cur.report_id = (data & 0xFF) as u8;
                    has_report_id = true;
                }
                0xA => {
                    if sp < MAX_GLOBAL_STACK {
                        stack[sp] = cur;
                        sp += 1;
                    } else {
                        eprintln!("descriptor: PUSH stack overflow");
                    }
                }
                0xB => {
                    if sp > 0 {
                        sp -= 1;
                        cur = stack[sp];
                    } else {
                        eprintln!("descriptor: POP on empty stack");
                    }
                }
                _ => {}
            },
            0 => {
                let n = (cur.report_size as u64)
                    .checked_mul(cur.report_count as u64)
                    .unwrap_or(0)
                    .min(0xFFFF_FFFF) as u32;
                let rid = (cur.report_id as usize) & 0xFF;
                match tag {
                    0x8 => out[rid].input_bits = out[rid].input_bits.saturating_add(n),
                    0x9 => out[rid].output_bits = out[rid].output_bits.saturating_add(n),
                    0xB => out[rid].feature_bits = out[rid].feature_bits.saturating_add(n),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    for s in out.iter_mut() {
        s.input_bytes = bits_to_bytes(s.input_bits);
        s.output_bytes = bits_to_bytes(s.output_bits);
        s.feature_bytes = bits_to_bytes(s.feature_bits);
    }

    (out, has_report_id)
}

// ============================================================================
// Formatter: HID Report Descriptor به متن خوانا
// ============================================================================

fn usage_page_name(page: u32) -> Option<&'static str> {
    Some(match page {
        0x01 => "Generic Desktop",
        0x02 => "Simulation Controls",
        0x03 => "VR Controls",
        0x04 => "Sport Controls",
        0x05 => "Game Controls",
        0x06 => "Generic Device",
        0x07 => "Keyboard/Keypad",
        0x08 => "LED",
        0x09 => "Button",
        0x0A => "Ordinal",
        0x0B => "Telephony",
        0x0C => "Consumer",
        0x0D => "Digitizer",
        0x0E => "Haptics",
        0x0F => "Physical Input Device",
        0x10 => "Unicode",
        0x14 => "Alphanumeric Display",
        0x40 => "Medical Instruments",
        0x80 => "Monitor",
        0x84 => "Power",
        0x85 => "Battery System",
        0x8C => "Bar Code Scanner",
        0x8D => "Scale",
        _ => return None,
    })
}

fn usage_name(page: u32, usage: u32) -> Option<&'static str> {
    Some(match page {
        0x01 => match usage {
            0x00 => "Undefined",
            0x01 => "Pointer",
            0x02 => "Mouse",
            0x04 => "Joystick",
            0x05 => "Game Pad",
            0x06 => "Keyboard",
            0x07 => "Keypad",
            0x08 => "Multi-axis Controller",
            0x30 => "X",
            0x31 => "Y",
            0x32 => "Z",
            0x33 => "Rx",
            0x34 => "Ry",
            0x35 => "Rz",
            0x36 => "Slider",
            0x37 => "Dial",
            0x38 => "Wheel",
            0x39 => "Hat switch",
            _ => return None,
        },
        0x08 => match usage {
            0x01 => "Num Lock",
            0x02 => "Caps Lock",
            0x03 => "Scroll Lock",
            0x04 => "Compose",
            0x05 => "Kana",
            _ => return None,
        },
        0x0C => match usage {
            0x00 => "Unassigned",
            0x01 => "Consumer Control",
            0x30 => "Power",
            0x32 => "Sleep",
            0x38 => "Eject",
            0xE9 => "Volume Increment",
            0xEA => "Volume Decrement",
            0xE2 => "Mute",
            0xB5 => "Scan Next Track",
            0xB6 => "Scan Previous Track",
            0xB0 => "Play",
            0xB1 => "Pause",
            0xCD => "Play/Pause",
            0xB7 => "Stop",
            _ => return None,
        },
        _ => return None,
    })
}

/// descriptor باینری را به متن خوانا با indent و توضیح تبدیل می‌کند.
#[allow(unused_assignments)]
pub fn format_descriptor(desc: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0usize;
    let mut depth: usize = 0;
    let mut last_usage_page: u32 = 0;
    let mut usage_page_stack: [u32; 4] = [0; 4];
    let mut sp = 0usize;

    while i < desc.len() {
        let start = i;
        let prefix = desc[i];
        i += 1;

        if prefix == 0xFE {
            if i + 1 >= desc.len() {
                break;
            }
            let dlen = desc[i] as usize;
            if i + 2 + dlen > desc.len() {
                break;
            }
            i += 2 + dlen;
            continue;
        }

        let size_code = (prefix & 0x03) as usize;
        let type_ = (prefix >> 2) & 0x03;
        let tag = (prefix >> 4) & 0x0F;
        let data_size = if size_code == 3 { 4 } else { size_code };

        let mut data: u32 = 0;
        for b in 0..data_size {
            if i < desc.len() {
                data |= (desc[i] as u32) << (8 * b);
                i += 1;
            }
        }

        // hex bytes
        let hex: String = desc[start..i]
            .iter()
            .map(|b| format!("0x{:02x}, ", b))
            .collect();
        let hex_padded = format!("{:<26}", hex);

        // indent
        let indent = "    ".repeat(depth + 1);

        let mut item_name: String = String::new();
        let mut extra: String = String::new();

        match type_ {
            0 => {
                // Main
                match tag {
                    0x8 => item_name = "INPUT".into(),
                    0x9 => item_name = "OUTPUT".into(),
                    0xB => item_name = "FEATURE".into(),
                    0xA => {
                        item_name = "COLLECTION".into();
                        extra = match data {
                            0x00 => "Physical".into(),
                            0x01 => "Application".into(),
                            0x02 => "Logical".into(),
                            0x03 => "Report".into(),
                            0x04 => "Named Array".into(),
                            0x05 => "Usage Switch".into(),
                            0x06 => "Usage Modifier".into(),
                            _ => String::new(),
                        };
                    }
                    0xC => item_name = "END_COLLECTION".into(),
                    _ => item_name = "MAIN".into(),
                }
            }
            1 => {
                // Global
                match tag {
                    0x0 => {
                        item_name = "USAGE_PAGE".into();
                        if let Some(n) = usage_page_name(data) {
                            extra = n.into();
                        } else if data >= 0xFF00 {
                            extra = format!("Vendor Defined (0x{:04X})", data);
                        }
                        last_usage_page = data;
                    }
                    0x1 => item_name = "LOGICAL_MINIMUM".into(),
                    0x2 => item_name = "LOGICAL_MAXIMUM".into(),
                    0x3 => item_name = "PHYSICAL_MINIMUM".into(),
                    0x4 => item_name = "PHYSICAL_MAXIMUM".into(),
                    0x5 => item_name = "UNIT_EXPONENT".into(),
                    0x6 => item_name = "UNIT".into(),
                    0x7 => item_name = "REPORT_SIZE".into(),
                    0x8 => item_name = "REPORT_ID".into(),
                    0x9 => item_name = "REPORT_COUNT".into(),
                    0xA => {
                        item_name = "PUSH".into();
                        if sp < usage_page_stack.len() {
                            usage_page_stack[sp] = last_usage_page;
                            sp += 1;
                        }
                    }
                    0xB => {
                        item_name = "POP".into();
                        if sp > 0 {
                            sp -= 1;
                            last_usage_page = usage_page_stack[sp];
                        }
                    }
                    _ => item_name = "GLOBAL".into(),
                }
            }
            2 => {
                // Local
                match tag {
                    0x0 => {
                        item_name = "USAGE".into();
                        if let Some(n) = usage_name(last_usage_page, data) {
                            extra = n.into();
                        }
                    }
                    0x1 => item_name = "USAGE_MINIMUM".into(),
                    0x2 => item_name = "USAGE_MAXIMUM".into(),
                    0x3 => item_name = "DESIGNATOR_INDEX".into(),
                    0x4 => item_name = "DESIGNATOR_MINIMUM".into(),
                    0x5 => item_name = "DESIGNATOR_MAXIMUM".into(),
                    0x7 => item_name = "STRING_INDEX".into(),
                    0x8 => item_name = "STRING_MINIMUM".into(),
                    0x9 => item_name = "STRING_MAXIMUM".into(),
                    0xA => item_name = "DELIMITER".into(),
                    _ => item_name = "LOCAL".into(),
                }
            }
            _ => item_name = "RESERVED".into(),
        }

        // END_COLLECTION قبل از چاپ، depth کم شود
        if type_ == 0 && tag == 0xC && depth > 0 {
            depth -= 1;
            let indent = "    ".repeat(depth + 1);
            out.push_str(&format!("{}{}// {}", indent, hex_padded, item_name));
        } else {
            out.push_str(&format!("{}{}// {}", indent, hex_padded, item_name));
        }

        if !extra.is_empty() {
            out.push_str(&format!(" ({})", extra));
        }
        out.push('\n');

        if type_ == 0 && tag == 0xA {
            depth += 1;
        }
    }

    out
}
