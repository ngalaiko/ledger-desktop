use std::fmt;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize, schemars::JsonSchema,
)]
pub enum Period {
    Week,
    Month,
    Year,
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Period::Week => "Week",
            Period::Month => "Month",
            Period::Year => "Year",
        };
        write!(f, "{s}")
    }
}
