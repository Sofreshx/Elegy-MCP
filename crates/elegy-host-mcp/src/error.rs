use elegy_core::CoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HostError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    ServerInitialize(#[from] rmcp::service::ServerInitializeError),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Rmcp(#[from] rmcp::RmcpError),
}
