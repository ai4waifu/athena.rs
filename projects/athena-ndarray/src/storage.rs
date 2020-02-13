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
///
/// Owning 读出的复制策略由实现决定（POD / GC `try_clone_in` 等）。
/// **不得**要求元素实现 Rust [`Clone`] 才能接入本 trait；便利 [`InMemoryStorage`] 对 POD 另有 `T: Clone` bound。
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

    /// 读取恰好一个有界区间（owning）。
    fn read_range(&self, offset: u64, len: usize) -> Result<Vec<T>, Self::Error>;

    /// 写入恰好一个有界区间。
    fn write_range(&mut self, offset: u64, values: &[T]) -> Result<(), Self::Error>;
}

/// 进程内 [`Vec`] 承载的 storage（小数组 / 测试 / POD 便利路径，不是规模上限）。
///
/// 构造移动、热路径优先 [`Self::as_slice`]。`ArrayStorage` 的 owning 读对 POD 经 [`Clone`]（与 `Copy` 同代价），
/// 非 POD / GC 值类型应自建 [`ArrayStorage`]（例如 engine 内经 `try_clone_in`），不要在此硬套 Rust [`Clone`]。
#[derive(Debug)]
pub struct InMemoryStorage<T> {
    data: Vec<T>,
}

impl<T> InMemoryStorage<T> {
    /// 从已有向量创建（移动）。
    pub fn from_vec(data: Vec<T>) -> Self {
        Self { data }
    }

    /// 只读连续视图（零拷贝）。
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    /// 可变连续视图（零拷贝）。
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        &mut self.data
    }

    /// 取出内部向量。
    pub fn into_vec(self) -> Vec<T> {
        self.data
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
