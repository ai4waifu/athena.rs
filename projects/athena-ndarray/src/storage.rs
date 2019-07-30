//! 领域无关 storage 合同。

use crate::ArrayError;

/// 存储能力报告。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageCapabilities {
    /// 可写。
    pub writable: bool,
    /// 随机范围读。
    pub random_read: bool,
    /// 顺序读。
    pub sequential_read: bool,
    /// 进程结束后仍存在。
    pub persistent: bool,
}

/// 领域无关的分块存储合同。
pub trait ArrayStorage<T> {
    /// 存储错误。
    type Error;

    /// 逻辑元素个数。
    fn len(&self) -> u64;

    /// 是否为空。
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 能力报告。
    fn capabilities(&self) -> StorageCapabilities;

    /// 读取恰好一个有界区间。
    fn read_range(&self, offset: u64, len: usize) -> Result<Vec<T>, Self::Error>;

    /// 写入恰好一个有界区间。
    fn write_range(&mut self, offset: u64, values: &[T]) -> Result<(), Self::Error>;
}

/// 进程内 [`Vec`] 承载的 storage（小数组 / 测试 / 便利路径，不是规模上限）。
#[derive(Debug, Clone)]
pub struct InMemoryStorage<T> {
    data: Vec<T>,
}

impl<T> InMemoryStorage<T> {
    /// 从已有向量创建。
    pub fn from_vec(data: Vec<T>) -> Self {
        Self { data }
    }

    /// 只读视图。
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
}

impl<T: Clone> ArrayStorage<T> for InMemoryStorage<T> {
    type Error = ArrayError;

    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities { writable: true, random_read: true, sequential_read: true, persistent: false }
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<Vec<T>, Self::Error> {
        let start = usize::try_from(offset).map_err(|_| ArrayError::RangeOverflow)?;
        let end = start.checked_add(len).ok_or(ArrayError::RangeOverflow)?;
        if end > self.data.len() {
            return Err(ArrayError::OutOfBounds);
        }
        Ok(self.data[start..end].to_vec())
    }

    fn write_range(&mut self, offset: u64, values: &[T]) -> Result<(), Self::Error> {
        let start = usize::try_from(offset).map_err(|_| ArrayError::RangeOverflow)?;
        let end = start.checked_add(values.len()).ok_or(ArrayError::RangeOverflow)?;
        if end > self.data.len() {
            return Err(ArrayError::OutOfBounds);
        }
        self.data[start..end].clone_from_slice(values);
        Ok(())
    }
}
