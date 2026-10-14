//! Fixed-capacity paged accumulation of large batch inputs.
//!
//! A browser column can be hundreds of megabytes. Handing it over as one
//! typed array forces the host to materialize a second copy of the whole
//! column inside one call. A [`Paged`] reserves the final size once, then
//! accepts bounded pages, so no transfer needs more than one page of extra
//! memory and the finished column is a single contiguous allocation.

use std::fmt;

/// Why a page sequence was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagingError {
    /// The declared total could not be reserved.
    Reserve {
        /// The declared element count.
        total: usize,
    },
    /// A page would exceed the declared total.
    Overflow {
        /// The declared element count.
        total: usize,
    },
    /// The column was finished before every element arrived.
    Incomplete {
        /// Elements received so far.
        received: usize,
        /// The declared element count.
        total: usize,
    },
}

impl fmt::Display for PagingError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Reserve { total } => write!(out, "cannot reserve {total} elements"),
            Self::Overflow { total } => write!(out, "page exceeds the declared {total} elements"),
            Self::Incomplete { received, total } => {
                write!(out, "received {received} of {total} declared elements")
            }
        }
    }
}

impl std::error::Error for PagingError {}

/// One column accumulated page by page against a declared length.
#[derive(Debug)]
pub struct Paged<T> {
    values: Vec<T>,
    total: usize,
}

impl<T: Copy> Paged<T> {
    /// Reserves the full column once.
    ///
    /// # Errors
    /// Returns [`PagingError::Reserve`] when the allocation is refused.
    pub fn new(total: usize) -> Result<Self, PagingError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(total)
            .map_err(|_| PagingError::Reserve { total })?;
        Ok(Self { values, total })
    }

    /// Appends one page.
    ///
    /// # Errors
    /// Returns [`PagingError::Overflow`] when the page passes the declared total.
    pub fn push(&mut self, page: &[T]) -> Result<(), PagingError> {
        if page.len() > self.total - self.values.len() {
            return Err(PagingError::Overflow { total: self.total });
        }
        self.values.extend_from_slice(page);
        Ok(())
    }

    /// Hands over the complete column and leaves this accumulator empty.
    ///
    /// # Errors
    /// Returns [`PagingError::Incomplete`] unless every declared element arrived.
    pub fn finish(&mut self) -> Result<Vec<T>, PagingError> {
        if self.values.len() != self.total {
            return Err(PagingError::Incomplete {
                received: self.values.len(),
                total: self.total,
            });
        }
        self.total = 0;
        Ok(std::mem::take(&mut self.values))
    }

    /// The elements received so far.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.values
    }

    /// Whether every declared element has arrived.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.values.len() == self.total
    }
}
