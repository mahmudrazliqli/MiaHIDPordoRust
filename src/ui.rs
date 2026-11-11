#![allow(deprecated)]
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::app::{ReportSizes, UiState, MAX_DESCRIPTOR_SIZE, MAX_REPORT_ID};
use crate::config;
use crate::descriptor::{format_descriptor, parse_report_descriptor};
use crate::device::{enumerate, format_bytes, parse_input, DeviceEntry};
use crate::reader::{start_reader, ReaderMsg};
use crate::report_tab::{append_log, build_tab, clear_log, log_raw};
use crate::sync_op::{execute, SyncKind, SyncRequest, SyncResult};

pub struct AppUi {
    pub widgets: UiState,
    pub api: hidapi::HidApi,
    pub devices: Vec<DeviceEntry>,
    pub hid_device: Arc<Mutex<Option<hidapi::HidDevice>>>,
    pub reader_handle: Option<JoinHandle<()>>,
}

impl AppUi {
    pub fn is_connected(&self) -> bool {
        self.hid_device.lock().unwrap().is_some()
    }
}

pub fn build_ui(app: &gtk::Application) {
    let builder = gtk::Builder::from_resource("/org/miahidpordo/window1.glade");

    let window: gtk::Window = builder.object("main_window").unwrap();

    // عنوان پنجره با نسخه
    const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
    let title = format!("Mia HID Pordo {}", APP_VERSION);
    window.set_title(&title);
    let status_label: gtk::Label = builder.object("status_label").unwrap();
    let notebook: gtk::Notebook = builder.object("report_notebook").unwrap();
    let device_combo: gtk::ComboBox = builder.object("device_combo").unwrap();
    let hex_check: gtk::CheckButton = builder.object("hex_check").unwrap();
    let auto_check: gtk::CheckButton = builder.object("check_autoconnect").unwrap();
    let font_spin: gtk::SpinButton = builder.object("font_size_spin").unwrap();
    let connect_button: gtk::Button = builder.object("connect_button").unwrap();
    let refresh_button: gtk::Button = builder.object("refresh_button").unwrap();
    let clear_button: gtk::Button = builder.object("clear_button").unwrap();
    let pause_button: gtk::Button = builder.object("pause_button").unwrap();
    let show_descriptor_button: gtk::Button = builder.object("show_descriptor_button").unwrap();

    let rend = gtk::CellRendererText::new();
    let layout = device_combo.clone().upcast::<gtk::CellLayout>();
    layout.pack_start(&rend, true);
    layout.add_attribute(&rend, "text", 0);

    let css = gtk::CssProvider::new();
    css.load_from_resource("/org/miahidpordo/style.css");
    if let Some(screen) = gtk::gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let widgets = UiState {
        window: window.clone(),
        status_label,
        notebook,
        device_combo,
        hex_check: hex_check.clone(),
        auto_check: auto_check.clone(),
        font_spin: font_spin.clone(),
        connect_button: connect_button.clone(),
        refresh_button: refresh_button.clone(),
        clear_button: clear_button.clone(),
        pause_button: pause_button.clone(),
        show_descriptor_button: show_descriptor_button.clone(),
        tabs: std::array::from_fn(|_| None),
        has_report_id: false,
        main_sizes: [ReportSizes::default(); MAX_REPORT_ID],
        raw_descriptor: Vec::new(),
        reader_running: Arc::new(AtomicBool::new(false)),
        reader_paused: Arc::new(AtomicBool::new(false)),
        hex_mode: Arc::new(AtomicBool::new(false)),
        cfg: crate::config::Config::default(),
        pending_auto_connect: false,
    };

    let api = hidapi::HidApi::new().expect("Failed to init hidapi");

    let state = Rc::new(RefCell::new(AppUi {
        widgets,
        api,
        devices: Vec::new(),
        hid_device: Arc::new(Mutex::new(None)),
        reader_handle: None,
    }));

    // ---- Load config ----
    load_and_apply_config(&state, &window, &hex_check, &auto_check, &font_spin);

    // ---- Refresh ----
    refresh_button.connect_clicked({
        let state = state.clone();
        move |_| refresh_device_list(&state)
    });

    // ---- Connect/Disconnect ----
    connect_button.connect_clicked({
        let state = state.clone();
        move |_| {
            let connected = state.borrow().is_connected();
            if connected {
                disconnect(&state);
            } else {
                connect(&state);
            }
        }
    });

    // ---- HEX ----
    hex_check.connect_toggled({
        let state = state.clone();
        move |chk| {
            let hex = chk.is_active();
            let s = state.borrow();
            s.widgets.hex_mode.store(hex, Ordering::SeqCst);
            let label = if hex { "Mode: HEX" } else { "Mode: ASCII" };
            s.widgets.set_status(label);
        }
    });

    // ---- Pause ----
    pause_button.connect_clicked({
        let state = state.clone();
        move |_| {
            let s = state.borrow();
            if !s.is_connected() {
                s.widgets.set_status("Not connected — nothing to pause");
                return;
            }
            let new_paused = !s.widgets.reader_paused.load(Ordering::SeqCst);
            s.widgets.reader_paused.store(new_paused, Ordering::SeqCst);

            if let Some(img) = s
                .widgets
                .pause_button
                .child()
                .and_then(|c| c.downcast::<gtk::Image>().ok())
            {
                let name = if new_paused {
                    "media-playback-start"
                } else {
                    "media-playback-pause"
                };
                img.set_from_icon_name(Some(name), gtk::IconSize::Button);
            }
            let label = if new_paused { "Input paused" } else { "Input resumed" };
            s.widgets.set_status(label);
        }
    });

    // ---- Clear ----
    clear_button.connect_clicked({
        let state = state.clone();
        move |_| {
            let mut s = state.borrow_mut();
            for slot in s.widgets.tabs.iter_mut() {
                if let Some(tab) = slot.as_mut() {
                    clear_log(tab);
                }
            }
            s.widgets.set_status("Logs cleared");
        }
    });

    // ---- Show Descriptor ----
    show_descriptor_button.connect_clicked({
        let state = state.clone();
        move |_| show_descriptor(&state)
    });

    refresh_device_list(&state);
    // ---- Save config on close ----
    {
        let state = state.clone();
        window.connect_delete_event(move |_, _| {
            let connected = {
                let s = state.borrow();
                s.is_connected()
            }; // borrow اینجا آزاد می‌شود
            if connected {
                disconnect(&state);
            }
            save_config_from_state(&state);
            glib::Propagation::Proceed
        });
    }

    window.set_application(Some(app));
    window.show_all();



    // ---- Auto-connect (بعد از show) ----
    {
        let state = state.clone();
        glib::idle_add_local(move || {
            let (pending, last_id) = {
                let s = state.borrow();
                (s.widgets.pending_auto_connect, s.widgets.cfg.last_device.clone())
            };
            if pending && !last_id.is_empty() {
                // دستگاه را در combo پیدا کن
                let idx = {
                    let s = state.borrow();
                    s.devices.iter().position(|d| d.id == last_id)
                };
                if let Some(i) = idx {
                    state.borrow().widgets.device_combo.set_active(Some(i as u32));
                    connect(&state);
                }
            }
            glib::ControlFlow::Break
        });
    }
}

