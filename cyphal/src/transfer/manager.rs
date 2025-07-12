use crate::RxError;
use crate::time::{Duration, Timestamp};
use crate::transfer::Frame;
use crate::transport::Transport;

use embedded_time::fixed_point::FixedPoint;

use super::TransferMetadata;

// Process incoming frame. This needs to be provided some interface to the listeners to determine if a new transfer should
// be accepted or not. Not implemented as part of the trait, will probably be moved out later, just here to model
// what needs to happen.
// TODO come up with an error type
//fn ingest<'a>(&mut self, check_new_transfer: impl FnOnce(&super::TransferMetadata<C>) -> bool) -> Result<Self::RxTransferToken, ()> {
//    // parse for metadata
//    // if exists, append to exising transfer
//    // if not, try check_new_transfer and if true, add new transfer
//}

pub enum UpdateTransferError {
    /// No space left to create a new transfer instance
    NoSpace,
    /// The transfer does not exist
    DoesNotExist,
    /// Transfer timed out since receiving the previous frame
    TimedOut,
    /// Error in handling reception of an existing transfer
    RxError(RxError),
}

pub enum CreateTransferError {
    /// There is no more memory to create the new transfer
    NoSpace,
    /// A transfer with the same metadata already exists
    AlreadyExists,
}

pub enum TokenAccessError {
    /// Token does not reference an existing transfer
    InvalidToken,
    /// Transfer timed out
    // TODO can this even be distinguished?
    TransferTimeout,
}

pub enum InternalOrUserError<I, U> {
    InternalError(I),
    UserError(U),
}

/// Trait to declare a session manager. This is responsible for managing ongoing transfers.
///
/// The intent here is to provide an interface to easily define
/// what management strategy you want to implement. This allows you to
/// select different models based on e.g. your memory allocation strategy,
/// or if a model provided by this crate does not suffice, you can implement
/// your own.
pub trait TransferManager<C: embedded_time::Clock, T: Transport<C>> {
    // Shouldn't be copy or clone, probably.
    type RxTransferToken;
    type TxTransferToken;

    // TODO now that I am dependent on Transport, I can store transport-specific metadata

    /// Attempt to append a new frame onto an existing transfer, optionally returning a transfer token
    /// if it's the final frame of a multi-frame transfer.
    ///
    /// Note that this will be called for every incoming frame, it is expected to
    /// discriminate against new transfers.
    fn append_frame(
        &mut self,
        frame: &Frame<C>,
        metadata: &T::FrameMetadata,
    ) -> Result<Option<Self::RxTransferToken>, UpdateTransferError>;

    /// Create a new transfer from the incoming frame, optionally returning a transfer token if it's a single-frame
    /// transfer.
    ///
    /// Note that this will always be called *after* determining a frame is:
    /// - Not part of an existing transfer, and
    /// - Expected by the user, and
    /// - the first frame
    ///
    /// So implementations do not need to check for the existance of a transfer.
    fn new_transfer(
        &mut self,
        frame: &Frame<C>,
        metadata: &T::FrameMetadata,
    ) -> Result<Option<Self::RxTransferToken>, CreateTransferError>;

    /// Provides read access into the transfer payload to the user's calback, consuming the RX token.
    ///
    /// The token is consumed, so implementations are expected to free the transfer's memory. In the future there may be
    /// a non-consuming method for this, to peek at the buffer but keep it allocated.
    // TODO maybe I want to return the error inside the callback instead of at the outer layer.
    fn with_rx_transfer(
        &mut self,
        token: Self::RxTransferToken,
        cb: impl FnOnce(&super::TransferMetadata<C>, &[u8]),
    ) -> Result<(), TokenAccessError>;

    fn cancel_rx_transfer(&mut self, token: Self::RxTransferToken) -> Result<(), TokenAccessError>;

    /// Allocates new space for a TX transfer, providing reference to a buffer to write the payload into
    /// to a user provided callback that either returns an error or the amount of buffer used.
    ///
    /// The implementation may choose to provide a smaller or larger buffer to the user, as serialized DSDL types may take up
    /// less space than their extent, but in that case it is up to the user to handle errors if the buffer is too
    /// small for their purposes. The alternative is to simply return the "out of space" error.
    ///
    /// The transfer metadata is not always needed, but implementations may use it for uniquely identifying tokens
    fn create_transmission<'a, E>(
        &'a mut self,
        requested_buffer_size: usize,
        metadata: &TransferMetadata<C>,
        cb: impl FnOnce(&'a mut [u8]) -> Result<usize, E>,
    ) -> Result<Self::TxTransferToken, InternalOrUserError<CreateTransferError, E>>;

    /// Provides the user access into the transfer payload for the purposes of generating a transport-specific frame and transmitting
    /// it. Returns a new transfer token if the user has not yet consumed the entire payload.
    ///
    /// The callback will return the number of bytes consumed, which must be used to increment the payload slice provided
    /// to the user. The user may consume 0 bytes (such as when the transmit buffer is full)
    fn transmit(
        &mut self,
        token: Self::TxTransferToken,
        cb: impl FnOnce(&TransferMetadata<C>, &mut T::TxMetadata, &[u8]) -> usize,
    ) -> Result<Option<Self::TxTransferToken>, TokenAccessError>;

    fn cancel_tx_transfer(&mut self, token: Self::TxTransferToken) -> Result<(), TokenAccessError>;

    // TODO may want to add more hooks for transfer cleanup to allow users to check metadata of published transfers
    // and not just fail blindly

    /// Housekeeping function called to clean up timed-out transfers
    ///
    /// Note: an implementation is expected to also clean up complete transfers after some period,
    /// or it will be possible for the user to not clear out a transfer via usage.
    fn update_transfers(&mut self, timestamp: Timestamp<C>, timeout: Duration);
}

pub fn timestamp_expired<C: embedded_time::Clock, D>(
    timeout: D,
    now: Timestamp<C>,
    then: Option<Timestamp<C>>,
) -> bool
where
    D: embedded_time::duration::Duration + FixedPoint,
    <C as embedded_time::Clock>::T: From<<D as FixedPoint>::T>,
{
    if let Some(then) = then {
        if now - then > timeout.to_generic(C::SCALING_FACTOR).unwrap() {
            return true;
        }
    }

    false
}
