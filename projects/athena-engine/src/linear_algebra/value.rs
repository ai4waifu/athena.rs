//! `MatrixValue` — 带 parent/shape/layout 的矩阵值；精确与机器分缓冲。

use std::sync::Arc;

use athena_numeric::{Integer, Rational};
use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    parent::{ElementParentKind, MatrixParent},
    shape::{Layout, MatrixShape, StorageOrder},
};
use crate::numeric_clone::{clone_integer, clone_integers, clone_rational, clone_rationals, resize_integers, resize_rationals};

/// 元素缓冲（精确与机器不得混用同一不透明 `f64` 语义）。
#[derive(Debug, PartialEq)]
pub enum MatrixBuffer {
    /// `ℤ` 稠密缓冲（`Arc` 支持别名 / copy-on-write）。
    Integers(Arc<Vec<Integer>>),
    /// `ℚ` 稠密缓冲。
    Rationals(Arc<Vec<Rational>>),
    /// 机器实数稠密缓冲。
    MachineF64(Arc<Vec<f64>>),
}

impl MatrixBuffer {
    /// 元素个数。
    pub fn len(&self) -> usize {
        match self {
            Self::Integers(v) => v.len(),
            Self::Rationals(v) => v.len(),
            Self::MachineF64(v) => v.len(),
        }
    }

    /// 是否为空缓冲。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Owning 复制（`Arc` 别名共享，与原 `Clone` 语义一致）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Integers(v) => Self::Integers(Arc::clone(v)),
            Self::Rationals(v) => Self::Rationals(Arc::clone(v)),
            Self::MachineF64(v) => Self::MachineF64(Arc::clone(v)),
        }
    }
}

/// 矩阵值。
#[derive(Debug, PartialEq)]
pub struct MatrixValue {
    parent: MatrixParent,
    shape: MatrixShape,
    layout: Layout,
    /// 缓冲起点偏移（视图用）。
    offset: isize,
    data: MatrixBuffer,
}

impl MatrixValue {
    /// 矩阵 parent。
    pub const fn parent(&self) -> MatrixParent {
        self.parent
    }

    /// 矩阵 shape。
    pub const fn shape(&self) -> MatrixShape {
        self.shape
    }

    /// 存储 layout。
    pub const fn layout(&self) -> Layout {
        self.layout
    }

    /// 缓冲视图起点。
    pub const fn offset(&self) -> isize {
        self.offset
    }

    /// 缓冲。
    pub fn buffer(&self) -> &MatrixBuffer {
        &self.data
    }

    /// 自有稠密缓冲的强引用计数（别名检测）。
    pub fn buffer_strong_count(&self) -> usize {
        match &self.data {
            MatrixBuffer::Integers(v) => Arc::strong_count(v),
            MatrixBuffer::Rationals(v) => Arc::strong_count(v),
            MatrixBuffer::MachineF64(v) => Arc::strong_count(v),
        }
    }