fn refresh_device_list(state: &Rc<RefCell<AppUi>>) {
    let (entries, new_sel) = {
        let s = state.borrow();
        if s.is_connected() {
            return;
        }
        let saved_id: Option<String> = s
            .widgets
            .device_combo
            .active()
            .and_then(|i| s.devices.get(i as usize).map(|d| d.id.clone()));
        enumerate(&s.api, saved_id.as_deref())
    };

    let store = gtk::ListStore::new(&[glib::Type::STRING]);
    for e in &entries {
        let iter = store.append();
        store.set_value(&iter, 0, &e.label.to_value());
    }

    let s = state.borrow();
    s.widgets.device_combo.set_model(Some(&store));
    if let Some(i) = new_sel {
        s.widgets.device_combo.set_active(Some(i as u32));
    } else if !entries.is_empty() {
        s.widgets.device_combo.set_active(Some(0));
    }

    let count = entries.len();
    if count > 0 {
        s.widgets.set_status(&format!("Found {} device(s)", count));
    } else {
        s.widgets.set_status("No HID devices found");
    }
    s.widgets.connect_button.set_sensitive(count > 0);
    drop(s);

    state.borrow_mut().devices = entries;
}

#[allow(clippy::needless_range_loop)]
fn connect(state: &Rc<RefCell<AppUi>>) {
    let path = {
        let s = state.borrow();
        let Some(i) = s.widgets.device_combo.active() else {
            s.widgets.set_status("Select a device first");
            return;
        };
        match s.devices.get(i as usize) {
            Some(d) => d.path.clone(),
            None => {
                s.widgets.set_status("Invalid selection");
                return;
            }
        }
    };

    let cpath = match std::ffi::CString::new(path) {
        Ok(c) => c,
        Err(_) => {
            state.borrow().widgets.set_status("Invalid device path");
            return;
        }
    };

    let device = {
        let s = state.borrow();
        match s.api.open_path(&cpath) {
            Ok(d) => d,
            Err(e) => {
                s.widgets.set_status(&format!("Failed to open device: {}", e));
                return;
            }
        }
    };
    device.set_blocking_mode(false).ok();

    let mut desc_buf = [0u8; MAX_DESCRIPTOR_SIZE];
    let n = match device.get_report_descriptor(&mut desc_buf) {
        Ok(n) => n,
        Err(e) => {
            state
                .borrow()
                .widgets
                .set_status(&format!("Failed to read Report Descriptor: {}", e));
            return;
        }
    };
    if n == 0 {
        state.borrow().widgets.set_status("Empty Report Descriptor");
        return;
    }

    let desc = desc_buf[..n].to_vec();
    let (sizes, has_report_id) = parse_report_descriptor(&desc);

    let mut s = state.borrow_mut();
    s.widgets.clear_tabs();
    s.widgets.has_report_id = has_report_id;
    s.widgets.main_sizes = sizes;
    s.widgets.raw_descriptor = desc.clone();

    let mut tab_count = 0usize;
    let mut has_input = false;

    for rid in 0..MAX_REPORT_ID {
        let hidsizes = sizes[rid];
        if hidsizes.input_bytes == 0
            && hidsizes.output_bytes == 0
            && hidsizes.feature_bytes == 0
        {
            continue;
        }
        if hidsizes.input_bytes > 0 {
            has_input = true;
        }

        let title = if has_report_id {
            format!("Report ID:0x{:02X}", rid)
        } else {
            "Default".to_string()
        };

        let (tab, widget) = build_tab(rid as u8, has_report_id, hidsizes);

        // Ctrl+Scroll روی log view برای تغییر font size
        {
            let font_spin = s.widgets.font_spin.clone();
            let log_scroll = tab.log_scroll.clone();
            log_scroll.add_events(gtk::gdk::EventMask::SCROLL_MASK);
            log_scroll.connect_scroll_event(move |_, ev| {
                let state = ev.state();
                if !state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                    return glib::Propagation::Proceed;
                }
                let delta = match ev.direction() {
                    gtk::gdk::ScrollDirection::Up => 1,
                    gtk::gdk::ScrollDirection::Down => -1,
                    _ => return glib::Propagation::Proceed,
                };
                let cur = font_spin.value_as_int();
                let new = (cur + delta).clamp(6, 48);
                if new != cur {
                    font_spin.set_value(new as f64);
                }
                glib::Propagation::Stop
            });
        }

        let label = gtk::Label::new(Some(&title));
        let page = s.widgets.notebook.append_page(&widget, Some(&label));
        s.widgets.notebook.set_current_page(Some(page));
        widget.show_all();

        // ---- وصل کردن callbackها ----
        if let Some(btn) = tab.send_btn.clone() {
            connect_send_cb(&btn, state, rid as u8);
        }
        if let Some(btn) = tab.set_feat_btn.clone() {
            connect_feature_set_cb(&btn, state, rid as u8);
        }
        if let Some(btn) = tab.get_feat_btn.clone() {
            connect_feature_get_cb(&btn, state, rid as u8);
        }
        if let Some(btn) = tab.read_btn.clone() {
            connect_read_cb(&btn, state, rid as u8);
        }

        s.widgets.tabs[rid] = Some(tab);
        tab_count += 1;
    }

    *s.hid_device.lock().unwrap() = Some(device);
    s.widgets.connect_button.set_label("Disconnect");
    s.widgets.device_combo.set_sensitive(false);
    s.widgets.refresh_button.set_sensitive(false);

    let msg = format!(
        "Connected. Descriptor: {} bytes, {} tab(s){}",
        n,
        tab_count,
        if has_input { "" } else { " (no Input)" }
    );
    s.widgets.set_status(&msg);
    drop(s);

    if has_input {
        start_reader_thread(state);
    }
}

