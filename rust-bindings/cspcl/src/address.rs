use bytes::Bytes;

use crate::Error::ParseAddress;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CspAddress {
    pub addr: u8,
    pub port: u8,
}

impl TryFrom<Bytes> for CspAddress {
    type Error = crate::Error;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        let mut raw_addr = value.into_iter();
        if raw_addr.len() != 2 {
            return Err(ParseAddress);
        }
        let addr = raw_addr.next().ok_or(ParseAddress)?;
        let port = raw_addr.next().ok_or(ParseAddress)?;
        Ok(Self { addr, port })
    }
}

impl From<CspAddress> for Bytes {
    fn from(val: CspAddress) -> Self {
        Bytes::from(vec![val.addr, val.port])
    }
}
