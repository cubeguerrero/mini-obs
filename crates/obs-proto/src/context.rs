use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::id::{ParseIdErr, SpanId, TraceId};

pub const TRACEPARENT_HEADER: &str = "traceparent";
pub const TRACE_FLAG_SAMPLED: u8 = 0x01;

#[derive(Debug, PartialEq, Eq)]
pub enum TraceContextErr {
    InvalidFormat,
    UnsupportedVersion,
    InvalidTraceId(ParseIdErr),
    InvalidSpanId(ParseIdErr),
    InvalidFlags,
}

impl Display for TraceContextErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub trace_flags: u8,
}

impl TraceContext {
    pub fn new_root() -> Self {
        Self {
            trace_id: TraceId::generate(),
            span_id: SpanId::generate(),
            trace_flags: TRACE_FLAG_SAMPLED,
        }
    }

    pub fn new_child(&self) -> Self {
        Self {
            trace_id: self.trace_id,
            span_id: SpanId::generate(),
            trace_flags: self.trace_flags,
        }
    }

    pub fn is_sampled(&self) -> bool {
        self.trace_flags & TRACE_FLAG_SAMPLED != 0
    }
}

impl Display for TraceContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "00-{}-{}-{:02x}",
            self.trace_id, self.span_id, self.trace_flags
        )
    }
}

impl FromStr for TraceContext {
    type Err = TraceContextErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 4 {
            return Err(Self::Err::InvalidFormat);
        }

        if parts[0] != "00" {
            return Err(Self::Err::UnsupportedVersion);
        }

        let trace_id = TraceId::from_str(parts[1]).map_err(Self::Err::InvalidTraceId)?;
        let span_id = SpanId::from_str(parts[2]).map_err(Self::Err::InvalidSpanId)?;

        let flags_str = parts[3];
        if flags_str.len() != 2
            || !flags_str.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(Self::Err::InvalidFlags);
        }
        let trace_flags = u8::from_str_radix(flags_str, 16).map_err(|_| Self::Err::InvalidFlags)?;

        Ok(TraceContext {
            trace_id,
            span_id,
            trace_flags,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_traceparent_roundtrip() {
        let raw = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let ctx = TraceContext::from_str(raw).expect("should parse valid traceparent");

        assert_eq!(ctx.trace_id.to_string(), "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(ctx.span_id.to_string(), "00f067aa0ba902b7");
        assert_eq!(ctx.trace_flags, 0x01);
        assert!(ctx.is_sampled());
        assert_eq!(ctx.to_string(), raw);
    }

    #[test]
    fn test_unsampled_traceparent_and_child_inheritance() {
        let raw = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00";
        let parent = TraceContext::from_str(raw).expect("should parse unsampled traceparent");
        assert!(!parent.is_sampled());

        let child = parent.new_child();
        assert_eq!(child.trace_id, parent.trace_id);
        assert_ne!(child.span_id, parent.span_id);
        assert_eq!(child.trace_flags, parent.trace_flags);
        assert!(!child.is_sampled());
    }

    #[test]
    fn test_invalid_traceparent_strs() {
        let invalid = [
            // Too few or too many hyphen-separated parts
            (
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7",
                TraceContextErr::InvalidFormat,
            ),
            (
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-extra",
                TraceContextErr::InvalidFormat,
            ),
            // Unsupported version
            (
                "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
                TraceContextErr::UnsupportedVersion,
            ),
            // All-zero trace_id
            (
                "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
                TraceContextErr::InvalidTraceId(ParseIdErr::AllZeros),
            ),
            // All-zero span_id
            (
                "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
                TraceContextErr::InvalidSpanId(ParseIdErr::AllZeros),
            ),
            // Invalid flags (wrong length or uppercase)
            (
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-1",
                TraceContextErr::InvalidFlags,
            ),
            (
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-0A",
                TraceContextErr::InvalidFlags,
            ),
        ];

        for (s, expected_err) in invalid {
            assert_eq!(TraceContext::from_str(s), Err(expected_err));
        }
    }
}