// ============================================================================
// Send Output
// ============================================================================
fn connect_send_cb(btn: &gtk::Button, state: &Rc<RefCell<AppUi>>, rid: u8) {
    let state = state.clone();
    btn.connect_clicked(move |_| {
        let (text, max, hex, has_rid, target) = {
            let s = state.borrow();
            let tab = match s.widgets.tabs[rid as usize].as_ref() {
                Some(t) => t,
                None => return,
            };
            let text = tab.output_entry.as_ref()
                .map(|e| e.text().to_string()).unwrap_or_default();
            let max = tab.sizes.output_bytes.min(crate::app::MAX_REPORT_BYTES as u32) as usize;
            let hex = s.widgets.hex_mode.load(Ordering::SeqCst);
            (text, max, hex, tab.has_report_id, max)
        };

        let mut payload = parse_input(&text, max, hex);
        if payload.is_empty() {
            log_to_tab(&state, rid, "OUT ", "(empty)", "");
            return;
        }
        if payload.len() < target {
            let warn = format!("WARNING: {} bytes < {} bytes (padding with 0)", payload.len(), target);
            log_to_tab(&state, rid, "OUT ", &warn, "");
            payload.resize(target, 0);
        }

        run_sync_op(
            &state,
            SyncRequest {
                kind: SyncKind::Output,
                report_id: rid,
                has_report_id: has_rid,
                payload,
                read_len: 0,
                rid_key: rid,
            },
        );
    });
}

