use rdev::{listen, EventType, Key, Button};
use std::thread;
use std::sync::mpsc::Sender;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    Start,
    Stop,
    Toggle,
}

pub struct InputListener {
    // Config flags to enable/disable specific triggers
    pub enable_mouse: Arc<AtomicBool>,
    pub enable_hold: Arc<AtomicBool>,
    pub enable_toggle: Arc<AtomicBool>,
}

impl InputListener {
    pub fn new() -> Self {
        Self {
            enable_mouse: Arc::new(AtomicBool::new(true)),
            enable_hold: Arc::new(AtomicBool::new(true)),
            enable_toggle: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn start(&self, tx: Sender<InputEvent>) {
        let enable_mouse = self.enable_mouse.clone();
        let enable_hold = self.enable_hold.clone();
        let enable_toggle = self.enable_toggle.clone();

        thread::spawn(move || {
            let mut is_ctrl = false;
            let mut is_win = false;
            let mut combo_active = false;

            if let Err(error) = listen(move |event| {
                match event.event_type {
                    // Mouse Mode
                    EventType::ButtonPress(Button::Middle) => {
                        if enable_mouse.load(Ordering::Relaxed) {
                            tx.send(InputEvent::Start).ok();
                        }
                    },
                    EventType::ButtonRelease(Button::Middle) => {
                        if enable_mouse.load(Ordering::Relaxed) {
                            tx.send(InputEvent::Stop).ok();
                        }
                    },
                    
                    // Toggle Mode (Right Alt)
                    EventType::KeyPress(Key::AltGr) => { // Windows uses AltGr for Right Alt
                        if enable_toggle.load(Ordering::Relaxed) {
                            tx.send(InputEvent::Toggle).ok();
                        }
                    },

                    // Hold Mode (Left Ctrl + Left Win)
                    EventType::KeyPress(Key::ControlLeft) => {
                        is_ctrl = true;
                        check_combo(&enable_hold, &mut combo_active, is_ctrl, is_win, &tx);
                    },
                    EventType::KeyRelease(Key::ControlLeft) => {
                        is_ctrl = false;
                        check_combo(&enable_hold, &mut combo_active, is_ctrl, is_win, &tx);
                    },
                    EventType::KeyPress(Key::MetaLeft) => {
                        is_win = true;
                        check_combo(&enable_hold, &mut combo_active, is_ctrl, is_win, &tx);
                    },
                    EventType::KeyRelease(Key::MetaLeft) => {
                        is_win = false;
                        check_combo(&enable_hold, &mut combo_active, is_ctrl, is_win, &tx);
                    },
                    
                    _ => {}
                }
            }) {
                println!("Error in input listener: {:?}", error);
            }
        });
    }
}

fn check_combo(enable_hold: &Arc<AtomicBool>, active: &mut bool, ctrl: bool, win: bool, tx: &Sender<InputEvent>) {
    if !enable_hold.load(Ordering::Relaxed) {
        return;
    }
    let is_combo = ctrl && win;
    if is_combo && !*active {
        *active = true;
        tx.send(InputEvent::Start).ok();
    } else if !is_combo && *active {
        *active = false;
        tx.send(InputEvent::Stop).ok();
    }
}
