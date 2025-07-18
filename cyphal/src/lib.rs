//! # UAVCAN implementation
//!
//! The intent with this implementation right now is to present a transport
//! and session-management agnostic interface for UAVCAN. What I'm working on
//! here is not meant to implement the higher-level protocol features, such
//! as automatic heartbeat publication. It is simply meant to manage ingesting
//! and producing raw frames to go on the bus. There is room to provide
//! application-level constructs in this crate, but that's not what I'm working
//! on right now.
//!
//! ## Comparison to canadensis
//!
//! The only other Rust UAVCAN implementation with any real progess at the
//! moment is canadensis. I *believe* that it is fully functional but I haven't
//! verified that.
//!
//! canadensis seems to be providing a more specific implementation (CAN-only)
//! that provides more application level features (e.g. a Node w/ Heartbeat
//! publishing) that relies on a global allocator. The intent (or experiment)
//! here is to provide a single unified interface for different transports
//! and storage backends. Application level functionality can live on top of
//! this. I can see issues with this running into issues in multi-threaded
//! environments, but I'll get to those when I get to them.
#![no_std]
//#![deny(warnings)]

#[allow(unused_imports)]
#[cfg(feature = "std")]
#[macro_use]
extern crate std;

#[cfg(test)]
extern crate test;

#[macro_use]
extern crate num_derive;

extern crate alloc;

pub mod time;

//mod crc16;
pub mod transfer;
pub mod transport;
pub mod types;

pub use node::{Node, TransmissionType};
use time::Duration;
pub use transfer::TransferKind;

pub use streaming_iterator::StreamingIterator;

mod node;

use types::*;

/// Protocol errors possible from receiving incoming frames.
#[derive(Copy, Clone, Debug)]
pub enum RxError {
    TransferStartMissingToggle,
    /// Anonymous transfers must only use a single frame
    AnonNotSingleFrame,
    /// Frames that are not last cannot have less than the maximum MTU
    NonLastUnderUtilization,
    /// No type of frame can contain empty data, must always have at least a tail byte
    FrameEmpty,
    /// Id field is formatted incorrectly
    InvalidCanId,
    /// Non-start frame received without session
    NewSessionNoStart,
    /// Session has expired
    Timeout,

    InvalidFrameOrdering,

    CrcError,

    InvalidPayload,

    /// Transport implementation has incorrectly assigned a remote node id to a message
    MessageWithRemoteId,
}

/// Errors that can be caused by incorrect parameters for transmission
///
/// TODO I should be able to capture these errors in the type system, making it impossible to do,
/// but this is still a first pass, so I'll leave them as runtime for now.
#[derive(Copy, Clone, Debug)]
pub enum TxError {
    AnonNotSingleFrame,
    ServiceNoSourceID,
    ServiceNoDestinationID,
}

// TODO could replace with custom impl's to reduce dependencies
// TODO how could I represent more priorities for different transports?
/// Protocol-level priorities.
///
/// Transports are supposed to be able to support more than these base 8
/// priorities, but there is currently no API for that.
#[derive(FromPrimitive, ToPrimitive, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub enum Priority {
    Exceptional,
    Immediate,
    Fast,
    High,
    Nominal,
    Low,
    Slow,
    Optional,
}

/// Simple subscription type to
// TODO remove this allow
#[allow(dead_code)]
pub struct Subscription {
    transfer_kind: TransferKind,
    port_id: PortId,
    extent: usize,
    timeout: Duration,
}

impl Subscription {
    pub fn new(
        transfer_kind: TransferKind,
        port_id: PortId,
        extent: usize,
        timeout: Duration,
    ) -> Self {
        Self {
            transfer_kind,
            port_id,
            extent,
            timeout,
        }
    }
}

impl PartialEq for Subscription {
    fn eq(&self, other: &Self) -> bool {
        self.transfer_kind == other.transfer_kind && self.port_id == other.port_id
    }
}