    fn validate_len(shape: MatrixShape, data: &MatrixBuffer) -> Result<(), Diagnostic> {
        let n = shape.element_count()?;
        if data.len() != n {
            return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch)
                .detail("reason", "buffer_len_mismatch")
                .detail("expected", n.to_string())
                .detail("got", data.len().to_string()));
        }
        Ok(())
    }

    fn parent_matches_buffer(parent: MatrixParent, data: &MatrixBuffer) -> Result<(), Diagnostic> {
        let ok = match (parent.element, data) {
            (ElementParentKind::Integers, MatrixBuffer::Integers(_)) => parent.rounding.is_exact_like(),
            (ElementParentKind::Rationals, MatrixBuffer::Rationals(_)) => parent.rounding.is_exact_like(),
            (ElementParentKind::MachineReal, MatrixBuffer::MachineF64(_)) => !parent.rounding.is_exact_like(),
            _ => false,
        };
        if ok {
            Ok(())
        }
        else {
            Err(Diagnostic::new(DiagnosticCode::TypeMismatch)
                .detail("reason", "parent_buffer_mismatch")
                .detail("element", format!("{:?}", parent.element)))
        }
    }

    /// 从稠密行主序整数数据构造。
    pub fn from_integers_row_major(rows: u64, cols: u64, data: Vec<Integer>) -> Result<Self, Diagnostic> {
        let shape = MatrixShape::new(rows, cols);
        let layout = Layout::row_major(shape)?;
        let buffer = MatrixBuffer::Integers(Arc::new(data));
        Self::validate_len(shape, &buffer)?;
        let parent = MatrixParent::integers();
        Self::parent_matches_buffer(parent, &buffer)?;
        Ok(Self { parent, shape, layout, offset: 0, data: buffer })
    }

    /// 从稠密行主序有理数据构造。
    pub fn from_rationals_row_major(rows: u64, cols: u64, data: Vec<Rational>) -> Result<Self, Diagnostic> {
        let shape = MatrixShape::new(rows, cols);
        let layout = Layout::row_major(shape)?;
        let buffer = MatrixBuffer::Rationals(Arc::new(data));
        Self::validate_len(shape, &buffer)?;
        let parent = MatrixParent::rationals();
        Self::parent_matches_buffer(parent, &buffer)?;
        Ok(Self { parent, shape, layout, offset: 0, data: buffer })
    }

    /// 从稠密行主序机器实数构造。
    pub fn from_f64_row_major(rows: u64, cols: u64, data: Vec<f64>) -> Result<Self, Diagnostic> {
        let shape = MatrixShape::new(rows, cols);
        let layout = Layout::row_major(shape)?;
        let buffer = MatrixBuffer::MachineF64(Arc::new(data));
        Self::validate_len(shape, &buffer)?;
        let parent = MatrixParent::machine_real();
        Self::parent_matches_buffer(parent, &buffer)?;
        Ok(Self { parent, shape, layout, offset: 0, data: buffer })
    }

    /// 列主序机器实数。
    pub fn from_f64_column_major(rows: u64, cols: u64, data: Vec<f64>) -> Result<Self, Diagnostic> {
        let shape = MatrixShape::new(rows, cols);
        let layout = Layout::column_major(shape)?;
        let buffer = MatrixBuffer::MachineF64(Arc::new(data));
        Self::validate_len(shape, &buffer)?;
        let parent = MatrixParent::machine_real();
        Self::parent_matches_buffer(parent, &buffer)?;
        Ok(Self { parent, shape, layout, offset: 0, data: buffer })
    }

    /// 零矩阵（按 parent 元素类型）。
    pub fn zeros(parent: MatrixParent, shape: MatrixShape, order: StorageOrder) -> Result<Self, Diagnostic> {
        let n = shape.element_count()?;
        let layout = match order {
            StorageOrder::RowMajor => Layout::row_major(shape)?,
            StorageOrder::ColumnMajor => Layout::column_major(shape)?,
        };
        let data = match parent.element {
            ElementParentKind::Integers => MatrixBuffer::Integers(Arc::new({
                let mut __v = Vec::new();
                resize_integers(&mut __v, n, &Integer::zero());
                __v
            })),
            ElementParentKind::Rationals => MatrixBuffer::Rationals(Arc::new({
                let mut __v = Vec::new();
                resize_rationals(&mut __v, n, &Rational::zero());
                __v
            })),
            ElementParentKind::MachineReal => MatrixBuffer::MachineF64(Arc::new(vec![0.0; n])),
        };
        Self::parent_matches_buffer(parent, &data)?;
        Ok(Self { parent, shape, layout, offset: 0, data })
    }

    /// 单位矩阵（方阵）。
    pub fn identity(parent: MatrixParent, n: u64) -> Result<Self, Diagnostic> {
        let mut m = Self::zeros(parent, MatrixShape::new(n, n), StorageOrder::RowMajor)?;
        for i in 0..n {
            m.set_owned(i, i, MatrixEntry::one(parent.element)?)?;
        }
        Ok(m)
    }

    /// 线性存储下标。
    fn linear_index(&self, row: u64, col: u64) -> Result<usize, Diagnostic> {
        super::index::IndexSpec::check_scalar(self.shape, row, col)?;
        let off = self.layout.offset(row, col)?;
        let idx = self
            .offset
            .checked_add(off)
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::InvalidIndex).detail("reason", "view_offset"))?;
        if idx < 0 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidIndex).detail("reason", "negative_index"));
        }
        Ok(idx as usize)
    }

    /// Owning 复制（共享 `Arc` 缓冲）。
    pub fn owning_copy(&self) -> Self {
        Self { parent: self.parent, shape: self.shape, layout: self.layout, offset: self.offset, data: self.data.owning_copy() }
    }

    /// 读取元素（拷贝）。
    pub fn get(&self, row: u64, col: u64) -> Result<MatrixEntry, Diagnostic> {
        let i = self.linear_index(row, col)?;
        Ok(match &self.data {
            MatrixBuffer::Integers(v) => MatrixEntry::Integer(clone_integer(&v[i])),
            MatrixBuffer::Rationals(v) => MatrixEntry::Rational(clone_rational(&v[i])),
            MatrixBuffer::MachineF64(v) => MatrixEntry::MachineF64(v[i]),
        })
    }

    fn ensure_unique_buffer(&mut self) {
        match &mut self.data {
            MatrixBuffer::Integers(v) => {
                if Arc::strong_count(v) > 1 {
                    *v = Arc::new(clone_integers(v.as_ref()));
                }
            }
            MatrixBuffer::Rationals(v) => {
                if Arc::strong_count(v) > 1 {
                    *v = Arc::new(clone_rationals(v.as_ref()));
                }
            }
            MatrixBuffer::MachineF64(v) => {
                Arc::make_mut(v);
            }
        }
    }

    /// 写时复制后写入。
    pub fn set_owned(&mut self, row: u64, col: u64, value: MatrixEntry) -> Result<(), Diagnostic> {
        let i = self.linear_index(row, col)?;
        self.ensure_unique_buffer();
        match (&mut self.data, value) {
            (MatrixBuffer::Integers(v), MatrixEntry::Integer(x)) => {
                Arc::get_mut(v).expect("unique after ensure")[i] = x;
                Ok(())
            }
            (MatrixBuffer::Rationals(v), MatrixEntry::Rational(x)) => {
                Arc::get_mut(v).expect("unique after ensure")[i] = x;
                Ok(())
            }
            (MatrixBuffer::MachineF64(v), MatrixEntry::MachineF64(x)) => {
                Arc::get_mut(v).expect("unique after ensure")[i] = x;
                Ok(())
            }
            _ => Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "entry_parent_mismatch")),
        }
    }

    /// 转置视图（共享缓冲，交换 stride）。
    pub fn transpose_view(&self) -> Self {
        Self {
            parent: self.parent,
            shape: self.shape.transpose(),
            layout: self.layout.transposed(),
            offset: self.offset,
            data: self.data.owning_copy(),
        }
    }

    /// 物化为自有行主序缓冲。
    pub fn materialize_row_major(&self) -> Result<Self, Diagnostic> {
        let n = self.shape.element_count()?;
        match self.parent.element {
            ElementParentKind::Integers => {
                let mut out = Vec::with_capacity(n);
                for r in 0..self.shape.rows {
                    for c in 0..self.shape.cols {
                        out.push(match self.get(r, c)? {
                            MatrixEntry::Integer(x) => x,
                            _ => unreachable!(),
                        });
                    }
                }
                Self::from_integers_row_major(self.shape.rows, self.shape.cols, out)
            }
            ElementParentKind::Rationals => {
                let mut out = Vec::with_capacity(n);
                for r in 0..self.shape.rows {
                    for c in 0..self.shape.cols {
                        out.push(match self.get(r, c)? {
                            MatrixEntry::Rational(x) => x,
                            _ => unreachable!(),
                        });
                    }
                }
                Self::from_rationals_row_major(self.shape.rows, self.shape.cols, out)
            }
            ElementParentKind::MachineReal => {
                let mut out = Vec::with_capacity(n);
                for r in 0..self.shape.rows {
                    for c in 0..self.shape.cols {
                        out.push(match self.get(r, c)? {
                            MatrixEntry::MachineF64(x) => x,
                            _ => unreachable!(),
                        });
                    }
                }
                Self::from_f64_row_major(self.shape.rows, self.shape.cols, out)
            }
        }
    }

    /// 提升 `ℤ` → `ℚ`（精确路径）。
    pub fn promote_integers_to_rationals(&self) -> Result<Self, Diagnostic> {
        match &self.data {
            MatrixBuffer::Integers(_) => {
                let m = self.materialize_row_major()?;
                let MatrixBuffer::Integers(v) = m.data
                else {
                    unreachable!();
                };
                let rats: Vec<_> = v.iter().map(|x| Rational::from_integer(clone_integer(x))).collect();
                Self::from_rationals_row_major(m.shape.rows, m.shape.cols, rats)
            }
            MatrixBuffer::Rationals(_) => Ok(self.owning_copy()),
            MatrixBuffer::MachineF64(_) => {
                Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "cannot_promote_machine_to_exact"))
            }
        }
    }

    /// 按行主序收集有理元素（精确路径）。
    pub fn to_rationals_row_major(&self) -> Result<Vec<Rational>, Diagnostic> {
        let m = self.promote_integers_to_rationals()?.materialize_row_major()?;
        match m.data {
            MatrixBuffer::Rationals(v) => Ok(clone_rationals(v.as_ref())),
            _ => Err(Diagnostic::new(DiagnosticCode::TypeMismatch)),
        }
    }

    /// 按行主序收集 `f64`（机器路径）。
    pub fn to_f64_row_major(&self) -> Result<Vec<f64>, Diagnostic> {
        let m = self.materialize_row_major()?;
        match m.data {
            MatrixBuffer::MachineF64(v) => Ok((*v).clone()),
            _ => Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "not_machine")),
        }
    }
}

