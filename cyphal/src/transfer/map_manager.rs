use num_traits::real;

use crate::{transport::Transport, types::NodeId};

use super::{
    manager::{
        CreateTransferError, InternalOrUserError, TokenAccessError, TransferManager,
        UpdateTransferError,
    },
    Frame, TransferMetadata,
};

use core::hash;
use std::{collections::HashMap, hash::DefaultHasher, hash::Hasher, hash::Hash};
use std::vec::Vec;

struct RxTransfer<C: embedded_time::Clock, T: Transport<C>> {
    transfer_metadata: TransferMetadata<C>,
    transport_metadata: T::RxMetadata,
    payload: Vec<u8>,
}

struct TxTransfer<C: embedded_time::Clock, T: Transport<C>> {
    transfer_metadata: TransferMetadata<C>,
    transport_metadata: T::TxMetadata,
    consumed: usize,
    payload: Vec<u8>,
}

pub struct MapTransferManager<C: embedded_time::Clock, T: Transport<C>> {
    rx_transfers: HashMap<RxToken, RxTransfer<C, T>>,

    tx_transfers: HashMap<TxToken, TxTransfer<C, T>>,
}

#[derive(Eq, PartialEq, Hash)]
pub struct RxToken(u64);

fn hash_metadata<C: embedded_time::Clock>(metadata: &TransferMetadata<C>) -> u64 {
    let mut hasher = DefaultHasher::new();
    metadata.hash(&mut hasher);
    hasher.finish()
}

#[derive(Eq, PartialEq, Hash)]
pub struct TxToken(u64);

impl<C: embedded_time::Clock, T: Transport<C>> TransferManager<C, T> for MapTransferManager<C, T> {
    type RxTransferToken = RxToken;
    type TxTransferToken = TxToken;

    fn append_frame(
        &mut self,
        frame: &Frame<C>,
    ) -> Result<Option<Self::RxTransferToken>, UpdateTransferError> {
        let token = RxToken(hash_metadata(&frame.metadata));

        match self.rx_transfers.get_mut(&token) {
            Some(rx_transfer) => {
                rx_transfer.payload.extend_from_slice(frame.payload);

                if frame.last_frame {
                    // Return token on completion of transfer
                    Ok(Some(token))
                } else {
                    Ok(None)
                }
            }
            None => Err(UpdateTransferError::DoesNotExist),
        }
    }

    fn new_transfer(
        &mut self,
        frame: &Frame<C>,
    ) -> Result<Option<Self::RxTransferToken>, CreateTransferError> {
        let token = RxToken(hash_metadata(&frame.metadata));

        if let Some(_) = self.rx_transfers.get(&token) {
            return Err(CreateTransferError::AlreadyExists);
        }

        self.rx_transfers.insert(token, RxTransfer {
            transfer_metadata: frame.metadata.clone(),
            transport_metadata: T::RxMetadata::default(),
            payload: Vec::from(frame.payload),
        });

        if frame.last_frame {
            Ok(Some(token))
        } else {
            Ok(None)
        }
    }

    fn with_rx_transfer(
        &mut self,
        token: Self::RxTransferToken,
        cb: impl FnOnce(&super::TransferMetadata<C>, &[u8]),
    ) -> Result<(), TokenAccessError> {
        if let Some(transfer) = self.rx_transfers.get(&token) {
            cb(&transfer.transfer_metadata, &transfer.payload);
        }

        Ok(())
    }

    fn cancel_rx_transfer(
            &mut self,
            token: Self::RxTransferToken,
        ) -> Result<(), TokenAccessError> {
        self.rx_transfers.remove(&token).ok_or(TokenAccessError::InvalidToken).map(|_| ())
    }

    fn cancel_tx_transfer(
            &mut self,
            token: Self::TxTransferToken,
        ) -> Result<(), TokenAccessError> {
        self.tx_transfers.remove(&token).ok_or(TokenAccessError::InvalidToken).map(|_| ())
    }

    fn create_transmission<'a, E>(
        &'a mut self,
        requested_buffer_size: usize,
        metadata: &TransferMetadata<C>,
        cb: impl FnOnce(&'a mut [u8]) -> Result<usize, E>,
    ) -> Result<Self::TxTransferToken, InternalOrUserError<CreateTransferError, E>> {
        let token = TxToken(hash_metadata(metadata));

        if let Some(xfer) = self.tx_transfers.get(token) {
            return Err(InternalOrUserError::InternalError(CreateTransferError::AlreadyExists));
        }

        let final_buf_size = T::get_crc_padded_size(requested_buffer_size);

        let buf = Vec::new();
        buf.resize(final_buf_size, 0u8);

        match cb(&mut buf[0..requested_buffer_size]) {
            Ok(mut consumed) => {
                // Don't let the user screw this up for us
                consumed = std::cmp::min(buf.len(), consumed);


                // Process transport CRC + padding and get the actual payload length
                let real_len = T::process_tx_crc(buf.as_mut_slice(), consumed);

                // Invariant in this specific implementation, we always should be able to allocate the full sized buffer,
                // and our transport should have told us the largest size we needed
                assert!(real_len <= buf.len(), "Transport CRC deleted data!");
                buf.resize(real_len, 0u8);

                let _ = self.tx_transfers.insert(token, TxTransfer {
                    transfer_metadata: metadata.clone(),
                    transport_metadata: T::TxMetadata::default(),
                    consumed: 0usize,
                    payload: buf,
                });

                Ok(token)
            }
            Err(err) => Err(InternalOrUserError::UserError(err)),
        }
    }

    fn transmit(
        &mut self,
        token: Self::TxTransferToken,
        cb: impl FnOnce(&TransferMetadata<C>, &mut T::TxMetadata, &[u8]) -> usize,
    ) -> Result<Option<Self::TxTransferToken>, TokenAccessError> {
        // TODO distinguish timeouts from lack of access, and handle timeouts
        let transfer = self.tx_transfers.get_mut(&token).ok_or(TokenAccessError::InvalidToken)?;

        let consumed = cb(&transfer.transfer_metadata, &mut transfer.transport_metadata, &mut transfer.payload[transfer.consumed..]);
        transfer.consumed += consumed;

        if transfer.consumed >= transfer.payload.len() {
            // Transfer complete
            self.tx_transfers.remove(&token);
            Ok(None)
        } else {
            Ok(Some(token))
        }
    }

    fn update_transfers(&mut self, timestamp: crate::time::Timestamp<C>) {
        todo!()
    }
}
