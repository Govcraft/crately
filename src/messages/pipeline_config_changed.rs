//! PipelineConfigChanged message type.
//!
//! This message is broadcast when pipeline configuration has been reloaded,
//! allowing pipeline actors to update their runtime behavior based on new settings.

use acton_reactive::prelude::*;

use crate::actors::config::PipelineConfig;

/// Event broadcast when pipeline configuration has been reloaded.
///
/// This message is sent by the ConfigManager actor after successfully
/// reloading configuration from disk. Pipeline actors (DownloaderActor,
/// FileReaderActor, ProcessorActor, VectorizerActor) subscribe to this
/// event to update their runtime behavior based on new configuration values.
///
/// # Actor Model Pattern
///
/// This follows the event broadcasting pattern where ConfigManager is
/// the publisher and pipeline actors are subscribers. The event flows:
///
/// 1. ConfigManager reloads config from disk
/// 2. ConfigManager validates pipeline configuration
/// 3. ConfigManager broadcasts PipelineConfigChanged with new settings
/// 4. Subscribed pipeline actors receive event and update their behavior
/// 5. Console actor may also subscribe to display reload confirmation
///
/// # Event Architecture
///
/// This event is broadcast via the broker pub/sub system, allowing multiple
/// actors to react independently to configuration changes. No direct coupling
/// exists between ConfigManager and pipeline actors.
///
/// # Fields
///
/// * `pipeline` - The updated pipeline configuration containing settings for
///   all four processing stages (download, read, process, vectorize)
///
/// # Example
///
/// ```no_run
/// use crately::messages::PipelineConfigChanged;
/// use crately::actors::config::PipelineConfig;
/// use acton_reactive::prelude::*;
///
/// async fn handle_config_change(pipeline_config: PipelineConfig) {
///     let message = PipelineConfigChanged {
///         pipeline: pipeline_config,
///     };
///     // Broadcast to subscribed actors
///     // broker.broadcast(message).await;
/// }
/// ```
#[acton_message(raw)]
#[derive(Clone)]
pub struct PipelineConfigChanged {
    /// The updated pipeline configuration
    pub pipeline: PipelineConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::config::PipelineConfig;

    #[test]
    fn test_pipeline_config_changed_creation() {
        let pipeline = PipelineConfig::default();
        let message = PipelineConfigChanged {
            pipeline: pipeline.clone(),
        };
        assert_eq!(message.pipeline, pipeline);
    }

    #[test]
    fn test_pipeline_config_changed_clone() {
        let pipeline = PipelineConfig::default();
        let message1 = PipelineConfigChanged {
            pipeline: pipeline.clone(),
        };
        let message2 = message1.clone();
        assert_eq!(message1.pipeline, message2.pipeline);
    }

    #[test]
    fn test_pipeline_config_changed_debug() {
        let pipeline = PipelineConfig::default();
        let message = PipelineConfigChanged { pipeline };
        let debug_str = format!("{:?}", message);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("PipelineConfigChanged"));
    }
}
