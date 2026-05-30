//! Progress reporting for the CLI: TTY spinner and plain-text fallback.

use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dk_core::review::Progress;

/// Renders [`Progress`] events from the review pipeline. On a TTY it animates a
/// spinner with elapsed time during the (long) agent call; otherwise it prints
/// plain stage lines. All output goes to stderr, never stdout.
pub struct ProgressReporter {
    agent: String,
    tty: bool,
    ticker: Mutex<Option<Ticker>>,
}

impl ProgressReporter {
    pub fn new(agent: &str) -> Self {
        ProgressReporter {
            agent: agent.to_string(),
            tty: io::stderr().is_terminal(),
            ticker: Mutex::new(None),
        }
    }

    pub fn handle(&self, event: Progress) {
        match event {
            Progress::AgentRunning { attempt, total } => {
                let label = if total > 1 {
                    format!("Reviewing with {} (attempt {attempt}/{total})", self.agent)
                } else {
                    format!("Reviewing with {}", self.agent)
                };
                if self.tty {
                    self.swap_ticker(Some(Ticker::start(label)));
                } else {
                    eprintln!("dk: {label}…");
                }
            }
            Progress::Validating { .. } => {
                self.swap_ticker(None);
                eprintln!("dk: validating response…");
            }
        }
    }

    fn swap_ticker(&self, next: Option<Ticker>) {
        let mut guard = self.ticker.lock().unwrap();
        if let Some(mut old) = guard.take() {
            old.stop();
        }
        *guard = next;
    }

    /// Stop any running spinner. Call after the pipeline returns (incl. errors).
    pub fn finish(&self) {
        self.swap_ticker(None);
    }
}

/// Background spinner thread that repaints an elapsed-time line on stderr.
struct Ticker {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Ticker {
    fn start(label: String) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::spawn(move || {
            const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let start = Instant::now();
            let mut i = 0usize;
            while !flag.load(Ordering::Relaxed) {
                eprint!(
                    "\r\x1b[2K{} {label}… {}s",
                    FRAMES[i % FRAMES.len()],
                    start.elapsed().as_secs()
                );
                let _ = io::stderr().flush();
                i += 1;
                std::thread::sleep(Duration::from_millis(120));
            }
        });
        Ticker {
            stop,
            handle: Some(handle),
        }
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        eprint!("\r\x1b[2K"); // clear the spinner line
        let _ = io::stderr().flush();
    }
}

/// Print `error [CODE]: message` to stderr and exit with status 1.
pub fn fail(code: &str, message: &str) -> ! {
    eprintln!("error [{code}]: {message}");
    std::process::exit(1);
}