// ============================================================================
// Feature Set
// ============================================================================
fn connect_feature_set_cb(btn: &gtk::Button, state: &Rc<RefCell<AppUi>>, rid: u8) {
    let state = state.clone();
    btn.connect_clicked(move |_| {
        let (text, max, hex, has_rid) = {
            let s = state.borrow();
            let tab = match s.widgets.tabs[rid as usize].as_ref() {
                Some(t) => t,
                None => return,
            };
            let text = tab.feature_entry.as_ref()
                .map(|e| e.text().to_string()).unwrap_or_default();
            let max = tab.sizes.feature_bytes.min(crate::app::MAX_REPORT_BYTES as u32) as usize;
            let hex = s.widgets.hex_mode.load(Ordering::SeqCst);
            (text, max, hex, tab.has_report_id)
        };

        let mut payload = parse_input(&text, max, hex);
        if payload.is_empty() {
            log_to_tab(&state, rid, "FEAT", "(empty)", "");
            return;
        }
        if payload.len() < max {
            let warn = format!("WARNING: {} bytes < {} bytes (padding with 0)", payload.len(), max);
            log_to_tab(&state, rid, "FEAT", &warn, "");
            payload.resize(max, 0);
        }

        run_sync_op(
            &state,
            SyncRequest {
                kind: SyncKind::FeatureSet,
                report_id: rid,
                has_report_id: has_rid,
                payload,
                read_len: 0,
                rid_key: rid,
            },
        );
    });
}

// ============================================================================
// Feature Get
// ============================================================================
fn connect_feature_get_cb(btn: &gtk::Button, state: &Rc<RefCell<AppUi>>, rid: u8) {
    let state = state.clone();
    btn.connect_clicked(move |_| {
        let (read_len, has_rid) = {
            let s = state.borrow();
            let tab = match s.widgets.tabs[rid as usize].as_ref() {
                Some(t) => t,
                None => return,
            };
            let n = tab.sizes.feature_bytes.min(crate::app::MAX_REPORT_BYTES as u32) as usize;
            (n, tab.has_report_id)
        };
        run_sync_op(
            &state,
            SyncRequest {
                kind: SyncKind::FeatureGet,
                report_id: rid,
                has_report_id: has_rid,
                payload: Vec::new(),
                read_len,
                rid_key: rid,
            },
        );
    });
}

