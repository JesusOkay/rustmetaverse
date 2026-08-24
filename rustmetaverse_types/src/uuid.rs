use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct UUID(pub Uuid);

impl UUID {
    pub const ZERO: UUID = UUID(Uuid::nil());

    pub fn new() -> Self {
        UUID(Uuid::new_v4())
    }

    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Ok(UUID(Uuid::parse_str(s)?))
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        UUID(Uuid::from_bytes(bytes))
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_nil()
    }
}

impl fmt::Display for UUID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for UUID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for UUID {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(UUID(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for UUID {
    fn from(uuid: Uuid) -> Self {
        UUID(uuid)
    }
}

impl From<UUID> for Uuid {
    fn from(val: UUID) -> Self {
        val.0
    }
}
