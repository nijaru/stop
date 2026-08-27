use super::PortScan;
use crate::error::StopError;

pub(super) fn scan(_port: u16) -> Result<PortScan, StopError> {
    Err(StopError::UnsupportedPlatform)
}
