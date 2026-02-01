use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use ratatui::crossterm::event::{self as crossterm_event, Event as CrosstermEvent, KeyEvent};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle as TokioJoinHandle;

use super::events::Event;

/// 10ms provides responsive keyboard input (100 Hz polling).
const INPUT_POLL_MS: u64 = 10;

/// 80ms (12.5 Hz) provides smooth spinner/progress animations, matching CLI tick rate.
const TICK_INTERVAL_MS: u64 = 80;

/// Tick task shutdown timeout (milliseconds).
const TICK_SHUTDOWN_TIMEOUT_MS: u64 = 200;

/// Trait for abstracting crossterm event polling to enable testing.
trait InputSource: Send + 'static {
    /// Poll for available input with a timeout.
    fn poll(&mut self, timeout: Duration) -> Result<bool, std::io::Error>;
    /// Read the next input event.
    fn read(&mut self) -> Result<InputEvent, std::io::Error>;
}

/// Input event types that can be produced.
#[derive(Debug, Clone)]
enum InputEvent {
    Key(KeyEvent),
    Resize,
}

/// Production implementation using crossterm.
struct CrosstermInputSource;

impl InputSource for CrosstermInputSource {
    fn poll(&mut self, timeout: Duration) -> Result<bool, std::io::Error> {
        crossterm_event::poll(timeout)
    }

    fn read(&mut self) -> Result<InputEvent, std::io::Error> {
        match crossterm_event::read()? {
            CrosstermEvent::Key(key) => Ok(InputEvent::Key(key)),
            CrosstermEvent::Resize(_, _) => Ok(InputEvent::Resize),
            _ => Err(std::io::Error::other("Unsupported event type")),
        }
    }
}

/// Trait for abstracting time intervals to enable testing.
trait IntervalSource: Send + 'static {
    /// Wait for the next interval tick.
    fn tick(&mut self) -> impl std::future::Future<Output = ()> + Send;
}

/// Production implementation using tokio intervals.
struct TokioIntervalSource {
    interval: tokio::time::Interval,
}

impl TokioIntervalSource {
    fn new(duration: Duration) -> Self {
        Self {
            interval: tokio::time::interval(duration),
        }
    }
}

impl IntervalSource for TokioIntervalSource {
    async fn tick(&mut self) {
        self.interval.tick().await;
    }
}

/// Handles for background event-producing tasks.
pub struct EventProducers {
    input_thread: JoinHandle<()>,
    tick_task: TokioJoinHandle<()>,
    should_stop: Arc<AtomicBool>,
    shutdown_tx: broadcast::Sender<()>,
}

impl EventProducers {
    /// Spawn background event producers using production sources.
    pub fn spawn(event_tx: mpsc::Sender<Event>) -> Self {
        Self::spawn_with_sources(
            event_tx,
            CrosstermInputSource,
            TokioIntervalSource::new(Duration::from_millis(TICK_INTERVAL_MS)),
        )
    }

