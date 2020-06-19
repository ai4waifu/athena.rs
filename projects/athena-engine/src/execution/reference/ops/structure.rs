//! Reference 槽 → Term 辅助。

use athena_types::{Result, TermId};

use super::super::{ReferenceExecutor, Slot, helpers::*};
use crate::runtime::session::Session;

impl ReferenceExecutor {
    pub(crate) fn slot_as_term(&self, session: &mut Session, slot: Slot) -> Result<TermId> {
        match slot {
            Slot::Term(term) => Ok(term),
            Slot::Boolean(value) => Ok(session.builder().boolean(value, Default::default())),
            Slot::Symbol(symbol) => Ok(session.builder().symbol_id(symbol, Default::default())),
            Slot::Unit => Ok(session.builder().null(Default::default())),
            Slot::Scope(_) | Slot::Result(_) | Slot::Value(_) | Slot::Empty => Err(diag("slot_not_term")),
        }
    }
}
