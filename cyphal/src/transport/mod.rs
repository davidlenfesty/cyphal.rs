//! Transport-specific functionality.
//!
//! The current iteration requires 2 different implementations:
//! - SessionMetadata trait
//! - Transport trait
//!
//! Take a look at the CAN implementation for an example.

// Declaring all of the sub transport modules here.
pub mod can;

use crate::transfer::TransferMetadata;
use crate::NodeId;
use crate::{RxError, TxError};

pub trait Transport<C: embedded_time::Clock> {
    type Frame;

    const MTU_SIZE: usize;

    const CRC_SIZE: usize;

    /// Check to see if the index for a receiver frame would be a valid index
    fn is_valid_next_index(frame_idx: u32, transfer_idx: u32) -> bool;

    /// Process CRC for the selected data.
    fn update_crc(current_crc: Option<u32>, data: &[u8]) -> u32;

    fn rx_process_frame<'a>(
        frame: &'a Self::Frame,
    ) -> Result<crate::transfer::Frame<'a, C>, RxError>;

    fn transmit_frame(
        metadata: &TransferMetadata<C>,
        data: &[u8],
        node_id: Option<NodeId>,
        timestamp: embedded_time::Instant<C>,
    ) -> Result<(Self::Frame, usize), TxError>;
}
