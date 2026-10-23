use serde::Deserializer;

use super::*;

impl<'de> Deserialize<'de> for License {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }

    fn deserialize_in_place<D>(_deserializer: D, _place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
}

struct LicenseVisitor {}

impl FromStr for License {
    type Err = Infallible;

    fn from_str(_: &str) -> Result<Self, Self::Err> {
        todo!("License::from_str");
    }
}
