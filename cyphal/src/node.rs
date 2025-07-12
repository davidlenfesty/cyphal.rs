use core::marker::PhantomData;

use core::clone::Clone;

use crate::transfer::manager::{
    CreateTransferError, InternalOrUserError, TokenAccessError, UpdateTransferError,
};
use crate::transfer::{TransferManager, TransferMetadata};
use crate::transport::Transport;
use crate::{RxError, TransferKind, TxError, types::*};

/// Node implementation. Generic across session managers and transport types.
#[derive(Debug, Clone, Copy)]
pub struct Node<M: TransferManager<C, T>, T: Transport<C>, C: embedded_time::Clock> {
    id: Option<NodeId>,

    /// Session manager. Made public so it could be managed by implementation.
    ///
    /// Instead of being public, could be placed behind a `with_session_manager` fn
    /// which took a closure. I can't decide which API is better.
    pub transfer_manager: M,

    _clock: PhantomData<C>,
    _transport: PhantomData<T>,
}

#[derive(Debug, Clone, Copy)]
pub enum TransmitFrameError {
    TokenError(TokenAccessError),
    TxError(TxError),
    /// This indicates an error with the transfer manager implementation,
    /// when there is no access erro but the callback has not been called
    InvalidHandling,
}

impl<'a, M, T, C> Node<M, T, C>
where
    M: TransferManager<C, T>,
    T: Transport<C>,
    C: embedded_time::Clock + Clone,
{
    pub fn new(id: Option<NodeId>, session_manager: M) -> Self {
        Self {
            id,
            transfer_manager: session_manager,
            _clock: PhantomData,
            _transport: PhantomData,
        }
    }

    pub fn try_receive_frame(
        self: &mut Self,
        frame: &T::Frame,
    ) -> Result<Option<M::RxTransferToken>, RxError> {
        let (frame, metadata) = T::rx_process_frame(frame)?;

        // Check if a message is for us
        if let Some(node_id) = frame.metadata.remote_node_id {
            match frame.metadata.transfer_kind {
                TransferKind::Message => {
                    return Err(RxError::MessageWithRemoteId);
                }
                TransferKind::Request | TransferKind::Response => {
                    match self.id {
                        Some(id) => {
                            if node_id != id {
                                // Targeted message, but not for us
                                return Ok(None);
                            }
                        }
                        None => {
                            // Targeted message, but we are anonymous
                            return Ok(None);
                        }
                    }
                }
            }
        }

        // TODO check subscriptions

        match self.transfer_manager.append_frame(&frame, metadata) {
            Ok(tok) => Ok(tok),
            Err(UpdateTransferError::NoSpace) => {
                // TODO should I handle this error explicitly? yes
                Ok(None)
            }
            Err(UpdateTransferError::DoesNotExist) => {
                if !frame.first_frame {
                    return Err(RxError::NewSessionNoStart);
                }

                match self.transfer_manager.new_transfer(&frame) {
                    Ok(tok) => Ok(tok),
                    Err(CreateTransferError::AlreadyExists) => {
                        // This is theoretically unreachable
                        // TODO handle error
                        Ok(None)
                    }
                    Err(CreateTransferError::NoSpace) => {
                        // TODO handle error
                        Ok(None)
                    }
                }
            }
            Err(UpdateTransferError::RxError(e)) => Err(e),
            Err(UpdateTransferError::TimedOut) => Err(RxError::Timeout),
        }
    }

    // TODO implement
    // This needs to take: data, metadata, timestamp
    // Generally I think the API around starting a transfer needs a bit of thought
    pub fn start_tx_transfer<E>(
        &mut self,
        requested_buffer_size: usize,
        metadata: &TransferMetadata<C>,
        cb: impl FnOnce(&mut [u8]) -> Result<usize, E>,
    ) -> Result<M::TxTransferToken, InternalOrUserError<CreateTransferError, E>> {
        self.transfer_manager
            .create_transmission(requested_buffer_size, metadata, cb)
    }

    // TODO users may want a variant of this function that preserves the token
    // so they can peek the transfer metadata for logging
    /// Creates a new frame for the provided transport to provide.
    pub fn transmit_frame(
        &mut self,
        token: M::TxTransferToken,
        // TODO node should hold a clock instance
        timestamp: embedded_time::Instant<C>,
    ) -> Result<(T::Frame, Option<M::TxTransferToken>), TransmitFrameError> {
        let mut frame_out = Err(TransmitFrameError::InvalidHandling);
        let res = M::transmit(
            &mut self.transfer_manager,
            token,
            |transfer_metadata, transport_metadata, data| {
                let frame = T::transmit_frame(
                    transfer_metadata,
                    transport_metadata,
                    data,
                    self.id,
                    timestamp,
                );
                match frame {
                    Ok((frame, consumed)) => {
                        frame_out = Ok(frame);
                        consumed
                    }

                    Err(e) => {
                        frame_out = Err(TransmitFrameError::TxError(e));
                        0
                    }
                }
            },
        );

        match res {
            Ok(token) => {
                match frame_out {
                    Ok(frame) => Ok((frame, token)),
                    // Some TxError occurred, so we can't continue sending things,
                    // clean up.
                    Err(TransmitFrameError::TxError(e)) => {
                        if let Some(token) = token {
                            // Dropping any returned error here, the token should be correct
                            // from the fact we got a transmit error
                            let _ = self.transfer_manager.cancel_tx_transfer(token);
                        }
                        Err(TransmitFrameError::TxError(e))
                    }
                    // Generic error, just return it and move on
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(TransmitFrameError::TokenError(e)),
        }
    }
}
