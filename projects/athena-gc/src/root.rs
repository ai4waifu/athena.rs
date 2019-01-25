//! Root registry：Session / TS handle / graph / cache pin 等统一登记。

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
}

/// 已登记 root。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GcRoot {
    /// 令牌。
    pub token: RootToken,
    /// 对象。
    pub object: GcObjectId,
    /// 种类。
    pub kind: RootKind,
}

/// Root 表。
#[derive(Debug, Default)]
pub struct RootRegistry {
    next_token: u64,
    roots: Vec<Option<GcRoot>>,
    free: Vec<usize>,
}

impl RootRegistry {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记 root，返回令牌。
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

    /// 移除 root（未知令牌则 no-op）。
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

    /// 迭代当前 roots。
    pub fn iter(&self) -> impl Iterator<Item = GcRoot> + '_ {
        self.roots.iter().filter_map(|s| *s)
    }

    /// 当前 root 数量。
    pub fn len(&self) -> usize {
        self.roots.iter().filter(|s| s.is_some()).count()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
