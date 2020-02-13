//! 逻辑数组与有界分块访问。

use std::cell::RefCell;

use crate::{
    ArrayError, ArrayStorage, ChunkPlan, InMemoryStorage, LogicalShape, MemoryBudget,
    budget::{BudgetLedger, ChunkGuard},
};

/// 由分块存储承载的逻辑数组（不要求整表驻留 RAM）。
#[derive(Debug)]
pub struct ChunkedArray<T, S> {
    shape: LogicalShape,
    store: S,
    budget: MemoryBudget,
    ledger: RefCell<BudgetLedger>,
    marker: std::marker::PhantomData<T>,
}

/// 首轮 `Array` 合同别名：存储后端 逻辑数组。
pub type Array<T, S> = ChunkedArray<T, S>;

/// 行主序稠密二维数组别名（`shape = [nrows, ncols]`）。
pub type Array2d<T, S> = ChunkedArray<T, S>;

/// 内存驻留的只读连续视图（小数组便利路径，不是规模上限）。
#[derive(Debug, Clone, Copy)]
pub struct ArrayView<'a, T> {
    shape: &'a LogicalShape,
    data: &'a [T],
}

impl<'a, T> ArrayView<'a, T> {
    /// 创建视图；长度必须匹配 shape。
    pub fn new(shape: &'a LogicalShape, data: &'a [T]) -> Result<Self, ArrayError> {
        if data.len() as u64 != shape.element_count() {
            return Err(ArrayError::LengthMismatch { expected: shape.element_count(), actual: data.len() as u64 });
        }
        Ok(Self { shape, data })
    }

    /// Shape。
    pub const fn shape(&self) -> &LogicalShape {
        self.shape
    }

    /// 连续元素切片。
    pub const fn as_slice(&self) -> &[T] {
        self.data
    }
}

impl<T, S: ArrayStorage<T>> ChunkedArray<T, S> {
    /// 绑定 shape 与 storage，不物化全量数据。
    pub fn new(shape: LogicalShape, store: S, budget: MemoryBudget) -> Result<Self, ArrayError> {
        if shape.element_count() != store.len() {
            return Err(ArrayError::LengthMismatch { expected: shape.element_count(), actual: store.len() });
        }
        Ok(Self { shape, store, budget, ledger: RefCell::new(BudgetLedger::new()), marker: std::marker::PhantomData })
    }

    /// Shape。
    pub const fn shape(&self) -> &LogicalShape {
        &self.shape
    }

    /// 底层 storage（只读）。
    pub const fn store(&self) -> &S {
        &self.store
    }

    /// 底层 storage（可变）。
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    /// Memory budget。
    pub const fn memory_budget(&self) -> MemoryBudget {
        self.budget
    }

    /// 预算账本快照。
    pub fn ledger_snapshot(&self) -> BudgetLedger {
        *self.ledger.borrow()
    }

    /// 针对全数组生成分块计划。
    pub fn chunk_plan(&self) -> Result<ChunkPlan, ArrayError> {
        let max = self.max_elements()?;
        ChunkPlan::new(0, self.shape.element_count(), max)
    }

    /// 在预算内读取区间（[`ChunkGuard`] 占用 resident + open_chunks）。
    pub fn read_range(&self, offset: u64, len: usize) -> Result<Vec<T>, ArrayError> {
        self.check(offset, len)?;
        let elem_size = std::mem::size_of::<T>();
        let mut ledger = self.ledger.borrow_mut();
        let _guard = ChunkGuard::acquire(&mut ledger, self.budget, elem_size, len)?;
        self.store.read_range(offset, len).map_err(|_| ArrayError::Store)
    }

    /// 按有界 chunk 顺序访问全部元素（禁止一次整表加载）。
    pub fn for_each_chunk(&self, mut visit: impl FnMut(u64, &[T])) -> Result<(), ArrayError> {
        let plan = self.chunk_plan()?;
        let mut offset = plan.start;
        while offset < plan.end {
            let remaining = plan.end - offset;
            let len = usize::try_from(remaining.min(plan.max_elements as u64)).unwrap_or(plan.max_elements);
            let chunk = self.read_range(offset, len)?;
            visit(offset, &chunk);
            offset = offset.checked_add(len as u64).ok_or(ArrayError::RangeOverflow)?;
        }
        Ok(())
    }

    /// 尝试占用 scratch（不计入数组身份存活）。
    pub fn acquire_scratch(&self, bytes: usize) -> Result<(), ArrayError> {
        self.ledger.borrow_mut().acquire_scratch(self.budget, bytes)
    }

    /// 归还 scratch。
    pub fn release_scratch(&self, bytes: usize) {
        self.ledger.borrow_mut().release_scratch(bytes);
    }

    /// 尝试占用 spill 额度。
    pub fn acquire_spill(&self, bytes: usize) -> Result<(), ArrayError> {
        self.ledger.borrow_mut().acquire_spill(self.budget, bytes)
    }

    /// 归还 spill 额度。
    pub fn release_spill(&self, bytes: usize) {
        self.ledger.borrow_mut().release_spill(bytes);
    }

