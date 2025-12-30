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

// Kubernetes namespace
pub const INFERADB_NAMESPACE: &str = "inferadb";

// InferaDB deployment names
pub const DEPLOYMENT_ENGINE: &str = "dev-inferadb-engine";
pub const DEPLOYMENT_CONTROL: &str = "dev-inferadb-control";
pub const DEPLOYMENT_DASHBOARD: &str = "dev-inferadb-dashboard";

/// All InferaDB deployments for iteration
pub const INFERADB_DEPLOYMENTS: &[&str] =
    &[DEPLOYMENT_ENGINE, DEPLOYMENT_CONTROL, DEPLOYMENT_DASHBOARD];

// Kubernetes version for Talos cluster
pub const KUBERNETES_VERSION: &str = "1.32.0";

// Talos cluster configuration
pub const TALOS_WORKERS: &str = "1";
pub const TALOS_CONTROLPLANES: &str = "1";
pub const TALOS_PROVISIONER: &str = "docker";
pub const TALOS_WAIT_TIMEOUT: &str = "10m";

// Helm repositories
pub const HELM_TAILSCALE_REPO: &str = "tailscale";
pub const HELM_TAILSCALE_URL: &str = "https://pkgs.tailscale.com/helmcharts";

// Tip messages
pub const TIP_START_CLUSTER: &str = "Run 'inferadb dev start' to start the cluster";
pub const TIP_RESUME_CLUSTER: &str = "Run 'inferadb dev start' to resume the cluster";

/// Target line width for step output (before terminal margin).
pub const STEP_LINE_WIDTH: usize = 120;

// Container stabilization delay (seconds)
pub const CONTAINER_STABILIZE_DELAY_SECS: u64 = 3;

// Resource termination delay (seconds)
pub const RESOURCE_TERMINATE_DELAY_SECS: u64 = 5;
