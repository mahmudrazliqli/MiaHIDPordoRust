use gtk::prelude::*;

use crate::app::{ReportSizes, MAX_LOG_LINES};

#[allow(dead_code)]
pub struct ReportTab {
    pub report_id: u8,
    pub has_report_id: bool,
    pub sizes: ReportSizes,

    pub input_view: gtk::ListBox,
    pub log_scroll: gtk::ScrolledWindow,
    pub log_rows: usize,
    pub autoscroll: bool,

    // ویجت‌های ورودی/دکمه‌ها (اختیاری — بسته به capability)
    pub output_entry: Option<gtk::Entry>,
    pub send_btn: Option<gtk::Button>,
    pub read_btn: Option<gtk::Button>,
    pub feature_entry: Option<gtk::Entry>,
    pub set_feat_btn: Option<gtk::Button>,
    pub get_feat_btn: Option<gtk::Button>,
}

pub fn build_tab(
    report_id: u8,
    has_report_id: bool,
    sizes: ReportSizes,
) -> (ReportTab, gtk::Widget) {
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 6);
    vbox.set_border_width(6);

    // terminal view
    let scrolled = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    scrolled.set_vexpand(true);
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.set_activate_on_single_click(false);
    list.style_context().add_class("log-view");
    scrolled.add(&list);
    vbox.pack_start(&scrolled, true, true, 0);

    // output / read
    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    hbox.set_border_width(4);

    let mut output_entry = None;
    let mut send_btn = None;
    if sizes.output_bytes > 0 {
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("output bytes"));
        hbox.pack_start(&entry, true, true, 0);
        let btn = gtk::Button::with_label(&format!("Send {} bytes", sizes.output_bytes));
        btn.set_size_request(140, -1);
        hbox.pack_start(&btn, false, false, 0);
        output_entry = Some(entry);
        send_btn = Some(btn);
    }

    let mut read_btn = None;
    if sizes.input_bytes > 0 {
        let btn = gtk::Button::with_label(&format!("Read {} bytes", sizes.input_bytes));
        btn.set_size_request(140, -1);
        hbox.pack_start(&btn, false, false, 0);
        read_btn = Some(btn);
    }

    // feature
    let hbox2 = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    hbox2.set_border_width(4);

    let mut feature_entry = None;
    let mut set_feat_btn = None;
    let mut get_feat_btn = None;
    if sizes.feature_bytes > 0 {
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("feature bytes"));
        hbox2.pack_start(&entry, true, true, 0);
        let set_btn = gtk::Button::with_label(&format!("Set {} bytes", sizes.feature_bytes));
        set_btn.set_size_request(140, -1);
        hbox2.pack_start(&set_btn, false, false, 0);
        let get_btn = gtk::Button::with_label(&format!("Get {} bytes", sizes.feature_bytes));
        get_btn.set_size_request(140, -1);
        hbox2.pack_start(&get_btn, false, false, 0);
        feature_entry = Some(entry);
        set_feat_btn = Some(set_btn);
        get_feat_btn = Some(get_btn);
    }

    vbox.pack_start(&hbox, false, false, 0);
    vbox.pack_start(&hbox2, false, false, 0);

    let tab = ReportTab {
        report_id,
        has_report_id,
        sizes,
        input_view: list,
        log_scroll: scrolled.clone(),
        log_rows: 0,
        autoscroll: true,
        output_entry,
        send_btn,
        read_btn,
        feature_entry,
        set_feat_btn,
        get_feat_btn,
    };

    (tab, vbox.upcast())
}

pub fn append_log(tab: &mut ReportTab, prefix: &str, meta: &str, payload: &str) {
    let header = if meta.is_empty() {
        format!("[{}]", prefix)
    } else {
        format!("[{}] {}", prefix, meta)
    };
    let markup = format!(
        "<span foreground='#1565C0' size='small'>{}</span>{}",
        glib::markup_escape_text(&header),
        glib::markup_escape_text(payload),
    );
    let label = gtk::Label::new(None);
    label.set_markup(&markup);
    label.set_selectable(true);
    label.set_xalign(0.0);
    label.set_line_wrap(true);
    label.set_line_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_margin_start(4);
    label.set_margin_end(4);
    label.show();
    tab.input_view.insert(&label, -1);
    tab.log_rows += 1;

    while tab.log_rows > MAX_LOG_LINES {
        if let Some(row) = tab.input_view.row_at_index(0) {
            tab.input_view.remove(&row);
            tab.log_rows -= 1;
        } else {
            break;
        }
    }

    if tab.autoscroll {
        if let Some(adj) = tab.log_scroll.vadjustment().into() {
            let adj: gtk::Adjustment = adj;
            adj.set_value(adj.upper() - adj.page_size());
        }
    }
}

pub fn log_raw(tab: &mut ReportTab, prefix: &str, meta: &str, payload: &[u8], hex: bool) {
    let disp = crate::device::format_bytes(payload, hex);
    append_log(tab, prefix, meta, &disp);
}

pub fn clear_log(tab: &mut ReportTab) {
    while let Some(row) = tab.input_view.row_at_index(0) {
        tab.input_view.remove(&row);
    }
    tab.log_rows = 0;
}
