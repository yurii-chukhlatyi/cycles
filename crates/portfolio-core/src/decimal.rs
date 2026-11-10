use crate::{Error, Result};
use rust_decimal::Decimal;
pub fn add(a: Decimal, b: Decimal) -> Result<Decimal> {
    a.checked_add(b).ok_or(Error::Overflow)
}
pub fn sub(a: Decimal, b: Decimal) -> Result<Decimal> {
    a.checked_sub(b).ok_or(Error::Overflow)
}
pub fn mul(a: Decimal, b: Decimal) -> Result<Decimal> {
    a.checked_mul(b).ok_or(Error::Overflow)
}
pub fn div(a: Decimal, b: Decimal) -> Result<Decimal> {
    a.checked_div(b).ok_or(Error::Overflow)
}
pub fn number(a: Decimal, name: &str, negative: bool) -> Result<()> {
    if a.scale() > 12 || (!negative && a < Decimal::ZERO) {
        return Err(Error::Invalid(format!(
            "{name} must be nonnegative with at most 12 decimal places"
        )));
    }
    Ok(())
}
