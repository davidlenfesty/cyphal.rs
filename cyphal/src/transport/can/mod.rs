//! UAVCAN/CAN transport implementation.
//!
//! CAN will essentially be the "reference implementation", and *should* always follow
//! the best practices, so if you want to add support for a new transport, you should
//! follow the conventions here.
//!
//! Provides a unit struct to create a Node for CAN. This implements the common
//! transmit function that *must* be implemented by any transport. This can't be a
//! trait in stable unfortunately because it would require GATs, which won't be stable
//! for quite a while... :(.

// TODO what exactly did we actually need GAT for?

mod bitfields;
// TODO temp uncomment
//mod fd;
mod legacy;

#[cfg(test)]
mod tests;

// Exports
pub use bitfields::{CanMessageId, CanServiceId};
// TODO temp uncomment
//pub use fd::*;
pub use legacy::*;
