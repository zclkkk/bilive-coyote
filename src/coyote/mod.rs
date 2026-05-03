pub mod manager;
pub mod protocol;
pub mod qrcode;
pub mod session;

pub use manager::{CoyoteCommand, CoyoteHandle, CoyoteManager, CoyoteStatus};
pub use qrcode::generate_qr_data_url;
