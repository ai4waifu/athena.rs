//! 求解器注册表。

use std::{collections::BTreeMap, sync::Arc};

use athena_types::{Diagnostic, DiagnosticCode};

use super::{reflector::Reflector, types::SolverId};

/// 求解器注册表。
#[derive(Default)]
pub struct SolverRegistry {
    reflectors: BTreeMap<u32, Arc<dyn Reflector>>,
}

impl SolverRegistry {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册 reflector。
    pub fn register(&mut self, id: SolverId, reflector: Arc<dyn Reflector>) {
        self.reflectors.insert(id.0, reflector);
    }

    /// 查找。
    pub fn get(&self, id: SolverId) -> Result<&dyn Reflector, Diagnostic> {
        self.reflectors.get(&id.0).map(|r| r.as_ref()).ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "solver")
                .detail("operation", "lookup")
                .arg("solver_id", id.0)
        })
    }
}
