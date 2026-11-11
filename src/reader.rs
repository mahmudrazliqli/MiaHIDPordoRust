use hidapi::HidDevice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::app::MAX_READ_BUF;

#[derive(Debug)]
pub enum ReaderMsg {
    Data(Vec<u8>),
    Failed(String),
}

/// thread خواندن از دستگاه را شروع می‌کند.
/// - `running` و `paused` را از بیرون کنترل می‌کنیم.
/// - خروجی به `glib::Sender` می‌رود، که در thread اصلی GTK مصرف می‌شود.
pub fn start_reader(
    device: Arc<Mutex<Option<HidDevice>>>,
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    sender: glib::Sender<ReaderMsg>,
) -> thread::JoinHandle<()> {
    running.store(true, Ordering::SeqCst);

    thread::spawn(move || {
        let mut buf = [0u8; MAX_READ_BUF];

        while running.load(Ordering::Relaxed) {
            if paused.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(10));
                continue;
            }

            // خواندن داخل mutex کوتاه
            let result = {
                let guard = device.lock().unwrap();
                guard.as_ref().map(|d| d.read(&mut buf))
            };

            match result {
                Some(Ok(0)) => {
                    // بدون داده، کمی صبر
                    thread::sleep(Duration::from_micros(1000));
                }
                Some(Ok(n)) => {
                    let data = buf[..n].to_vec();
                    if sender.send(ReaderMsg::Data(data)).is_err() {
                        break;
                    }
                }
                Some(Err(e)) => {
                    running.store(false, Ordering::SeqCst);
                    let _ = sender.send(ReaderMsg::Failed(format!("{}", e)));
                    break;
                }
                None => break,
            }
        }
    })
}
