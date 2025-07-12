//! UAVCAN/CAN transport implementation.
//!
//! CAN will essentially be the "reference implementation", and *should* always follow
//! the best practices, so if you want to add support for a new transport, you should
//! follow the conventions here.

use arrayvec::ArrayVec;
use embedded_can::ExtendedId;
use num_traits::FromPrimitive;

use super::bitfields::*;
use crate::time::Timestamp;
use crate::transfer::{Frame, TransferMetadata};
use crate::transport::Transport;
use crate::{NodeId, Priority, RxError, TransferKind, TxError};

use crc_any::CRCu16;

/// Unit struct for declaring transport type
#[derive(Copy, Clone, Debug)]
pub struct Can;

pub struct FrameMetadata {
    pub toggle_bit: bool,
}

pub struct TxMetadata {
    first_frame: bool,
    toggle_bit: bool,
}

impl Default for TxMetadata {
    fn default() -> Self {
        return Self {
            first_frame: false,
            // Protocol version states SOT must have toggle set
            toggle_bit: true,
        };
    }
}

pub struct RxMetadata {
    crc: CRCu16,
    toggle_bit: bool,
}

impl Default for RxMetadata {
    fn default() -> Self {
        return Self {
            crc: CRCu16::crc16ccitt_false(),

            // Invert initial toggle bit, so when we check the first frame it works if it's set
            toggle_bit: false,
        };
    }
}

impl<C: embedded_time::Clock> Transport<C> for Can {
    type Frame = CanFrame<C>;
    type FrameMetadata = FrameMetadata;
    type RxMetadata = RxMetadata;
    type TxMetadata = TxMetadata;

    const MTU_SIZE: usize = 8;
    const CRC_SIZE: usize = 2;

    fn get_crc_padded_size(requested_size: usize) -> usize {
        // Just need to include CRC16
        return requested_size + 2;
    }

    fn update_rx_metadata(
        transport_metadata: &mut Self::RxMetadata,
        frame_metadata: Self::FrameMetadata,
        frame: &crate::transfer::Frame<C>,
    ) -> Result<(), RxError> {
        // Check for issues
        if frame_metadata.toggle_bit == transport_metadata.toggle_bit {
            return Err(RxError::InvalidFrameOrdering);
        }

        // update metadata
        transport_metadata.toggle_bit = frame_metadata.toggle_bit;
        transport_metadata.crc.digest(frame.payload);

        Ok(())
    }

    fn process_tx_crc(buffer: &mut [u8], data_size: usize) -> usize {
        let mut crc = CRCu16::crc16ccitt_false();
        crc.digest(&buffer[0..data_size]);

        // Append CRC
        // TODO endianness may be wrong
        let crc = crc.get_crc();
        buffer[data_size] = (crc & 0x00FF) as u8;
        buffer[data_size + 1] = (crc & 0xFF00 >> 8) as u8;

        data_size + 2
    }

