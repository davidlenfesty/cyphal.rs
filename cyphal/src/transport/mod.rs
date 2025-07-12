//! Transport-specific functionality.
//!
//! The current iteration requires 2 different implementations:
//! - SessionMetadata trait
//! - Transport trait
//!
//! Take a look at the CAN implementation for an example.

// Declaring all of the sub transport modules here.
pub mod can;

use crate::NodeId;
use crate::transfer::{Frame as TransferFrame, TransferMetadata};
use crate::{RxError, TxError};

pub trait Transport<C: embedded_time::Clock> {
    /// Core frame type, that can get received from the link layer.
    type Frame;

    /// Metadata produced by an incoming frame needed to match it up with ongoing transfers.
    type FrameMetadata;

    /// Metadata required to maintain an ongoing TX transfer
    type TxMetadata: Default;
    /// Metadata required to maintain an ongoing RX transfer
    type RxMetadata: Default;

    const MTU_SIZE: usize;

    const CRC_SIZE: usize;

    /// Size of payload after appending CRC and any necessary padding bytes
    fn get_crc_padded_size(requested_size: usize) -> usize;

    /// Update RX metadata for a newly received frame, and check for validity in transfer
    fn update_rx_metadata(
        transport_metadata: &mut Self::RxMetadata,
        frame_metadata: Self::FrameMetadata,
        frame: &TransferFrame<C>,
    ) -> Result<(), RxError>;

    /// Process the entire TX payload CRC, and append CRC with any required padding for this transport
    fn process_tx_crc(buffer: &mut [u8], data_size: usize) -> usize;

    fn rx_process_frame<'a>(
        frame: &'a Self::Frame,
    ) -> Result<(crate::transfer::Frame<'a, C>, Self::FrameMetadata), RxError>;

    fn transmit_frame(
        transfer_metadata: &TransferMetadata<C>,
        transport_metadata: &mut Self::TxMetadata,
        data: &[u8],
        node_id: Option<NodeId>,
        timestamp: embedded_time::Instant<C>,
    ) -> Result<(Self::Frame, usize), TxError>;
}