    /// 仅当全表字节 ≤ 驻留预算时允许连续视图；否则 [`ArrayError::FullMaterializeForbidden`]。
    pub fn try_full_view<'a, U>(budget: MemoryBudget, shape: &'a LogicalShape, data: &'a [U]) -> Result<ArrayView<'a, U>, ArrayError> {
        let view = ArrayView::new(shape, data)?;
        let bytes = (shape.element_count() as usize).saturating_mul(std::mem::size_of::<U>());
        if bytes > budget.bytes() {
            return Err(ArrayError::FullMaterializeForbidden { elements: shape.element_count(), resident_limit: budget.bytes() });
        }
        Ok(view)
    }

    fn max_elements(&self) -> Result<usize, ArrayError> {
        let size = std::mem::size_of::<T>();
        if size == 0 {
            return Ok(usize::MAX);
        }
        let max = self.budget.bytes() / size;
        if max == 0 { Err(ArrayError::BudgetTooSmall { element_size: size }) } else { Ok(max) }
    }

    fn check(&self, offset: u64, len: usize) -> Result<(), ArrayError> {
        let max = self.max_elements()?;
        if len > max {
            return Err(ArrayError::BudgetExceeded { requested: len, max });
        }
        let len64 = u64::try_from(len).map_err(|_| ArrayError::RangeOverflow)?;
        let end = offset.checked_add(len64).ok_or(ArrayError::RangeOverflow)?;
        if end > self.shape.element_count() { Err(ArrayError::OutOfBounds) } else { Ok(()) }
    }
}

/// 从内存向量创建一维逻辑数组。
pub fn array1d<T: Clone>(data: Vec<T>, budget: MemoryBudget) -> Result<ChunkedArray<T, InMemoryStorage<T>>, ArrayError> {
    let len = data.len() as u64;
    let shape = LogicalShape::new([len])?;
    let bytes = data.len().saturating_mul(std::mem::size_of::<T>());
    if bytes > budget.bytes() {
        return Err(ArrayError::FullMaterializeForbidden { elements: len, resident_limit: budget.bytes() });
    }
    let store = InMemoryStorage::from_vec(data);
    ChunkedArray::new(shape, store, budget)
}

/// 从行主序扁平向量创建二维逻辑数组 `shape = [nrows, ncols]`（POD / `T: Clone` 便利路径）。
pub fn array2d<T: Clone>(
    nrows: u64,
    ncols: u64,
    data: Vec<T>,
    budget: MemoryBudget,
) -> Result<Array2d<T, InMemoryStorage<T>>, ArrayError> {
    let shape = LogicalShape::new([nrows, ncols])?;
    if data.len() as u64 != shape.element_count() {
        return Err(ArrayError::LengthMismatch { expected: shape.element_count(), actual: data.len() as u64 });
    }
    let bytes = data.len().saturating_mul(std::mem::size_of::<T>());
    if bytes > budget.bytes() {
        return Err(ArrayError::FullMaterializeForbidden { elements: shape.element_count(), resident_limit: budget.bytes() });
    }
    let store = InMemoryStorage::from_vec(data);
    ChunkedArray::new(shape, store, budget)
}

/// 用任意 [`ArrayStorage`] 绑定二维 shape（GC 元素经实现方 `try_clone_in`，不经 Rust [`Clone`]）。
pub fn array2d_from_storage<T, S: ArrayStorage<T>>(
    nrows: u64,
    ncols: u64,
    store: S,
    budget: MemoryBudget,
) -> Result<Array2d<T, S>, ArrayError> {
    let shape = LogicalShape::new([nrows, ncols])?;
    ChunkedArray::new(shape, store, budget)
}

impl<T> ChunkedArray<T, InMemoryStorage<T>> {
    /// 在驻留预算内借用全表切片（零拷贝）。
    pub fn try_as_slice(&self) -> Result<&[T], ArrayError> {
        let bytes = (self.shape.element_count() as usize).saturating_mul(std::mem::size_of::<T>());
        if bytes > self.budget.bytes() {
            return Err(ArrayError::FullMaterializeForbidden {
                elements: self.shape.element_count(),
                resident_limit: self.budget.bytes(),
            });
        }
        Ok(self.store.as_slice())
    }

    /// 在驻留预算内借用全表可变切片。
    pub fn try_as_slice_mut(&mut self) -> Result<&mut [T], ArrayError> {
        let bytes = (self.shape.element_count() as usize).saturating_mul(std::mem::size_of::<T>());
        if bytes > self.budget.bytes() {
            return Err(ArrayError::FullMaterializeForbidden {
                elements: self.shape.element_count(),
                resident_limit: self.budget.bytes(),
            });
        }
        Ok(self.store.as_slice_mut())
    }

    /// 行主序二维索引 → 扁平偏移（仅 `rank == 2`）。
    pub fn row_major_offset(&self, row: u64, col: u64) -> Result<u64, ArrayError> {
        let dims = self.shape.dimensions();
        if dims.len() != 2 {
            return Err(ArrayError::LayoutMismatch);
        }
        if row >= dims[0] || col >= dims[1] {
            return Err(ArrayError::OutOfBounds);
        }
        row.checked_mul(dims[1]).and_then(|v| v.checked_add(col)).ok_or(ArrayError::RangeOverflow)
    }
}
