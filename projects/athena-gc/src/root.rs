//! Root registry：Session / TS handle / graph / cache pin / numeric 等统一登记。

use core::ptr::NonNull;

use crate::ids::{GcObjectId, RootToken};

/// Root 来源（编排层填写，GC 只认登记）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootKind {
    /// 活动 Session。
    Session,
    /// TS opaque handle。
    TsHandle,
    /// M-Graph root。
    MGraph,
    /// E-Graph root。
    EGraph,
    /// IR root。
    Ir,
    /// Solver frontier。
    SolverFrontier,
    /// 进行中计算。
    InFlight,
    /// Cache pin。
    CachePin,
    /// 用户显式 retain。
    UserRetain,
    /// Session 持有的 numeric limb block。
    Numeric,
}

/// 已登记 object root。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GcRoot {
    /// 令牌。
    pub token: RootToken,
    /// 对象。
    pub object: GcObjectId,
    /// 种类。
    pub kind: RootKind,
}

/// 已登记 numeric payload root（GC-owned limb block）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericRoot {
    /// 令牌。
    pub token: RootToken,
    /// Limb / payload 起点。
    pub payload: NonNull<u8>,
    /// 种类（通常 [`RootKind::Numeric`]）。
    pub kind: RootKind,
}

/// Root 表。
#[derive(Debug, Default)]
pub struct RootRegistry {
    next_token: u64,
    roots: Vec<Option<GcRoot>>,
    free: Vec<usize>,
    numeric_roots: Vec<Option<NumericRoot>>,
    numeric_free: Vec<usize>,
}

impl RootRegistry {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记 object root，返回令牌。
    pub fn register(&mut self, object: GcObjectId, kind: RootKind) -> RootToken {
        let token = RootToken(self.next_token);
        self.next_token = self.next_token.wrapping_add(1);
        let root = GcRoot { token, object, kind };
        if let Some(slot) = self.free.pop() {
            self.roots[slot] = Some(root);
        }
        else {
            self.roots.push(Some(root));
        }
        token
    }

    /// 登记 numeric payload root。
    pub fn register_numeric(&mut self, payload: NonNull<u8>, kind: RootKind) -> RootToken {
        let token = RootToken(self.next_token);
        self.next_token = self.next_token.wrapping_add(1);
        let root = NumericRoot { token, payload, kind };
        if let Some(slot) = self.numeric_free.pop() {
            self.numeric_roots[slot] = Some(root);
        }
        else {
            self.numeric_roots.push(Some(root));
        }
        token
    }

    /// 移除 object root（未知令牌则 no-op）。
    pub fn unregister(&mut self, token: RootToken) -> bool {
        for (i, slot) in self.roots.iter_mut().enumerate() {
            if slot.as_ref().is_some_and(|r| r.token == token) {
                *slot = None;
                self.free.push(i);
                return true;
            }
        }
        false
    }

    /// 移除 numeric root。
    pub fn unregister_numeric(&mut self, token: RootToken) -> bool {
        for (i, slot) in self.numeric_roots.iter_mut().enumerate() {
            if slot.as_ref().is_some_and(|r| r.token == token) {
                *slot = None;
                self.numeric_free.push(i);
                return true;
            }
        }
        false
    }

    /// 按 payload 撤掉一条 numeric root（值对象 24B 布局无法内嵌 [`RootToken`] 时的 Drop 路径）。
    ///
    /// 同一 payload 可有多条 root（共享 `Clone`）。每次 Drop 撤一条。
    pub fn unregister_one_numeric_for_payload(&mut self, payload: NonNull<u8>) -> bool {
        for (i, slot) in self.numeric_roots.iter_mut().enumerate() {
            if slot.as_ref().is_some_and(|r| r.payload == payload) {
                *slot = None;
                self.numeric_free.push(i);
                return true;
            }
        }
        false
    }

    /// 指定 payload 上当前 numeric root 条数。
    pub fn numeric_root_count_for_payload(&self, payload: NonNull<u8>) -> usize {
        self.numeric_roots.iter().filter(|s| s.as_ref().is_some_and(|r| r.payload == payload)).count()
    }

    /// 迭代当前 object roots。
    pub fn iter(&self) -> impl Iterator<Item = GcRoot> + '_ {
        self.roots.iter().filter_map(|s| *s)
    }

    /// 迭代 numeric roots。
    pub fn iter_numeric(&self) -> impl Iterator<Item = NumericRoot> + '_ {
        self.numeric_roots.iter().filter_map(|s| *s)
    }

    /// 当前 object root 数量。
    pub fn len(&self) -> usize {
        self.roots.iter().filter(|s| s.is_some()).count()
    }

    /// Numeric root 数量。
    pub fn numeric_len(&self) -> usize {
        self.numeric_roots.iter().filter(|s| s.is_some()).count()
    }

    /// 是否为空（object + numeric）。
    pub fn is_empty(&self) -> bool {
        self.len() == 0 && self.numeric_len() == 0
    }
}
