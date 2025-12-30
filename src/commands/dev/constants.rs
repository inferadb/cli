//! Constants for the dev cluster commands.

// Cluster identifiers
pub const CLUSTER_NAME: &str = "inferadb-dev";
pub const KUBE_CONTEXT: &str = "admin@inferadb-dev";
pub const REGISTRY_NAME: &str = "inferadb-registry";
pub const REGISTRY_PORT: u16 = 5050;

/// Prefix for Tailscale devices created by dev environment ingress resources
pub const TAILSCALE_DEVICE_PREFIX: &str = "inferadb-dev-";

// Repository URLs
pub const DEPLOY_REPO_URL: &str = "https://github.com/inferadb/deploy.git";
pub const ENGINE_REPO_URL: &str = "https://github.com/inferadb/engine.git";
pub const CONTROL_REPO_URL: &str = "https://github.com/inferadb/control.git";
pub const DASHBOARD_REPO_URL: &str = "https://github.com/inferadb/dashboard.git";

// Tip messages
pub const TIP_START_CLUSTER: &str = "Run 'inferadb dev start' to start the cluster";
pub const TIP_RESUME_CLUSTER: &str = "Run 'inferadb dev start' to resume the cluster";

/// Target line width for step output (before terminal margin).
pub const STEP_LINE_WIDTH: usize = 120;
