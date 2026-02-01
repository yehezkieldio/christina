use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use ratatui::crossterm::event::{self as crossterm_event, Event as CrosstermEvent};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle as TokioJoinHandle;

use super::events::Event;

/// 10ms provides responsive keyboard input (100 Hz polling).
const INPUT_POLL_MS: u64 = 10;

/// 80ms (12.5 Hz) provides smooth spinner/progress animations, matching CLI tick rate.
const TICK_INTERVAL_MS: u64 = 80;

/// Tick task shutdown timeout (milliseconds).
const TICK_SHUTDOWN_TIMEOUT_MS: u64 = 200;

/// Handles for background event-producing tasks.
pub struct EventProducers {
    input_thread: JoinHandle<()>,
    tick_task: TokioJoinHandle<()>,
    should_stop: Arc<AtomicBool>,
    shutdown_tx: broadcast::Sender<()>,
}

impl EventProducers {
    /// Spawn background event producers.
    ///
    /// This allows generation tasks and other producers to share the same channel.
    /// Includes explicit shutdown signaling via broadcast channel for coordinated termination.
    pub fn spawn(event_tx: mpsc::Sender<Event>) -> Self {
        let should_stop = Arc::new(AtomicBool::new(false));
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        // Input thread (dedicated OS thread for blocking I/O)
        let tx_input = event_tx.clone();
        let should_stop_input = Arc::clone(&should_stop);
        let mut shutdown_rx_input = shutdown_tx.subscribe();
        let input_thread = std::thread::spawn(move || {
            loop {
                // Check shutdown signal (non-blocking)
                if should_stop_input.load(Ordering::Relaxed) || shutdown_rx_input.try_recv().is_ok()
                {
                    break;
                }

                match crossterm_event::poll(std::time::Duration::from_millis(INPUT_POLL_MS)) {
                    Ok(true) => match crossterm_event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            if tx_input.blocking_send(Event::Input(key)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Resize(_, _)) => {
                            if tx_input.blocking_send(Event::Resize).is_err() {
                                break;
                            }
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    },
                    Ok(false) => {}
                    Err(_) => break,
                }
            }
        });

        // Tick task (async task for periodic ticks)
        let tx_tick = event_tx.clone();
        let should_stop_tick = Arc::clone(&should_stop);
        let mut shutdown_rx_tick = shutdown_tx.subscribe();
        let tick_task = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_millis(TICK_INTERVAL_MS));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if should_stop_tick.load(Ordering::Relaxed) {
                            break;
                        }
                        let _ = tx_tick.try_send(Event::Tick);
                    }
                    _ = shutdown_rx_tick.recv() => {
                        break;
                    }
                }
            }
        });

        Self {
            input_thread,
            tick_task,
            should_stop,
            shutdown_tx,
        }
    }

    pub async fn shutdown(self) {
        self.should_stop.store(true, Ordering::SeqCst);

        // Broadcast shutdown signal to all subscribers
        let _ = self.shutdown_tx.send(());

        // Wait for tick task with timeout
        let _ = tokio::time::timeout(
            tokio::time::Duration::from_millis(TICK_SHUTDOWN_TIMEOUT_MS),
            self.tick_task,
        )
        .await;

        // Wait for input thread (OS thread)
        let _ = self.input_thread.join();

        // Drop shutdown_tx to signal all subscribers that no more messages will come
        drop(self.shutdown_tx);
    }
}
