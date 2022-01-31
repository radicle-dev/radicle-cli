use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;

pub fn parse_value<T: FromStr>(flag: &str, value: OsString) -> anyhow::Result<T>
where
    <T as FromStr>::Err: std::error::Error,
{
    value
        .into_string()
        .map_err(|_| anyhow!("the value specified for '--{}' is not valid unicode", flag))?
        .parse()
        .map_err(|e| anyhow!("invalid value specified for '--{}' ({})", flag, e))
}

pub fn format(arg: lexopt::Arg) -> OsString {
    match arg {
        lexopt::Arg::Long(flag) => format!("--{}", flag).into(),
        lexopt::Arg::Short(flag) => format!("-{}", flag).into(),
        lexopt::Arg::Value(val) => val,
    }
}
