//! CAS arrays and bounded out-of-core storage. This is not a Titan Tensor runtime.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

/// Checked logical shape independent of addressable RAM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalShape {
    dimensions: Vec<u64>,
    elements: u64,
}

impl LogicalShape {
    /// Creates a shape and rejects `u64` element-count overflow.
    pub fn new(dimensions: impl Into<Vec<u64>>) -> Result<Self, ArrayError<std::convert::Infallible>> {
        let dimensions = dimensions.into();
        let elements = dimensions.iter().try_fold(1u64, |n, &d| n.checked_mul(d).ok_or(ArrayError::ShapeOverflow))?;
        Ok(Self { dimensions, elements })
    }
    /// Dimensions.
    pub fn dimensions(&self) -> &[u64] {
        &self.dimensions
    }
    /// Logical element count.
    pub const fn element_count(&self) -> u64 {
        self.elements
    }
}

/// Maximum resident memory for one operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBudget(usize);
impl MemoryBudget {
    /// Creates a non-zero budget in bytes.
    pub fn new(bytes: usize) -> Result<Self, ArrayError<std::convert::Infallible>> {
        if bytes == 0 { Err(ArrayError::ZeroBudget) } else { Ok(Self(bytes)) }
    }
    /// Budget in bytes.
    pub const fn bytes(self) -> usize {
        self.0
    }
}

/// Storage capability report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreCapabilities {
    /// Supports writes.
    pub writable: bool,
    /// Supports random range reads.
    pub random_read: bool,
    /// Survives the process.
    pub persistent: bool,
}

/// Bounded range storage implemented by memory, files, mmap, databases, or object stores.
pub trait ChunkStore<T> {
    /// Store error.
    type Error;
    /// Logical stored elements.
    fn len(&self) -> u64;
    /// Whether the store is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Capabilities.
    fn capabilities(&self) -> StoreCapabilities;
    /// Reads exactly one bounded range.
    fn read_range(&self, offset: u64, len: usize) -> Result<Vec<T>, Self::Error>;
    /// Writes exactly one bounded range.
    fn write_range(&mut self, offset: u64, values: &[T]) -> Result<(), Self::Error>;
}

/// Logical array backed by chunk storage.
#[derive(Debug)]
pub struct ChunkedArray<T, S> {
    shape: LogicalShape,
    store: S,
    budget: MemoryBudget,
    marker: std::marker::PhantomData<T>,
}

impl<T, S: ChunkStore<T>> ChunkedArray<T, S> {
    /// Binds a shape to storage without materializing it.
    pub fn new(shape: LogicalShape, store: S, budget: MemoryBudget) -> Result<Self, ArrayError<S::Error>> {
        if shape.element_count() != store.len() {
            return Err(ArrayError::LengthMismatch { expected: shape.element_count(), actual: store.len() });
        }
        Ok(Self { shape, store, budget, marker: std::marker::PhantomData })
    }
    /// Shape.
    pub const fn shape(&self) -> &LogicalShape {
        &self.shape
    }
    /// Memory budget.
    pub const fn memory_budget(&self) -> MemoryBudget {
        self.budget
    }
    /// Reads a range within budget.
    pub fn read_range(&self, offset: u64, len: usize) -> Result<Vec<T>, ArrayError<S::Error>> {
        self.check(offset, len)?;
        self.store.read_range(offset, len).map_err(ArrayError::Store)
    }
    /// Visits all values in bounded ordered chunks.
    pub fn for_each_chunk(&self, mut visit: impl FnMut(u64, &[T])) -> Result<(), ArrayError<S::Error>> {
        let max = self.max_elements()?;
        let mut offset = 0;
        while offset < self.shape.element_count() {
            let len = usize::try_from((self.shape.element_count() - offset).min(max as u64)).unwrap_or(max);
            let chunk = self.store.read_range(offset, len).map_err(ArrayError::Store)?;
            visit(offset, &chunk);
            offset += len as u64;
        }
        Ok(())
    }
    fn max_elements(&self) -> Result<usize, ArrayError<S::Error>> {
        let size = std::mem::size_of::<T>();
        if size == 0 {
            return Ok(usize::MAX);
        }
        let max = self.budget.bytes() / size;
        if max == 0 { Err(ArrayError::BudgetTooSmall { element_size: size }) } else { Ok(max) }
    }
    fn check(&self, offset: u64, len: usize) -> Result<(), ArrayError<S::Error>> {
        let max = self.max_elements()?;
        if len > max {
            return Err(ArrayError::BudgetExceeded { requested: len, max });
        }
        let len64 = u64::try_from(len).map_err(|_| ArrayError::RangeOverflow)?;
        let end = offset.checked_add(len64).ok_or(ArrayError::RangeOverflow)?;
        if end > self.shape.element_count() { Err(ArrayError::OutOfBounds) } else { Ok(()) }
    }
}

/// Array/storage error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayError<E> {
    /// Shape overflow.
    ShapeOverflow,
    /// Zero memory budget.
    ZeroBudget,
    /// One element exceeds budget.
    BudgetTooSmall {
        /// Element bytes.
        element_size: usize,
    },
    /// Store length mismatch.
    LengthMismatch {
        /// Expected elements.
        expected: u64,
        /// Actual elements.
        actual: u64,
    },
    /// Requested workset exceeds budget.
    BudgetExceeded {
        /// Requested elements.
        requested: usize,
        /// Maximum elements.
        max: usize,
    },
    /// Range arithmetic overflow.
    RangeOverflow,
    /// Range out of bounds.
    OutOfBounds,
    /// Store error.
    Store(E),
}