// ============================================================================
// Manual Read Input
// ============================================================================
fn connect_read_cb(btn: &gtk::Button, state: &Rc<RefCell<AppUi>>, rid: u8) {
    let state = state.clone();
    btn.connect_clicked(move |_| {
        let (read_len, has_rid) = {
            let s = state.borrow();
            let tab = match s.widgets.tabs[rid as usize].as_ref() {
                Some(t) => t,
                None => return,
            };
            let n = tab.sizes.input_bytes.min(crate::app::MAX_REPORT_BYTES as u32) as usize;
            (n, tab.has_report_id)
        };

        let device = {
            let s = state.borrow();
            s.hid_device.clone()
        };

        // کانال: نتیجه به thread اصلی برمی‌گردد
        let (sender, receiver) =
            glib::MainContext::channel::<(Vec<u8>, String)>(glib::Priority::DEFAULT);

        let state_clone = state.clone();
        receiver.attach(None, move |(result, err)| {
            let hex = {
                let s = state_clone.borrow();
                s.widgets.hex_mode.load(Ordering::SeqCst)
            };
            if !err.is_empty() {
                log_to_tab(
                    &state_clone,
                    rid,
                    "READ",
                    &format!("failed: {}", err),
                    "",
                );
            } else if result.is_empty() {
                log_to_tab(&state_clone, rid, "READ", "timeout (no data)", "");
            } else {
                let meta = format!("RECV {:2} bytes: ", result.len());
                log_to_tab_raw(&state_clone, rid, "READ", &meta, &result, hex);
            }
            glib::ControlFlow::Continue
        });

        std::thread::spawn(move || {
            let mut result = Vec::new();
            let mut err = String::new();
            {
                let guard = device.lock().unwrap();
                if let Some(dev) = guard.as_ref() {
                    let total = read_len + 1;
                    let mut buf = vec![0u8; total];
                    buf[0] = if has_rid { rid } else { 0 };
                    match dev.get_input_report(&mut buf) {
                        Ok(n) if n > 1 => result = buf[1..n].to_vec(),
                        Ok(_) => {}
                        Err(e) => err = format!("{}", e),
                    }
                } else {
                    err = "device not available".into();
                }
            }
            let _ = sender.send((result, err));
        });
    });
}

// ============================================================================
// اجرای SyncOp در thread جدا + برگشت به UI
// ============================================================================
fn run_sync_op(state: &Rc<RefCell<AppUi>>, req: SyncRequest) {
    let device = {
        let s = state.borrow();
        s.hid_device.clone()
    };
    let (sender, receiver) = glib::MainContext::channel(glib::Priority::DEFAULT);
    let state_clone = state.clone();

    receiver.attach(None, move |res: SyncResult| {
        handle_sync_result(&state_clone, res);
        glib::ControlFlow::Continue
    });

    std::thread::spawn(move || {
        let res = execute(&device, &req);
        let _ = sender.send(res);
    });
}

fn handle_sync_result(state: &Rc<RefCell<AppUi>>, res: SyncResult) {
    let rid = res.rid_key;
    let hex = state.borrow().widgets.hex_mode.load(Ordering::SeqCst);

    let (tab_exists, expect_output) = {
        let s = state.borrow();
        match s.widgets.tabs[rid as usize].as_ref() {
            Some(t) => (true, t.sizes.output_bytes as usize + 1),
            None => (false, 0),
        }
    };
    if !tab_exists {
        return;
    }

    match res.kind {
        SyncKind::Output => {
            if !res.ok {
                log_to_tab(
                    state,
                    rid,
                    "OUT ",
                    &format!("hid_write failed: {}", res.err),
                    "",
                );
            } else if res.bytes != expect_output {
                log_to_tab(
                    state,
                    rid,
                    "OUT ",
                    &format!(
                        "hid_write short: {} of {} bytes",
                        res.bytes, expect_output
                    ),
                    "",
                );
            } else {
                log_to_tab(state, rid, "OUT ", "SENT ok", "");
            }
        }
        SyncKind::FeatureSet => {
            if !res.ok {
                log_to_tab(
                    state,
                    rid,
                    "FEAT",
                    &format!("send_feature_report failed: {}", res.err),
                    "",
                );
            } else {
                log_to_tab(state, rid, "FEAT", "SET ok", "");
            }
        }
        SyncKind::FeatureGet => {
            if !res.ok {
                log_to_tab(
                    state,
                    rid,
                    "FEAT",
                    &format!("get_feature_report failed: {}", res.err),
                    "",
                );
            } else {
                let meta = format!("GET  {:2} bytes: ", res.payload.len());
                log_to_tab_raw(state, rid, "FEAT", &meta, &res.payload, hex);
            }
        }
    }
}

