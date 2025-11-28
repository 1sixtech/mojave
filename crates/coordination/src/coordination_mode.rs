/// How the node coordinates leadership.
#[derive(Debug, Clone, Copy)]
pub enum CoordinationMode {
    Kubernetes,
    Standalone,
}
