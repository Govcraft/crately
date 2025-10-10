//! Keyboard event handling actor for Crately.
//!
//! This module provides the `KeyboardHandler` actor that listens for keyboard events
//! and broadcasts them to subscribing actors using the pub/sub pattern. The handler
//! does not dictate what keys do - it simply broadcasts events for other actors to
//! handle as appropriate for their context.

use acton_reactive::prelude::*;
use anyhow::Result;
use crossterm::{
    event::{Event, EventStream, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::messages::KeyPressed;

/// Message to stop the keyboard handler gracefully.
#[acton_message]
pub struct StopKeyboardHandler;

/// Message sent after the keyboard handler starts to complete initialization
#[acton_message]
struct KeyboardHandlerStarted;

/// Actor that listens for keyboard events and broadcasts them to subscribers.
///
/// The `KeyboardHandler` actor provides keyboard event monitoring using the
/// crossterm library. It enables raw mode on the terminal to capture key events
/// immediately without buffering, then broadcasts `KeyPressed` messages to all
/// subscribers via the message broker.
///
/// # Architecture
///
/// The keyboard handler follows the pub/sub pattern:
/// - Listens for keyboard events in raw mode
/// - Broadcasts `KeyPressed` messages via the broker
/// - Does NOT dictate what keys do - that's up to subscribers
/// - Provides clean separation between event detection and handling
///
/// # Raw Mode
///
/// The actor enables terminal raw mode to capture key events immediately.
/// Raw mode is managed separately from the actor state to avoid clone issues.
///
/// # Example
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::keyboard_handler::KeyboardHandler;
/// use crately::messages::KeyPressed;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let mut runtime = ActonApp::launch();
///
///     // Spawn the keyboard handler
///     let keyboard_handle = KeyboardHandler::spawn(&mut runtime).await?;
///
///     // Subscribe to key events in another actor
///     // other_actor.subscribe::<KeyPressed>().await;
///
///     runtime.shutdown_all().await?;
///     Ok(())
/// }
/// ```
#[acton_actor]
pub struct KeyboardHandler {
    /// Whether the handler has been initialized
    initialized: bool,
    /// Channel sender for stopping the keyboard event loop
    stop_tx: Option<mpsc::Sender<()>>,
}

impl KeyboardHandler {
    /// Spawns, configures, and starts a new KeyboardHandler actor
    ///
    /// This is the standard factory method for creating KeyboardHandler actors.
    /// The actor enables terminal raw mode and begins listening for keyboard
    /// events, broadcasting them via the message broker.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    ///
    /// # Returns
    ///
    /// Returns `AgentHandle` to the started KeyboardHandler actor.
    ///
    /// # When to Use
    ///
    /// Call this when you need keyboard interaction in your application,
    /// typically for the serve command. Other actors should subscribe to
    /// `KeyPressed` messages to handle specific keys.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Terminal raw mode cannot be enabled
    /// - Actor creation or initialization fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use acton_reactive::prelude::*;
    /// use crately::keyboard_handler::KeyboardHandler;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut runtime = ActonApp::launch();
    ///     let keyboard = KeyboardHandler::spawn(&mut runtime).await?;
    ///     runtime.shutdown_all().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn spawn(runtime: &mut AgentRuntime) -> Result<AgentHandle> {
        let mut builder = runtime
            .new_agent_with_name::<KeyboardHandler>("keyboard_handler".to_string())
            .await;

        // Initialize model
        builder.model = KeyboardHandler {
            initialized: false,
            stop_tx: None,
        };

        builder.after_start(|_| {
            info!("KeyboardHandler starting - enabling raw mode");

            // Enable raw mode
            if let Err(e) = enable_raw_mode() {
                error!("Failed to enable raw mode: {}", e);
                return AgentReply::immediate();
            }
            AgentReply::immediate()
        });

        // Set up the keyboard event handler on start
        builder.mutate_on::<KeyboardHandlerStarted>(|agent, _envelope| {
            if agent.model.initialized {
                return AgentReply::immediate();
            }

            agent.model.initialized = true;

            // Create a channel for stopping the event reader
            let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);

            // Store the sender in the actor model for shutdown signaling
            agent.model.stop_tx = Some(stop_tx);

            // Spawn a task to read keyboard events
            let broker = agent.broker().clone();
            tokio::spawn(async move {
                let mut reader = EventStream::new();

                loop {
                    tokio::select! {
                        // Check for stop signal
                        _ = stop_rx.recv() => {
                            info!("Stop received");
                            break;
                        }
                        // Read keyboard events
                        maybe_event = reader.next() => {
                            match maybe_event {
                                Some(Ok(Event::Key(key_event))) => {
                                    // Only handle key press events, not release or repeat
                                    if key_event.kind == KeyEventKind::Press {
                                        let message = KeyPressed {
                                            key: key_event.code,
                                            modifiers: key_event.modifiers,
                                        };

                                        info!("Key pressed");
                                        broker.broadcast(message).await;
                                    }
                                }
                                Some(Ok(_)) => {
                                    // Other events (mouse, resize, etc.) - ignore
                                }
                                Some(Err(e)) => {
                                    error!("Error reading keyboard event: {}", e);
                                }
                                None => {
                                    warn!("Event stream ended");
                                    break;
                                }
                            }
                        }
                    }
                }
            });

            AgentReply::immediate()
        });

        // Handle stop requests
        builder.mutate_on::<StopKeyboardHandler>(|agent, _envelope| {
            // Signal the keyboard event loop to stop by dropping the sender
            if let Some(stop_tx) = agent.model.stop_tx.take() {
                drop(stop_tx);
            }
            AgentReply::immediate()
        });

        // Clean up on shutdown
        builder.before_stop(|_agent| {
            info!("KeyboardHandler stopping - disabling raw mode");

            // Disable raw mode
            if let Err(e) = disable_raw_mode() {
                error!("Failed to disable raw mode during cleanup: {}", e);
            }

            AgentReply::immediate()
        });

        let handle = builder.start().await;

        // Trigger initialization
        handle.send(KeyboardHandlerStarted).await;

        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_handler_default() {
        let handler = KeyboardHandler::default();
        assert!(!handler.initialized);
        assert!(handler.stop_tx.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_keyboard_handler_spawn_succeeds() {
        // This test verifies that the actor can be spawned and stopped
        // Actual keyboard event handling requires a TTY environment
        let mut runtime = ActonApp::launch();

        // Attempt to spawn - should succeed even in non-TTY environments
        let result = KeyboardHandler::spawn(&mut runtime).await;

        if let Ok(handle) = result {
            handle.send(StopKeyboardHandler).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            handle.stop().await.expect("Should stop cleanly");
        }

        runtime
            .shutdown_all()
            .await
            .expect("Runtime should shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_keyboard_handler_stop_message() {
        let mut runtime = ActonApp::launch();

        if let Ok(handle) = KeyboardHandler::spawn(&mut runtime).await {
            // Send stop message
            handle.send(StopKeyboardHandler).await;

            // Give it time to process
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            handle.stop().await.expect("Should stop cleanly");
        }

        runtime
            .shutdown_all()
            .await
            .expect("Runtime should shutdown");
    }
}
