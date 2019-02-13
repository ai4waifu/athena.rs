//! `infra` 分组：ndarray / graph / table 合同 smoke fixture。

use athena_graph::{CsrGraph, Graph, GraphDirection, bfs_order};
use athena_ndarray::{LogicalShape, MemoryBudget, array1d};
use athena_table::{Field, LogicalType, Schema, Table, TableExpr, column_from_store};

use crate::{
    fixture::{BenchGroup, Fixture, FixtureMeta, Suite},
    validate::{DeterminacyKind, ExactnessKind, ValidationSummary},
};

struct NdarrayBudgetFixture;

impl Fixture for NdarrayBudgetFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("infra.ndarray_chunk_budget", BenchGroup::Infra, "len_10_budget_24", "ndarray")
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let a = array1d((0u64..10).collect(), MemoryBudget::new(24).unwrap()).map_err(|e| format!("{e:?}"))?;
        let mut sizes = Vec::new();
        a.for_each_chunk(|_, c| sizes.push(c.len())).map_err(|e| format!("{e:?}"))?;
        if sizes != [3, 3, 3, 1] {
            return Err(format!("unexpected chunk sizes: {sizes:?}"));
        }
        assert!(LogicalShape::new([u64::MAX, 2]).is_err());
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "ndarray budget chunking"))
    }

    fn run_once(&self) {
        let _ = self.validate();
    }
}

struct GraphCsrFixture;

impl Fixture for GraphCsrFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("infra.graph_csr_stream", BenchGroup::Infra, "3_nodes_5_edges", "graph")
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let offsets = array1d(vec![0, 5, 5, 5], MemoryBudget::new(32).unwrap()).map_err(|e| format!("{e:?}"))?;
        let indices = array1d(vec![1, 2, 1, 2, 1], MemoryBudget::new(32).unwrap()).map_err(|e| format!("{e:?}"))?;
        let graph = CsrGraph::new(3, offsets, indices).map_err(|e| format!("{e:?}"))?;
        let mut chunks = Vec::new();
        graph.for_each_neighbor_chunk(0, |c| chunks.push(c.to_vec())).map_err(|e| format!("{e:?}"))?;
        if chunks.len() != 3 {
            return Err(format!("expected 3 neighbor chunks, got {}", chunks.len()));
        }
        let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
        let a = g.add_node(());
        let b = g.add_node(());
        g.add_edge(a, b, ());
        let order = bfs_order(&g, a).map_err(|e| format!("{e:?}"))?;
        if order.len() != 2 {
            return Err("bfs order length".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "graph csr + bfs"))
    }

    fn run_once(&self) {
        let _ = self.validate();
    }
}

struct TableLazyFixture;

impl Fixture for TableLazyFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("infra.table_lazy_meta", BenchGroup::Infra, "schema_2_cols", "table")
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let schema = Schema::new([Field::new("id", LogicalType::Int(64), false), Field::new("name", LogicalType::Utf8, true)])
            .map_err(|e| format!("{e:?}"))?;
        let table = Table::with_rows(schema.clone(), 50)
            .lazy()
            .select([TableExpr::Column("id".into())])
            .limit(5)
            .collect_meta()
            .map_err(|e| format!("{e:?}"))?;
        if table.row_count() != 5 {
            return Err("limit row count".into());
        }
        let field = Field::new("x", LogicalType::Int(64), false);
        let col = column_from_store(
            field,
            athena_ndarray::InMemoryStorage::from_vec((0i64..4).collect()),
            MemoryBudget::new(16).unwrap(),
        )
        .map_err(|e| format!("{e:?}"))?;
        if col.len() != 4 {
            return Err("column length".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "table lazy + column"))
    }

    fn run_once(&self) {
        let _ = self.validate();
    }
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(NdarrayBudgetFixture));
    suite.register(Box::new(GraphCsrFixture));
    suite.register(Box::new(TableLazyFixture));
}
