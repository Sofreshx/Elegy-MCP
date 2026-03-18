mod error;
mod host;

pub use error::HostError;
pub use host::{serve_stdio, ElegyMcpHost};
