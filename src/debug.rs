use crate::prelude::*;

pub fn main() -> Result<()> {
    warn!("{:#?}", gctx().config);
    Ok(())
}