    fn rx_process_frame<'a>(
        frame: &'a Self::Frame,
    ) -> Result<(crate::transfer::Frame<'a, C>, Self::FrameMetadata), RxError> {
        // Frames cannot be empty. They must at least have a tail byte.
        // NOTE: libcanard specifies this as only for multi-frame transfers but uses
        // this logic.
        if frame.payload.is_empty() {
            return Err(RxError::FrameEmpty);
        }

        // Pull tail byte from payload
        let tail_byte = TailByte(*frame.payload.last().unwrap());

        // Protocol version states SOT must have toggle set
        if tail_byte.start_of_transfer() && !tail_byte.toggle() {
            return Err(RxError::TransferStartMissingToggle);
        }
        // Non-last frames must use the MTU fully
        if !tail_byte.end_of_transfer() && frame.payload.len() < <Self as Transport<C>>::MTU_SIZE {
            return Err(RxError::NonLastUnderUtilization);
        }

        let frame_metadata = FrameMetadata {
            toggle_bit: tail_byte.toggle(),
        };

        if CanServiceId(frame.id.as_raw()).is_svc() {
            // Handle services
            let id = CanServiceId(frame.id.as_raw());

            // Ignore invalid frames
            if !id.valid() {
                return Err(RxError::InvalidCanId);
            }

            let transfer_kind = if id.is_req() {
                TransferKind::Request
            } else {
                TransferKind::Response
            };

            return Ok((
                Frame {
                    metadata: TransferMetadata {
                        timestamp: frame.timestamp,
                        priority: Priority::from_u8(id.priority()).unwrap(),
                        transfer_kind,
                        port_id: id.service_id(),
                        remote_node_id: Some(id.source_id()),
                        transfer_id: tail_byte.transfer_id(),
                    },

                    payload: &frame.payload[0..frame.payload.len() - 1],
                    first_frame: tail_byte.start_of_transfer(),
                    last_frame: tail_byte.end_of_transfer(),
                },
                frame_metadata,
            ));
        } else {
            // Handle messages
            let id = CanMessageId(frame.id.as_raw());

            // We can ignore ID in anonymous transfers
            let source_node_id = if id.is_anon() {
                // Anonymous transfers can only be single-frame transfers
                if !(tail_byte.start_of_transfer() && tail_byte.end_of_transfer()) {
                    return Err(RxError::AnonNotSingleFrame);
                }

                None
            } else {
                Some(id.source_id())
            };

            if !id.valid() {
                return Err(RxError::InvalidCanId);
            }

            return Ok((
                Frame {
                    metadata: TransferMetadata {
                        timestamp: frame.timestamp,
                        priority: Priority::from_u8(id.priority()).unwrap(),
                        transfer_kind: TransferKind::Message,
                        port_id: id.subject_id(),
                        remote_node_id: source_node_id,
                        transfer_id: tail_byte.transfer_id(),
                    },

                    payload: &frame.payload[0..frame.payload.len() - 1],
                    first_frame: tail_byte.start_of_transfer(),
                    last_frame: tail_byte.end_of_transfer(),
                },
                frame_metadata,
            ));
        }
    }

    fn transmit_frame(
        transfer_metadata: &TransferMetadata<C>,
        transport_metadata: &mut Self::TxMetadata,
        data: &[u8],
        node_id: Option<NodeId>,
        timestamp: embedded_time::Instant<C>,
    ) -> Result<(Self::Frame, usize), TxError> {
        // CRC included in data, calculated when creating a TX transfer
        let first_frame = transport_metadata.first_frame;
        let last_frame = data.len() <= 7;
        let toggle_bit = transport_metadata.toggle_bit;

        // Update metadata
        transport_metadata.first_frame = false;
        transport_metadata.toggle_bit = !toggle_bit;

        // Build CAN ID from transfer metadata
        let frame_id = match transfer_metadata.transfer_kind {
            TransferKind::Message => {
                if !last_frame && node_id.is_none() {
                    return Err(TxError::AnonNotSingleFrame);
                }

                CanMessageId::new(
                    transfer_metadata.priority,
                    transfer_metadata.port_id,
                    node_id,
                )
            }
            TransferKind::Request | TransferKind::Response => {
                let source = node_id.ok_or(TxError::ServiceNoSourceID)?;
                let destination = transfer_metadata
                    .remote_node_id
                    .ok_or(TxError::ServiceNoDestinationID)?;
                CanServiceId::new(
                    transfer_metadata.priority,
                    transfer_metadata.transfer_kind == TransferKind::Request,
                    transfer_metadata.port_id,
                    destination,
                    source,
                )
            }
        };

        // Build tail byte from metadata
        let tail_byte = TailByte::new(
            first_frame,
            last_frame,
            toggle_bit,
            transfer_metadata.transfer_id,
        );

        let consume_len = core::cmp::min(7, data.len());
        let mut payload = ArrayVec::from_iter(data[0..consume_len].iter().copied());
        // SAFETY, length of data in payload ensured to be 7 or less
        unsafe {
            payload.push_unchecked(tail_byte.0);
        }

        Ok((
            Self::Frame {
                timestamp,
                id: frame_id,
                payload,
            },
            consume_len,
        ))
    }
}

// TODO convert to embedded-hal PR type
/// Extended CAN frame (the only one supported by UAVCAN/CAN)
#[derive(Clone, Debug)]
pub struct CanFrame<C: embedded_time::Clock> {
    pub timestamp: Timestamp<C>,
    pub id: ExtendedId,
    pub payload: ArrayVec<[u8; 8]>,
}

impl<C: embedded_time::Clock> CanFrame<C> {
    pub fn new(timestamp: Timestamp<C>, id: u32, data: &[u8]) -> Self {
        Self {
            timestamp,
            // TODO get rid of this expect, it probably isn't necessary, just added quickly
            id: ExtendedId::new(id).expect("invalid ID"),
            payload: ArrayVec::<[u8; 8]>::from_iter(data.iter().copied()),
        }
    }
}
