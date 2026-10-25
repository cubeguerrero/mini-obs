use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq)]
pub enum ParseIdErr {
    InvalidLength,
    InvalidHex,
    AllZeros,
}

impl Display for ParseIdErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraceId(pub u128);

impl TraceId {
    pub fn generate() -> Self {
        loop {
            let val = rand::random::<u128>();
            if val != 0 {
                return Self(val);
            }
        }
    }
}

impl Display for TraceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

impl FromStr for TraceId {
    type Err = ParseIdErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 32 {
            return Err(Self::Err::InvalidLength);
        }

        if !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(Self::Err::InvalidHex);
        }

        let val = u128::from_str_radix(s, 16).map_err(|_| Self::Err::InvalidHex)?;

        if val == 0 {
            return Err(Self::Err::AllZeros);
        }

        Ok(TraceId(val))
    }
}

impl Serialize for TraceId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TraceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpanId(pub u64);

impl SpanId {
    pub fn generate() -> Self {
        loop {
            let val = rand::random::<u64>();
            if val != 0 {
                return Self(val);
            }
        }
    }
}

impl Display for SpanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl FromStr for SpanId {
    type Err = ParseIdErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 16 {
            return Err(Self::Err::InvalidLength);
        }

        if !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(Self::Err::InvalidHex);
        }

        let val = u64::from_str_radix(s, 16).map_err(|_| Self::Err::InvalidHex)?;

        if val == 0 {
            return Err(Self::Err::AllZeros);
        }

        Ok(SpanId(val))
    }
}

impl Serialize for SpanId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SpanId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_trace_id_strs() {
        let valid = [
            "4bf92f3577b34da6a3ce929d0e0e4736",
            "00000000000000000000000000000001",
            "ffffffffffffffffffffffffffffffff",
        ];

        for s in valid {
            let id = TraceId::from_str(s).expect("should parse valid trace_id");
            assert_eq!(id.to_string(), s);
        }
    }

    #[test]
    fn test_invalid_trace_id_strs() {
        let invalid = [
            ("4bf92f3577b34da6a3ce929d0e0e473", ParseIdErr::InvalidLength),
            (
                "4bf92f3577b34da6a3ce929d0e0e47361",
                ParseIdErr::InvalidLength,
            ),
            ("", ParseIdErr::InvalidLength),
            ("4bf92f3577b34da6a3ce929d0e0e473g", ParseIdErr::InvalidHex),
            ("4BF92F3577B34DA6A3CE929D0E0E4736", ParseIdErr::InvalidHex),
            ("4bf92f35-77b3-4da6-a3ce-929d0e0e", ParseIdErr::InvalidHex),
            ("4bf92f3577b34da6a3ce929d0e0e473*", ParseIdErr::InvalidHex),
            ("00000000000000000000000000000000", ParseIdErr::AllZeros),
        ];

        for (s, expected_err) in invalid {
            assert_eq!(TraceId::from_str(s), Err(expected_err));
        }
    }

    #[test]
    fn test_trace_id_json_roundtrip() {
        let raw_hex = "4bf92f3577b34da6a3ce929d0e0e4736";
        let id = TraceId::from_str(raw_hex).unwrap();

        // 1. Serialize to JSON string
        let json = serde_json::to_string(&id).expect("should serialize to JSON");
        assert_eq!(json, r#""4bf92f3577b34da6a3ce929d0e0e4736""#);

        // 2. Deserialize back from JSON string
        let decoded: TraceId = serde_json::from_str(&json).expect("should deserialize from JSON");
        assert_eq!(decoded, id);

        // 3. Invalid hex inside JSON should fail deserialization
        let bad_json = r#""00000000000000000000000000000000""#;
        assert!(serde_json::from_str::<TraceId>(bad_json).is_err());
    }

    #[test]
    fn test_valid_span_id_strs() {
        let valid = ["00f067aa0ba902b7", "0000000000000001", "ffffffffffffffff"];

        for s in valid {
            let id = SpanId::from_str(s).expect("should parse valid trace_id");
            assert_eq!(id.to_string(), s);
        }
    }

    #[test]
    fn test_invalid_span_id_strs() {
        let invalid = [
            ("00f067aa0ba902b", ParseIdErr::InvalidLength),
            ("00f067aa0ba902b71", ParseIdErr::InvalidLength),
            ("", ParseIdErr::InvalidLength),
            ("00f067aa0ba902bg", ParseIdErr::InvalidHex),
            ("00F067AA0BA902B7", ParseIdErr::InvalidHex),
            ("00f067a-0ba902b7", ParseIdErr::InvalidHex),
            ("0000000000000000", ParseIdErr::AllZeros),
        ];

        for (s, expected_err) in invalid {
            assert_eq!(SpanId::from_str(s), Err(expected_err));
        }
    }

    #[test]
    fn test_span_id_json_roundtrip() {
        let raw_hex = "00f067aa0ba902b7";
        let id = SpanId::from_str(raw_hex).unwrap();

        // 1. Serialize to JSON string
        let json = serde_json::to_string(&id).expect("should serialize to JSON");
        assert_eq!(json, r#""00f067aa0ba902b7""#);

        // 2. Deserialize back from JSON string
        let decoded: SpanId = serde_json::from_str(&json).expect("should deserialize from JSON");
        assert_eq!(decoded, id);

        // 3. Invalid hex inside JSON should fail deserialization
        let bad_json = r#""0000000000000000""#;
        assert!(serde_json::from_str::<SpanId>(bad_json).is_err());
    }
}
