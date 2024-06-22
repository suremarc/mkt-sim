pub mod client;
pub mod io;

pub use zerocopy;

use std::{fmt::Debug, mem::ManuallyDrop, ptr};

use zerocopy::{
    big_endian::U32, Immutable, IntoBytes, KnownLayout, TryCastError, TryFromBytes, Unaligned,
};

pub type CastResult<'a, T> = Result<&'a T, TryCastError<&'a [u8], T>>;

#[derive(Debug, Clone, Copy, TryFromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C, packed)]
pub struct Message<T: Tag, P: ?Sized + WireFormat = [u8]> {
    pub tag: T,
    pub sequence_number: U32,
    payload: ManuallyDrop<P>,
}

impl<T: Tag, P: ?Sized + WireFormat> WireFormat for Message<T, P> {}

impl<T: Tag, P: ?Sized + WireFormat> Message<T, P> {
    pub fn payload(&self) -> &P {
        // SAFETY:
        // It's safe to create a reference to the (potentially unaligned) field
        // because T: Unaligned implies T has no alignment requirements anyway.
        unsafe { &*ptr::addr_of!(self.payload) }
    }

    pub fn cast<Q: ?Sized + ApplicationLayer>(&self) -> CastResult<Message<T, Q>> {
        Message::<T, Q>::try_ref_from(self.as_bytes())
    }
}

impl<T: Tag, P: ApplicationLayer<Tag = T>> Message<T, P> {
    pub fn new(sequence_number: u32, payload: P) -> Self {
        Self {
            tag: payload.tag(),
            sequence_number: U32::new(sequence_number),
            payload: ManuallyDrop::new(payload),
        }
    }
}

pub trait WireFormat: TryFromBytes + IntoBytes + Immutable + KnownLayout + Unaligned {}

impl WireFormat for [u8] {}

pub trait Tag: Debug + Clone + Copy + WireFormat {}

pub trait ApplicationLayer: WireFormat {
    type Tag: Tag;

    fn tag(&self) -> Self::Tag;
}
