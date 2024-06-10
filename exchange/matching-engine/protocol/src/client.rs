use std::num::NonZeroU64;

use zerocopy::{
    big_endian::{U128, U64},
    Immutable, IntoBytes, KnownLayout, TryFromBytes, Unaligned,
};

use crate::{Tag, WireFormat};

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

impl WireFormat for OrderAck {}

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

#[derive(Debug, Clone, Copy, TryFromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C, packed)]
pub struct OrderAck {
    pub user: U128,
    pub id: U128,
    pub timestamp: U64,
}
