use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum Verbosity {
    Low = 0,
    Medium = 1,
    High = 2,
}

impl Verbosity {
    pub fn from_u8(level: u8) -> Self {
        match level {
            0 => Verbosity::Low,
            1 => Verbosity::Medium,
            2 => Verbosity::High,
            _ => Verbosity::Low,
        }
    }
}

static CURRENT_VERBOSITY: AtomicU8 = AtomicU8::new(0);
static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn set_verbosity(level: Verbosity) {
    CURRENT_VERBOSITY.store(level as u8, Ordering::SeqCst);
}

pub fn log(level: Verbosity, message: &str) {
    if level <= Verbosity::from_u8(CURRENT_VERBOSITY.load(Ordering::SeqCst)) {
        match level {
            Verbosity::Low => println!("ℹ️  {}", message),
            Verbosity::Medium => println!("🔍 {}", message),
            Verbosity::High => println!("🐛 {}", message),
        }
    }
}

pub fn log_data(level: Verbosity, title: &str, data: &[u8]) {
    if level <= Verbosity::from_u8(CURRENT_VERBOSITY.load(Ordering::SeqCst)) {
        let hex = data.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
        println!("🐛 {}", title);
        println!("  └─ {}", hex);
        println!();
    }
}

pub fn log_detail(level: Verbosity, title: &str, detail: &str) {
    if level <= Verbosity::from_u8(CURRENT_VERBOSITY.load(Ordering::SeqCst)) {
        match level {
            Verbosity::Low => println!("ℹ️  {}", title),
            Verbosity::Medium => println!("🔍 {}", title),
            Verbosity::High => println!("🐛 {}", title),
        }
        println!("  └─ {}", detail);
        println!();
    }
}

pub fn log_block(title: &str, lines: Vec<String>) {
    let event_num = EVENT_COUNTER.fetch_add(1, Ordering::SeqCst);
    println!("╭── Event #{}  [{}]", event_num, title);
    for line in lines {
        println!("│   {}", line);
    }
    println!("╰──────────────────────────────────");
    println!();
}