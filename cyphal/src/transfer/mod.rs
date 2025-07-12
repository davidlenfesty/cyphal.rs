//! Transfer management.
//!
use core::hash::Hash;

use crate::time::Timestamp;
use crate::types::*;

use crate::Priority;

pub mod manager;

#[cfg(feature = "std")]
pub mod map_manager;

pub use manager::TransferManager;

/// Protocol-level transfer types.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum TransferKind {
    Message,
    Response,
    Request,
}

/// Metadata describing a transfer. This metadata is transport-agnostic.
#[derive(Debug, Clone)]
pub struct TransferMetadata<C: embedded_time::Clock> {
    // for tx -> transmission_timeout
    pub timestamp: Timestamp<C>,
    pub priority: Priority,
    pub transfer_kind: TransferKind,
    pub port_id: PortId,
    pub remote_node_id: Option<NodeId>,
    pub transfer_id: TransferId,
}

impl<C: embedded_time::Clock> Hash for TransferMetadata<C> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Ignore the timestamp. Ideally we use it but it's not really necessary
        state.write_u8(self.priority as u8);
        state.write_u8(self.transfer_kind as u8);
        if let Some(remote_node_id) = self.remote_node_id {
            state.write_u16(remote_node_id);
        }
        state.write_u16(self.port_id);
        state.write_u8(self.transfer_id);
    }
}

//#[cfg(not(feature = "std"))]
//mod heap_based;
//#[cfg(feature = "std")]
//mod std_vec;
//
//#[cfg(not(feature = "std"))]
//pub use heap_based::HeapSessionManager;
//
//#[cfg(feature = "std")]
//pub use std_vec::StdVecSessionManager;

/// Session-related errors, caused by reception errors.
#[derive(Copy, Clone, Debug)]
pub enum TransferError {
    OutOfSpace,
    Timeout,
    NewSessionNoStart,
    InvalidTransferId,
    // TODO come up with a way to return more specific errors
    BadMetadata,
}

pub struct Frame<'a, C: embedded_time::Clock> {
    pub metadata: TransferMetadata<C>,
    pub payload: &'a [u8],

    // TODO how to enable out of order re-assembly?
    pub first_frame: bool,
    pub last_frame: bool,
}
