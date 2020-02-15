//! `Number` 的 ndarray storage：owning 复制走 GC [`Number::try_clone_in`]，不用 Rust [`Clone`]。

use athena_ndarray::{ArrayError, ArrayStorage, StorageCapabilities};
use athena_numeric::{Number, NumericContext};

use super::numeric_clone::clone_number;

/// 进程内 `Number` 行主序缓冲（小矩阵 / F4 Macaulay 便利路径）。
#[derive(Debug)]
pub struct NumberInMemoryStorage {
    data: Vec<Number>,
}

impl NumberInMemoryStorage {
    /// 移动构造。
    pub fn from_vec(data: Vec<Number>) -> Self {
        Self { data }
    }

    /// 零填充构造。
    pub fn zeros(len: usize) -> Self {
        Self { data: (0..len).map(|_| Number::small_int(0)).collect() }
    }

    /// 只读切片（零拷贝）。
    pub fn as_slice(&self) -> &[Number] {
        &self.data
    }

    /// 可变切片（零拷贝）。
    pub fn as_slice_mut(&mut self) -> &mut [Number] {
        &mut self.data
    }

    /// 取出缓冲。
    pub fn into_vec(self) -> Vec<Number> {
        self.data
    }

    fn dup(n: &Number) -> Result<Number, ArrayError> {
        n.try_clone_in(&NumericContext::portable_default()).map_err(|_| ArrayError::Store)
    }
}

impl ArrayStorage<Number> for NumberInMemoryStorage {
    type Error = ArrayError;

    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities { writable: true, random_read: true, sequential_read: true, persistent: false }
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<Vec<Number>, Self::Error> {
        let start = usize::try_from(offset).map_err(|_| ArrayError::RangeOverflow)?;
        let end = start.checked_add(len).ok_or(ArrayError::RangeOverflow)?;
        if end > self.data.len() {
            return Err(ArrayError::OutOfBounds);
        }
        self.data[start..end].iter().map(Self::dup).collect()
    }

    fn write_range(&mut self, offset: u64, values: &[Number]) -> Result<(), Self::Error> {
        let start = usize::try_from(offset).map_err(|_| ArrayError::RangeOverflow)?;
        let end = start.checked_add(values.len()).ok_or(ArrayError::RangeOverflow)?;
        if end > self.data.len() {
            return Err(ArrayError::OutOfBounds);
        }
        for (dst, src) in self.data[start..end].iter_mut().zip(values.iter()) {
            *dst = Self::dup(src)?;
        }
        Ok(())
    }
}

/// 经 portable context 复制单个 [`Number`]（与 [`clone_number`] 同合同）。
pub fn dup_number(n: &Number) -> Number {
    clone_number(n)
}
