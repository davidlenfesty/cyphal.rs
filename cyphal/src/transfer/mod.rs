//! Transfer management.
//!
use crate::time::Timestamp;
use crate::types::*;

use crate::Priority;

pub mod manager;

#[cfg(feature = "std")]
pub mod map_manager;

pub use manager::TransferManager;

/// Protocol-level transfer types.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum TransferKind {
    Message,
    Response,
    Request,
}

#[derive(Debug)]
pub struct TransferMetadata<C: embedded_time::Clock> {
    // for tx -> transmission_timeout
    pub timestamp: Timestamp<C>,
    pub priority: Priority,
    pub transfer_kind: TransferKind,
    pub port_id: PortId,
    pub remote_node_id: Option<NodeId>,
    pub transfer_id: TransferId,

    // not sure if this is valid yet.
    pub frame_id: u32,
    pub crc: u32,
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
    // TODO also CAN needs to carry state inside of an individual transfer...
    pub first_frame: bool,
    pub last_frame: bool,
}
