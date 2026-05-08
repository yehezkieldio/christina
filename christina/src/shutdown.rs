//! Shutdown signal handling for expensive generation work.
//!
//! The first signal starts graceful cancellation. A second signal exits the
//! process because the user has explicitly asked us to stop waiting.

use tokio::signal;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownSignal {
    CtrlC,
    #[cfg(unix)]
    Sigterm,
}

impl ShutdownSignal {
    pub const fn name(self) -> &'static str {
        match self {
            Self::CtrlC => "ctrl_c",
            #[cfg(unix)]
            Self::Sigterm => "sigterm",
        }
    }
}

pub fn spawn_signal_handler(token: CancellationToken) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        match wait_for_shutdown_signal().await {
            Ok(signal) => {
                tracing::warn!(signal = signal.name(), "Shutdown requested");
                token.cancel();
            }
            Err(err) => {
                tracing::warn!("Failed to install shutdown handler: {}", err);
                return;
            }
        }

        if let Ok(signal) = wait_for_shutdown_signal().await {
            tracing::warn!(signal = signal.name(), "Forced shutdown requested");
            std::process::exit(130);
        }
    })
}

async fn wait_for_shutdown_signal() -> std::io::Result<ShutdownSignal> {
    #[cfg(unix)]
    {
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = signal::ctrl_c() => {
                result?;
                Ok(ShutdownSignal::CtrlC)
            }
            _ = sigterm.recv() => Ok(ShutdownSignal::Sigterm),
        }
    }

    #[cfg(not(unix))]
    {
        signal::ctrl_c().await?;
        Ok(ShutdownSignal::CtrlC)
    }
}
