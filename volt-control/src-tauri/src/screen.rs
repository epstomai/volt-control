//! Screen sleep (设置 → 闲置熄屏). The Volt has no sleep of its own, and of its
//! lights only the screen is reachable (`display_brightness`; the button LEDs
//! have no control anywhere in the UA stack). After `screen_sleep_min` minutes
//! without activity the brightness goes to 0; the next activity restores it.
//!
//! The brightness to restore lives in the config (`screen_dimmed_from`), so a
//! screen left dark by a crash or a quit is lit again on the next connect.
//! Pressing the device's own buttons that change nothing on the host (the
//! encoder's focus switch) can't be seen, so it doesn't wake the screen.

use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use crate::engine::{Origin, Shared};
use crate::model::Op;
use crate::uasrv;

pub const BRIGHTNESS: &str = "dev.brightness";

/// How often the idle time is checked.
pub const CHECK: Duration = Duration::from_secs(5);

impl Shared {
    /// Someone is at the desk: restart the idle clock and light the screen.
    pub fn activity(&self) {
        *self.idle.lock().unwrap() = Instant::now();
        self.wake_screen();
    }

    /// Restores the brightness saved when the screen went dark, if any.
    fn wake_screen(&self) {
        let Some(n) = self.config.lock().unwrap().screen_dimmed_from else { return };
        if self.model.lock().unwrap().is_none() {
            return; // offline: restore once connected
        }
        if let Err(e) = self.set_param(BRIGHTNESS, n, Origin::Ui) {
            log::warn!("screen wake: {e}");
            return;
        }
        self.config.lock().unwrap().screen_dimmed_from = None;
        self.save_config();
        log::info!("screen on (brightness {:.0})", n * 255.0);
    }

    /// Darkens the screen once the idle time is up (called every [`CHECK`]).
    pub fn check_idle(&self) {
        let (minutes, dark) = {
            let cfg = self.config.lock().unwrap();
            (cfg.screen_sleep_min, cfg.screen_dimmed_from.is_some())
        };
        if minutes == 0 || dark || self.idle.lock().unwrap().elapsed() < Duration::from_secs(u64::from(minutes) * 60) {
            return;
        }
        let Some(n) = self.param(BRIGHTNESS).map(|p| p.normalized).filter(|&n| n > 0.0) else {
            return; // already dark by the user's choice: nothing to restore later
        };
        self.config.lock().unwrap().screen_dimmed_from = Some(n);
        self.save_config();
        if let Err(e) = self.set_param(BRIGHTNESS, 0.0, Origin::Ui) {
            log::warn!("screen sleep: {e}");
            self.config.lock().unwrap().screen_dimmed_from = None;
            self.save_config();
            return;
        }
        log::info!("screen off after {minutes} idle minutes");
    }

    /// Restore at app exit. The async write queue no longer runs then, so this
    /// writes over a short-lived connection of its own.
    pub fn wake_screen_now(&self) {
        let Some(n) = self.config.lock().unwrap().screen_dimmed_from else { return };
        let op = self.model.lock().unwrap().as_mut().and_then(|m| m.plan_write(BRIGHTNESS, n).ok());
        let Some(Op::Set { path, arg }) = op else { return };
        let addr: SocketAddr = uasrv::DEFAULT_ADDR.parse().expect("valid address");
        let sent = TcpStream::connect_timeout(&addr, Duration::from_secs(1))
            .and_then(|mut s| s.write_all(format!("set {path} {arg}\0").as_bytes()).and_then(|_| s.flush()));
        match sent {
            Ok(()) => {
                self.config.lock().unwrap().screen_dimmed_from = None;
                self.save_config();
                log::info!("screen restored at exit");
            }
            Err(e) => log::warn!("screen restore at exit: {e}"), // the next start retries
        }
    }
}
