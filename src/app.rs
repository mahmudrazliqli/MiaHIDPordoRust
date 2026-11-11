use gtk::prelude::*;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use crate::report_tab::ReportTab;

pub const MAX_REPORT_ID: usize = 256;
pub const MAX_LOG_LINES: usize = 500;
pub const MAX_REPORT_BYTES: usize = 1024;
pub const MAX_READ_BUF: usize = MAX_REPORT_BYTES + 1;
pub const MAX_DESCRIPTOR_SIZE: usize = 4096;

#[derive(Default, Clone, Copy)]
pub struct ReportSizes {
    pub input_bytes: u32,
    pub output_bytes: u32,
    pub feature_bytes: u32,
    pub input_bits: u32,
    pub output_bits: u32,
    pub feature_bits: u32,
}

#[allow(dead_code)]
pub struct UiState {
    pub window: gtk::Window,
    pub status_label: gtk::Label,
    pub notebook: gtk::Notebook,
    pub device_combo: gtk::ComboBox,
    pub hex_check: gtk::CheckButton,
    pub auto_check: gtk::CheckButton,
    pub font_spin: gtk::SpinButton,
    pub connect_button: gtk::Button,
    pub refresh_button: gtk::Button,
    pub clear_button: gtk::Button,
    pub pause_button: gtk::Button,
    pub show_descriptor_button: gtk::Button,

    pub tabs: [Option<ReportTab>; MAX_REPORT_ID],
    pub has_report_id: bool,
    pub main_sizes: [ReportSizes; MAX_REPORT_ID],
    pub raw_descriptor: Vec<u8>,

    pub reader_running: Arc<AtomicBool>,
    pub reader_paused: Arc<AtomicBool>,
    pub hex_mode: Arc<AtomicBool>,

    // برای config
    pub cfg: crate::config::Config,
    pub pending_auto_connect: bool,
}

impl UiState {
    pub fn clear_tabs(&mut self) {
        let n = self.notebook.n_pages();
        for i in (0..n).rev() {
            self.notebook.remove_page(Some(i));
        }
        self.tabs = std::array::from_fn(|_| None);
        self.has_report_id = false;
        self.main_sizes = [ReportSizes::default(); MAX_REPORT_ID];
        self.raw_descriptor.clear();
    }

    pub fn set_status(&self, msg: &str) {
        self.status_label.set_text(msg);
    }
}