fn show_descriptor(state: &Rc<RefCell<AppUi>>) {
    let (desc, rid) = {
        let s = state.borrow();
        if s.widgets.raw_descriptor.is_empty() {
            s.widgets.set_status("No descriptor available (connect first)");
            return;
        }
        let rid = s
            .widgets
            .notebook
            .current_page()
            .map(|p| p as usize)
            .unwrap_or(0);
        (s.widgets.raw_descriptor.clone(), rid)
    };

    // tab فعال یا اولین tab
    let target_rid = {
        let s = state.borrow();
        let mut found: Option<u8> = None;
        for (i, slot) in s.widgets.tabs.iter().enumerate() {
            if slot.is_some() {
                if found.is_none() {
                    found = Some(i as u8);
                }
                if i == rid {
                    found = Some(i as u8);
                    break;
                }
            }
        }
        found
    };
    let Some(target) = target_rid else {
        return;
    };

    let text = format_descriptor(&desc);

    let mut s = state.borrow_mut();
    if let Some(tab) = s.widgets.tabs[target as usize].as_mut() {
        append_log(
            tab,
            "DESC",
            &format!("HID Report Descriptor {} bytes", desc.len()),
            "",
        );
        // هر خط را جدا append می‌کنیم (چون markup escape لازم است)
        for line in text.lines() {
            append_log(tab, "DESC", "", line);
        }
    }
    s.widgets.set_status("HID Report Descriptor logged");
}

// ============================================================================
// Reader (همان قبلی)
// ============================================================================
fn start_reader_thread(state: &Rc<RefCell<AppUi>>) {
    let (device, running, paused) = {
        let s = state.borrow();
        (
            s.hid_device.clone(),
            s.widgets.reader_running.clone(),
            s.widgets.reader_paused.clone(),
        )
    };

    let (sender, receiver) = glib::MainContext::channel(glib::Priority::DEFAULT);
    let state_clone = state.clone();
    receiver.attach(None, move |msg| {
        handle_reader_msg(&state_clone, msg);
        glib::ControlFlow::Continue
    });

    let handle = start_reader(device, running, paused, sender);
    state.borrow_mut().reader_handle = Some(handle);
}

fn handle_reader_msg(state: &Rc<RefCell<AppUi>>, msg: ReaderMsg) {
    let mut s = state.borrow_mut();
    if !s.is_connected() {
        return;
    }
    match msg {
        ReaderMsg::Data(data) => {
            if data.is_empty() {
                return;
            }
            let (rid, payload) = if s.widgets.has_report_id && data.len() > 1 {
                (data[0], &data[1..])
            } else {
                (0u8, &data[..])
            };
            let hex = s.widgets.hex_mode.load(Ordering::SeqCst);
            let n = payload.len();
            let meta = format!("RECV {:2} bytes: ", n);
            if let Some(tab) = s.widgets.tabs[rid as usize].as_mut() {
                log_raw(tab, "IN  ", &meta, payload, hex);
            }
        }
        ReaderMsg::Failed(err) => {
            s.widgets.set_status(&format!("Reader stopped: {}", err));
            s.widgets.reader_running.store(false, Ordering::SeqCst);
            *s.hid_device.lock().unwrap() = None;
            s.reader_handle = None;
        }
    }
}

fn log_to_tab(state: &Rc<RefCell<AppUi>>, rid: u8, prefix: &str, meta: &str, payload: &str) {
    let mut s = state.borrow_mut();
    if let Some(tab) = s.widgets.tabs[rid as usize].as_mut() {
        append_log(tab, prefix, meta, payload);
    }
}

fn log_to_tab_raw(
    state: &Rc<RefCell<AppUi>>,
    rid: u8,
    prefix: &str,
    meta: &str,
    payload: &[u8],
    hex: bool,
) {
    let mut s = state.borrow_mut();
    if let Some(tab) = s.widgets.tabs[rid as usize].as_mut() {
        let disp = format_bytes(payload, hex);
        append_log(tab, prefix, meta, &disp);
    }
}

