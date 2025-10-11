//! Message to print a horizontal separator line to the console.

use acton_reactive::prelude::*;

/// Message sent to the Console actor to print a horizontal separator line.
///
/// The `PrintSeparator` message instructs the Console actor to output a visual
/// separator line, useful for dividing sections of console output to improve
/// readability and visual organization.
///
/// # Purpose
///
/// Console separators serve several important purposes:
///
/// - **Visual Organization**: Create clear divisions between different sections of output
/// - **Readability**: Improve scannability of console output for users
/// - **Status Grouping**: Separate different phases or states of application execution
/// - **Professional Presentation**: Provide consistent, polished console formatting
///
/// # Usage
///
/// The `PrintSeparator` message has no fields because it represents a simple formatting
/// action with no parameterization. The actual separator style and length are defined
/// by the Console actor's implementation.
///
/// ## Example
///
/// ```rust,no_run
/// use acton_reactive::prelude::*;
/// use crately::messages::PrintSeparator;
///
/// async fn display_section_with_separator(console: AgentHandle) {
///     // Print some content
///     // ...
///
///     // Add a visual separator
///     console.send(PrintSeparator).await;
///
///     // Print next section of content
///     // ...
/// }
/// ```
///
/// # Design Notes
///
/// This is a fieldless struct (unit-like struct) because:
///
/// - The separator has a fixed visual style defined by the Console actor
/// - No customization parameters are needed for basic separation functionality
/// - Keeping messages small reduces overhead in the actor system
/// - Consistent separator appearance improves UX predictability
///
/// If variable separator styles are needed in the future, consider either:
/// - Creating additional message types for different separator styles
/// - Adding an optional field to specify separator characteristics
#[acton_message]
pub struct PrintSeparator;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that PrintSeparator can be created and cloned (required by ActonMessage).
    #[test]
    fn test_print_separator_creation() {
        let separator = PrintSeparator;
        let _cloned = separator.clone();
        // If we get here without panicking, Clone is properly implemented
    }

    /// Verify that PrintSeparator implements Debug (required by ActonMessage).
    #[test]
    fn test_print_separator_debug() {
        let separator = PrintSeparator;
        let debug_string = format!("{:?}", separator);
        assert!(
            !debug_string.is_empty(),
            "Debug output should not be empty"
        );
    }

    /// Verify that PrintSeparator is Send + Sync (required for actor message passing).
    #[test]
    fn test_print_separator_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<PrintSeparator>();
        assert_sync::<PrintSeparator>();
    }
}