    /// Spawn background event producers with injectable sources for testing.
    fn spawn_with_sources<I, T>(
        event_tx: mpsc::Sender<Event>,
        mut input_source: I,
        mut tick_source: T,
    ) -> Self
    where
        I: InputSource,
        T: IntervalSource,
    {
        let should_stop = Arc::new(AtomicBool::new(false));
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        let tx_input = event_tx.clone();
        let should_stop_input = Arc::clone(&should_stop);
        let mut shutdown_rx_input = shutdown_tx.subscribe();
        let input_thread = std::thread::spawn(move || {
            loop {
                if should_stop_input.load(Ordering::Relaxed) || shutdown_rx_input.try_recv().is_ok()
                {
                    break;
                }

                match input_source.poll(Duration::from_millis(INPUT_POLL_MS)) {
                    Ok(true) => match input_source.read() {
                        Ok(InputEvent::Key(key)) => {
                            if tx_input.blocking_send(Event::Input(key)).is_err() {
                                break;
                            }
                        }
                        Ok(InputEvent::Resize) => {
                            if tx_input.blocking_send(Event::Resize).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    },
                    Ok(false) => {}
                    Err(_) => break,
                }
            }
        });

        let tx_tick = event_tx.clone();
        let should_stop_tick = Arc::clone(&should_stop);
        let mut shutdown_rx_tick = shutdown_tx.subscribe();
        let tick_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tick_source.tick() => {
                        if should_stop_tick.load(Ordering::Relaxed) {
                            break;
                        }
                        if tx_tick.try_send(Event::Tick).is_err() {
                            tracing::debug!("Tick channel closed, stopping tick task");
                            break;
                        }
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

        if self.shutdown_tx.send(()).is_err() {
            tracing::debug!("Shutdown channel closed, no subscribers to notify");
        }

        let _ = tokio::time::timeout(
            Duration::from_millis(TICK_SHUTDOWN_TIMEOUT_MS),
            self.tick_task,
        )
        .await;

        let _ = self.input_thread.join();

        drop(self.shutdown_tx);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockInputSource {
        events: Arc<Mutex<Vec<Result<InputEvent, std::io::Error>>>>,
        poll_count: Arc<Mutex<usize>>,
    }

    impl MockInputSource {
        fn new(events: Vec<Result<InputEvent, std::io::Error>>) -> Self {
            Self {
                events: Arc::new(Mutex::new(events)),
                poll_count: Arc::new(Mutex::new(0)),
            }
        }

        fn empty() -> Self {
            Self::new(vec![])
        }
    }

    impl InputSource for MockInputSource {
        fn poll(&mut self, _timeout: Duration) -> Result<bool, std::io::Error> {
            let mut count = self.poll_count.lock().unwrap();
            *count += 1;

            let events = self.events.lock().unwrap();
            Ok(!events.is_empty())
        }

        fn read(&mut self) -> Result<InputEvent, std::io::Error> {
            let mut events = self.events.lock().unwrap();
            if events.is_empty() {
                Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "No events available",
                ))
            } else {
                events.remove(0)
            }
        }
    }

    struct MockIntervalSource {
        ticks: Arc<Mutex<usize>>,
        max_ticks: usize,
    }

    impl MockIntervalSource {
        fn new(max_ticks: usize) -> Self {
            Self {
                ticks: Arc::new(Mutex::new(0)),
                max_ticks,
            }
        }
    }

    impl IntervalSource for MockIntervalSource {
        async fn tick(&mut self) {
            let tick_value = {
                let mut ticks = self.ticks.lock().unwrap();
                *ticks += 1;
                *ticks
            };
            
            if tick_value >= self.max_ticks {
                tokio::time::sleep(Duration::from_secs(3600)).await;
            } else {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
    }

    #[tokio::test]
    async fn test_tick_producer_generates_ticks() {
        let (tx, mut rx) = mpsc::channel(10);
        let mock_interval = MockIntervalSource::new(5);
        let mock_input = MockInputSource::empty();

        let producers = EventProducers::spawn_with_sources(tx, mock_input, mock_interval);

        let mut tick_count = 0;
        let timeout_result = tokio::time::timeout(Duration::from_millis(500), async {
            while let Some(Event::Tick) = rx.recv().await {
                tick_count += 1;
                if tick_count >= 3 {
                    break;
                }
            }
        })
        .await;

        assert!(timeout_result.is_ok());
        assert_eq!(tick_count, 3);

        producers.shutdown().await;
    }

    #[tokio::test]
    async fn test_input_producer_generates_key_events() {
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};

        let (tx, mut rx) = mpsc::channel(10);
        let key_event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
        let mock_input = MockInputSource::new(vec![Ok(InputEvent::Key(key_event))]);
        let mock_interval = MockIntervalSource::new(0);

        let producers = EventProducers::spawn_with_sources(tx, mock_input, mock_interval);

        std::thread::sleep(Duration::from_millis(50));

        let event = rx.recv().await;
        assert!(matches!(event, Some(Event::Input(_))));

        producers.shutdown().await;
    }

    #[tokio::test]
    async fn test_input_producer_generates_resize_events() {
        let (tx, mut rx) = mpsc::channel(10);
        let mock_input = MockInputSource::new(vec![Ok(InputEvent::Resize)]);
        let mock_interval = MockIntervalSource::new(0);

        let producers = EventProducers::spawn_with_sources(tx, mock_input, mock_interval);

        std::thread::sleep(Duration::from_millis(50));

        let event = rx.recv().await;
        assert!(matches!(event, Some(Event::Resize)));

        producers.shutdown().await;
    }

    #[tokio::test]
    async fn test_input_producer_handles_errors_gracefully() {
        let (tx, mut rx) = mpsc::channel(10);
        let mock_input = MockInputSource::new(vec![Err(std::io::Error::other("Read error"))]);
        let mock_interval = MockIntervalSource::new(3);

        let producers = EventProducers::spawn_with_sources(tx, mock_input, mock_interval);

        std::thread::sleep(Duration::from_millis(50));

        let mut tick_count = 0;
        while let Ok(event) = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            if let Some(Event::Tick) = event {
                tick_count += 1;
            }
            if tick_count >= 2 {
                break;
            }
        }

        assert!(tick_count >= 2);

        producers.shutdown().await;
    }

    #[tokio::test]
    async fn test_shutdown_stops_all_producers() {
        let (tx, mut rx) = mpsc::channel(10);
        let mock_input = MockInputSource::empty();
        let mock_interval = MockIntervalSource::new(2);

        let producers = EventProducers::spawn_with_sources(tx, mock_input, mock_interval);

        let event = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(event.is_ok());

        producers.shutdown().await;

        rx.close();
        while rx.recv().await.is_some() {}
    }

    #[tokio::test]
    async fn test_shutdown_with_atomic_flag() {
        let (tx, _rx) = mpsc::channel(10);
        let mock_input = MockInputSource::empty();
        let mock_interval = MockIntervalSource::new(100);

        let producers = EventProducers::spawn_with_sources(tx, mock_input, mock_interval);

        assert!(!producers.should_stop.load(Ordering::Relaxed));

        producers.shutdown().await;
    }

    #[tokio::test]
    async fn test_channel_close_stops_producers() {
        let (tx, rx) = mpsc::channel(10);
        let mock_input = MockInputSource::empty();
        let mock_interval = MockIntervalSource::new(100);

        let producers = EventProducers::spawn_with_sources(tx, mock_input, mock_interval);

        drop(rx);

        tokio::time::sleep(Duration::from_millis(100)).await;

        producers.shutdown().await;
    }

    #[tokio::test]
    async fn test_multiple_input_events() {
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};

        let (tx, mut rx) = mpsc::channel(10);
        let key1 = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
        let key2 = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty());
        let mock_input = MockInputSource::new(vec![
            Ok(InputEvent::Key(key1)),
            Ok(InputEvent::Key(key2)),
            Ok(InputEvent::Resize),
        ]);
        let mock_interval = MockIntervalSource::new(0);

        let producers = EventProducers::spawn_with_sources(tx, mock_input, mock_interval);

        std::thread::sleep(Duration::from_millis(100));

        let mut events = vec![];
        while let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(50), rx.recv()).await
        {
            events.push(event);
            if events.len() >= 3 {
                break;
            }
        }

        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], Event::Input(_)));
        assert!(matches!(events[1], Event::Input(_)));
        assert!(matches!(events[2], Event::Resize));

        producers.shutdown().await;
    }

    #[tokio::test]
    async fn test_tick_producer_respects_shutdown_signal() {
        let (tx, mut rx) = mpsc::channel(10);
        let mock_input = MockInputSource::empty();
        let mock_interval = MockIntervalSource::new(1000);

        let producers = EventProducers::spawn_with_sources(tx, mock_input, mock_interval);

        tokio::time::sleep(Duration::from_millis(50)).await;

        producers.should_stop.store(true, Ordering::SeqCst);

        tokio::time::sleep(Duration::from_millis(50)).await;

        let result =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(result.is_err() || matches!(result.unwrap(), Some(Event::Tick)));

        producers.shutdown().await;
    }
}
