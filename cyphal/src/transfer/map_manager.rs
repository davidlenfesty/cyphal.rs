use super::{
    manager::{
        CreateTransferError, InternalOrUserError, TokenAccessError, TransferManager,
        UpdateTransferError,
    },
    Frame, TransferMetadata,
};

use std::collections::HashMap;
use std::vec::Vec;

struct RxTransfer<C: embedded_time::Clock> {
    metadata: TransferMetadata<C>,
    payload: Vec<u8>,
}

struct TxTransfer<C: embedded_time::Clock> {
    metadata: TransferMetadata<C>,
    consumed: usize,
    payload: Vec<u8>,
}

pub struct MapTransferManager<C: embedded_time::Clock> {
    // Uses metadata instead of token as the key because the token is only created after the final frame.
    rx_transfers: HashMap<TransferMetadata<C>, RxTransfer<C>>,
    tx_transfers: HashMap<TxToken, TxTransfer<C>>,
}

pub struct RxToken;
pub struct TxToken;

impl<C: embedded_time::Clock> TransferManager<C> for MapTransferManager<C> {
    type RxTransferToken = RxToken;
    type TxTransferToken = TxToken;

    fn append_frame(
        &mut self,
        frame: &Frame<C>,
    ) -> Result<Option<Self::RxTransferToken>, UpdateTransferError> {
        match self.rx_transfers.get_mut(frame.metadata) {
            Some(rx_transfer) => rx_transfer.payload.extend_from_slice(frame.payload),
            None => Err(UpdateTransferError::DoesNotExist),
        }
    }

    fn new_transfer(
        &mut self,
        frame: &Frame<C>,
    ) -> Result<Option<Self::RxTransferToken>, CreateTransferError> {
    }

    fn with_rx_transfer(
        &mut self,
        token: Self::RxTransferToken,
        cb: impl FnOnce(&super::TransferMetadata<C>, &[u8]),
    ) -> Result<(), TokenAccessError> {
    }

    fn create_transmission<'a, T>(
        &'a mut self,
        requested_buffer_size: usize,
        cb: impl FnOnce(&'a mut [u8]) -> Result<usize, T>,
    ) -> Result<Self::TxTransferToken, InternalOrUserError<CreateTransferError, T>> {
    }

    fn transmit(
        &mut self,
        token: Self::TxTransferToken,
        cb: impl FnOnce(&[u8]) -> usize,
    ) -> Result<Option<Self::TxTransferToken>, TokenAccessError> {
    }

    fn update_transfers(&mut self, timestamp: crate::time::Timestamp<C>) {}
}
