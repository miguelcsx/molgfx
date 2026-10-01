//! Timestamp availability is independent of completed-output evidence.

/// A GPU interval, or the explicit reason it cannot be reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuTiming {
    /// Resolved duration in nanoseconds; zero is a valid measured duration.
    Resolved(u64),
    /// The adapter does not support timestamp queries.
    Unsupported,
    /// The backend returned its unresolved zero sentinel.
    Unresolved,
    /// A query slot retained timestamp ticks from an earlier completed output.
    Stale,
    /// The end timestamp precedes the start timestamp.
    InvalidOrder,
    /// The timestamp payload is incomplete.
    Malformed,
    /// The timestamp period cannot yield a finite nonnegative duration.
    InvalidPeriod,
    /// Caller-authored timestamp descriptors prevented full capture coverage.
    DescriptorWrites,
    /// Actual pass count exceeded the bounded capture capacity.
    CapacityExceeded,
}

impl GpuTiming {
    /// Returns only measured durations, never a placeholder zero.
    #[must_use]
    pub const fn nanoseconds(self) -> Option<u64> {
        match self {
            Self::Resolved(value) => Some(value),
            _ => None,
        }
    }

    /// Stable machine-readable absence reason.
    #[must_use]
    pub const fn unavailable_reason(self) -> Option<&'static str> {
        match self {
            Self::Resolved(_) => None,
            Self::Unsupported => Some("unsupported"),
            Self::Unresolved => Some("unresolved"),
            Self::Stale => Some("stale"),
            Self::InvalidOrder => Some("invalid_order"),
            Self::Malformed => Some("malformed"),
            Self::InvalidPeriod => Some("invalid_period"),
            Self::DescriptorWrites => Some("descriptor_writes"),
            Self::CapacityExceeded => Some("capacity_exceeded"),
        }
    }

    pub(super) fn from_timestamps(
        start: u64,
        end: u64,
        period: f32,
    ) -> Result<Self, crate::RenderError> {
        if start == 0 || end == 0 {
            return Ok(Self::Unresolved);
        }
        let Some(ticks) = end.checked_sub(start) else {
            return Ok(Self::InvalidOrder);
        };
        if !period.is_finite() || period <= 0.0 {
            return Ok(Self::InvalidPeriod);
        }
        let remainder =
            u32::try_from(ticks % 1_000_000_000).map_err(|_| molgfx_gpu::GpuError::DeviceLost)?;
        let seconds = std::time::Duration::new(ticks / 1_000_000_000, remainder).as_secs_f64()
            * f64::from(period);
        let Ok(duration) = std::time::Duration::try_from_secs_f64(seconds) else {
            return Ok(Self::InvalidPeriod);
        };
        Ok(Self::Resolved(crate::fallback(
            u64::try_from(duration.as_nanos()),
            u64::MAX,
        )))
    }
}