trait RoundingExact {
    fn is_exact_like(self) -> bool;
}

impl RoundingExact for super::parent::RoundingPolicy {
    fn is_exact_like(self) -> bool {
        matches!(self, super::parent::RoundingPolicy::Exact)
    }
}

/// 矩阵元素拷贝。
#[derive(Debug, PartialEq)]
pub enum MatrixEntry {
    /// 整数。
    Integer(Integer),
    /// 有理。
    Rational(Rational),
    /// 机器实数。
    MachineF64(f64),
}

impl MatrixEntry {
    /// 乘法单位元。
    pub fn one(kind: ElementParentKind) -> Result<Self, Diagnostic> {
        Ok(match kind {
            ElementParentKind::Integers => Self::Integer(Integer::one()),
            ElementParentKind::Rationals => Self::Rational(Rational::one()),
            ElementParentKind::MachineReal => Self::MachineF64(1.0),
        })
    }

    /// 加法单位元。
    pub fn zero(kind: ElementParentKind) -> Result<Self, Diagnostic> {
        Ok(match kind {
            ElementParentKind::Integers => Self::Integer(Integer::zero()),
            ElementParentKind::Rationals => Self::Rational(Rational::zero()),
            ElementParentKind::MachineReal => Self::MachineF64(0.0),
        })
    }

    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Integer(x) => Self::Integer(clone_integer(x)),
            Self::Rational(x) => Self::Rational(clone_rational(x)),
            Self::MachineF64(x) => Self::MachineF64(*x),
        }
    }
}

impl Clone for MatrixValue {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}

impl Clone for MatrixEntry {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}
