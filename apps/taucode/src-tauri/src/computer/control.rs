use super::policy::Context;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Mutex,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Key(u16, bool),
    Text(String),
    Mouse {
        point: [f64; 2],
        button: u8,
        down: bool,
        clicks: u32,
    },
    Move([f64; 2], bool),
    Scroll(i32, bool),
}
pub trait Input: Send {
    fn post(&mut self, event: &Event) -> Result<(), String>;
    fn stop(&mut self) {}
    fn end_action(&mut self, _cancelled: bool) -> Option<Box<dyn FnOnce() + Send>> {
        None
    }
}
struct Held {
    input: Box<dyn Input>,
    keys: Vec<u16>,
    buttons: Vec<(u8, [f64; 2])>,
}
pub struct Control {
    stopped: AtomicBool,
    epoch: AtomicU64,
    pub paused: AtomicBool,
    held: Mutex<Held>,
}
impl Control {
    pub fn replace_input(&self, input: impl Input + 'static) {
        self.stop();
        self.held.lock().unwrap().input = Box::new(input);
    }
    pub fn install_input(&self, input: impl Input + 'static) -> Result<(), String> {
        let mut held = self.held.lock().map_err(|_| "Input lock poisoned")?;
        if self.stopped() {
            return Err("Stopped".into());
        }
        held.input.stop();
        held.input = Box::new(input);
        Ok(())
    }
    pub fn guarded<T>(
        &self,
        expected: &Context,
        context: impl FnOnce() -> Result<Context, String>,
        action: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _held = self.held.lock().map_err(|_| "Input lock poisoned")?;
        if self.stopped() || self.paused.load(Ordering::SeqCst) {
            return Err("Run stopped or paused".into());
        }
        if context()? != *expected {
            return Err("Focus or secure-field state changed; action cancelled".into());
        }
        action()
    }
    pub fn new(input: impl Input + 'static) -> Self {
        Self {
            stopped: AtomicBool::new(true),
            epoch: AtomicU64::new(0),
            paused: AtomicBool::new(false),
            held: Mutex::new(Held {
                input: Box::new(input),
                keys: vec![],
                buttons: vec![],
            }),
        }
    }
    pub fn start(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.release();
        self.paused.store(false, Ordering::SeqCst);
        self.stopped.store(false, Ordering::SeqCst);
    }
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }
    pub fn pause(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        if paused {
            self.epoch.fetch_add(1, Ordering::SeqCst);
            self.release_with(true);
        }
    }
    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
    pub fn post(
        &self,
        event: Event,
        expected: &Context,
        context: impl FnOnce() -> Result<Context, String>,
    ) -> Result<(), String> {
        if context()? != *expected {
            return Err("Focus or secure-field state changed; action cancelled".into());
        }
        let mut held = self.held.lock().map_err(|_| "Input lock poisoned")?;
        if self.stopped() || self.paused.load(Ordering::SeqCst) {
            return Err("Run stopped or paused".into());
        }
        // Record before posting, so stop also releases inputs after a partially failed post.
        match &event {
            Event::Key(k, true) => {
                if !held.keys.contains(k) {
                    held.keys.push(*k);
                }
            }
            Event::Mouse {
                button,
                point,
                down: true,
                ..
            } => held.buttons.push((*button, *point)),
            Event::Move(point, _) => {
                for (_, p) in &mut held.buttons {
                    *p = *point;
                }
            }
            _ => (),
        }
        held.input.post(&event)?;
        match &event {
            Event::Key(k, false) => held.keys.retain(|v| v != k),
            Event::Mouse {
                button,
                down: false,
                ..
            } => held.buttons.retain(|(b, _)| b != button),
            _ => (),
        }
        Ok(())
    }
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.release_with(true);
        self.held
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .input
            .stop();
    }
    pub fn release(&self) {
        self.release_with(false);
    }
    fn release_with(&self, cancelled: bool) {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        for key in std::mem::take(&mut held.keys).into_iter().rev() {
            let _ = held.input.post(&Event::Key(key, false));
        }
        for (button, point) in std::mem::take(&mut held.buttons) {
            let _ = held.input.post(&Event::Mouse {
                point,
                button,
                down: false,
                clicks: 1,
            });
        }
        let finish = held.input.end_action(cancelled);
        drop(held); // Focus settle/restore must never block the kill-switch input lock.
        if let Some(finish) = finish {
            finish();
        }
    }
    pub async fn checkpoint(&self) -> Result<(), String> {
        loop {
            if self.stopped() {
                return Err("Stopped".into());
            }
            if !self.paused.load(Ordering::SeqCst) {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }
    pub async fn until<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        tokio::pin!(future);
        loop {
            tokio::select! { result=&mut future=>return result, _=tokio::time::sleep(std::time::Duration::from_millis(20))=> {if self.stopped(){return Err("Stopped".into());}} }
        }
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        self.stop();
    }
}
