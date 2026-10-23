use std::str::FromStr;

use num_bigint::BigInt;
use num_traits::Zero;
use rust_decimal::Decimal;

use crate::error::QResult;
use serde::{Deserialize, Serialize};

use crate::Document;

pub mod decimal_constraint;
pub mod dict_constraint;
pub mod integer_constraint;
pub mod list_constraint;
pub mod string_constraint;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SharedConstraint {
    document: Document,
}
