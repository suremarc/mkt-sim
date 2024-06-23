use std::num::NonZeroU64;

use zerocopy::{
    big_endian::{U128, U64},
    Immutable, IntoBytes, KnownLayout, TryFromBytes, Unaligned,
};

use crate::{ApplicationLayer, Tag, WireFormat};

#[derive(Debug, Clone, Copy, TryFromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
#[repr(u8)]
pub enum RequestKind {
    NewOrder,
    CancelOrder,
}

impl WireFormat for RequestKind {}
impl Tag for RequestKind {}

#[derive(Debug, Clone, Copy, TryFromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
#[repr(u8)]
pub enum ResponseKind {
    NewOrderAck,
    CancelOrderAck,
    Error,
}

impl WireFormat for ResponseKind {}
impl Tag for ResponseKind {}

#[derive(Debug, Clone, Copy, TryFromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C, packed)]
pub struct Order {
    pub user: U128,
    pub side: Side,
    pub price: Option<NonZeroU64>,
}

impl WireFormat for Order {}

#[derive(Debug, Clone, Copy, TryFromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
#[repr(u8)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, TryFromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C, packed)]
pub struct CancelOrder {
    pub id: U128,
}

impl WireFormat for CancelOrder {}

#[derive(Debug, Clone, Copy, TryFromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C, packed)]
pub struct NewOrderAck {
    pub user: U128,
    pub id: U128,
    pub timestamp: U64,
}

impl WireFormat for NewOrderAck {}
impl ApplicationLayer for NewOrderAck {
    type Tag = ResponseKind;

    fn tag(&self) -> Self::Tag {
        ResponseKind::NewOrderAck
    }
}

#[derive(Debug, Clone, Copy, TryFromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
#[repr(u8)]
pub enum ErrorKind {
    Invalid,
    UnexpectedSequenceNumber,
}

impl WireFormat for ErrorKind {}
impl ApplicationLayer for ErrorKind {
    type Tag = ResponseKind;

    fn tag(&self) -> Self::Tag {
        ResponseKind::Error
    }
}
