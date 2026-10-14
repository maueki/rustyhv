use crate::app_api::{self, Error};

pub(super) fn run() -> Result<(), Error> {
    app_api::write_str("Hello from app!\r\n")
}