fn disconnect(state: &Rc<RefCell<AppUi>>) {
    {
        let s = state.borrow();
        s.widgets.reader_running.store(false, Ordering::SeqCst);
        s.widgets.reader_paused.store(false, Ordering::SeqCst);
        *s.hid_device.lock().unwrap() = None;
    }
    if let Some(h) = state.borrow_mut().reader_handle.take() {
        let _ = h.join();
    }
    let mut s = state.borrow_mut();
    s.widgets.clear_tabs();
    s.widgets.set_status("Disconnected");
    s.widgets.connect_button.set_label("Connect");
    s.widgets.device_combo.set_sensitive(true);
    s.widgets.refresh_button.set_sensitive(true);
    drop(s);

    refresh_device_list(state);
}

// ============================================================================
// Config: load/save + apply
// ============================================================================
fn load_and_apply_config(
    state: &Rc<RefCell<AppUi>>,
    window: &gtk::Window,
    hex_check: &gtk::CheckButton,
    auto_check: &gtk::CheckButton,
    font_spin: &gtk::SpinButton,
) {
    let cfg = config::load();

    // اعمال hex_mode
    hex_check.set_active(cfg.hex_mode);
    state
        .borrow()
        .widgets
        .hex_mode
        .store(cfg.hex_mode, Ordering::SeqCst);

    // autoconnect
    auto_check.set_active(cfg.autoconnect);

    // font size
    let fs = cfg.log_font_size.clamp(6, 48);
    font_spin.set_value(fs as f64);
    apply_log_font_size(fs);

    // window size
    if cfg.window_width > 0 && cfg.window_height > 0 {
        window.resize(cfg.window_width, cfg.window_height);
    }
    // window position (فقط X11، نه Wayland)
    if !is_wayland() && cfg.window_x >= 0 && cfg.window_y >= 0 {
        window.move_(cfg.window_x, cfg.window_y);
    }

    // ذخیره در state برای استفاده بعدی
    {
        let mut s = state.borrow_mut();
        s.widgets.cfg = cfg.clone();
        s.widgets.pending_auto_connect = cfg.autoconnect && !cfg.last_device.is_empty();
    }

    // callback font spin
    font_spin.connect_value_changed({
        let state = state.clone();
        move |spin| {
            let size = spin.value_as_int().clamp(6, 48);
            apply_log_font_size(size);
            state.borrow_mut().widgets.cfg.log_font_size = size;
        }
    });

    // Ctrl+Scroll روی log view: مرحله بعد داخل build_tab وصل می‌شود
    // اینجا فقط spin را تنظیم کردیم.
}

fn save_config_from_state(state: &Rc<RefCell<AppUi>>) {
    let (mut cfg, win) = {
        let s = state.borrow();
        (s.widgets.cfg.clone(), s.widgets.window.clone())
    };

    cfg.hex_mode = state.borrow().widgets.hex_mode.load(Ordering::SeqCst);
    cfg.autoconnect = state
        .borrow()
        .widgets
        .auto_check
        .is_active();

    cfg.log_font_size = state
        .borrow()
        .widgets
        .font_spin
        .value_as_int()
        .clamp(6, 48);

    let (w, h) = win.size();
    cfg.window_width = w;
    cfg.window_height = h;
    if !is_wayland() {
        let pos = win.position();
        cfg.window_x = pos.0;
        cfg.window_y = pos.1;
    }

    // last_device از combo
    {
        let s = state.borrow();
        if let Some(i) = s.widgets.device_combo.active() {
            if let Some(d) = s.devices.get(i as usize) {
                cfg.last_device = d.id.clone();
            }
        }
    }

    config::save(&cfg);
    state.borrow_mut().widgets.cfg = cfg;
}

fn is_wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE")
        .map(|s| s.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
        || std::env::var("WAYLAND_DISPLAY").is_ok()
}

// ============================================================================
// Font size CSS provider
// ============================================================================
fn apply_log_font_size(size: i32) {
    let size = size.clamp(6, 48);
    let css = gtk::CssProvider::new();
    let data = format!(
        "list.log-view label {{ font-family: monospace; font-size: {}pt; }}",
        size
    );
    let _ = css.load_from_data(data.as_bytes());
    if let Some(screen) = gtk::gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
    }
}

