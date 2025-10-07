use acton_reactive::prelude::*;

/// Actor responsible for downloading crate artifacts from crates.io
///
/// The `CrateDownloader` actor manages the process of downloading crate files
/// (.crate archives) from the crates.io registry. It handles HTTP requests to
/// download specific versions of crates based on their specifier (name and version).
///
/// # State Management
///
/// This actor maintains the crate specifier that identifies which crate version
/// to download. The actor can be spawned with a specific crate specifier and
/// will handle download requests asynchronously.
///
/// # Actor Lifecycle
///
/// The actor is created using the acton-reactive framework and can be managed
/// by the `AgentRuntime`. Handler methods for download operations should be
/// implemented separately to process download requests and manage the download
/// lifecycle.
///
/// # Examples
///
/// ```no_run
/// use acton_reactive::prelude::*;
/// use crately::crate_downloader::CrateDownloader;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = ActonApp::launch();
/// let specifier = CrateSpecifier::from_str("serde@1.0.0")?;
/// let downloader = CrateDownloader::new(specifier);
///
/// // Actor can be spawned and managed through the runtime
/// // Handler implementations would process download requests
/// # Ok(())
/// # }
/// ```
#[acton_actor]
pub struct CrateDownloader {}
