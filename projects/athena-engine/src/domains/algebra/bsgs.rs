//! BSGS 链：base、Schreier 余陪集代表与强生成集缓存。

use std::collections::{HashMap, HashSet, VecDeque};

use athena_numeric::Integer;

use super::permutation::RawPerm;

/// Schreier–Sims 缓存（presentation 附属数据，非 `Group` 本体）。
#[derive(Debug)]
pub struct BsgsChain {
    /// 作用度数。
    pub degree: u32,
    /// 基 `β₁, …, βₖ`。
    pub base: Vec<u32>,
    /// 第 k 层 Schreier 余陪集代表：`t(β_k) = v`。
    pub transversals: Vec<HashMap<u32, RawPerm>>,
    /// 强生成集（按层；层 k 固定 `base[0..k]`）。
    pub strong_generators: Vec<Vec<RawPerm>>,
    /// 群阶 `∏ |transversals[k]|`。
    pub order: Integer,
}

impl BsgsChain {
    /// 由生成元构造 BSGS 链（Schreier–Sims）。
    pub fn from_generators(generators: &[RawPerm], degree: u32) -> Self {
        let mut base = Vec::new();
        let mut transversals = Vec::new();
        let mut strong_generators = Vec::new();
        let mut gen_set = generators_with_inverses(generators);

        loop {
            let beta = match next_base_point(&base, &gen_set, degree) {
                Some(b) => b,
                None => break,
            };
            base.push(beta);
            let (trans, schreier) = schreier_tree(beta, &gen_set, degree);
            transversals.push(trans);
            strong_generators.push(gen_set.iter().map(RawPerm::owning_copy).collect());
            gen_set.extend(schreier);
            gen_set = dedupe_perms(gen_set);
            gen_set = filter_stabilizer(&gen_set, &base, degree);
            if base.len() >= degree as usize {
                break;
            }
        }

        let order = transversals.iter().fold(Integer::one(), |acc, t| acc.mul(&Integer::from_i64(t.len() as i64)));
        Self { degree, base, transversals, strong_generators, order }
    }

    /// 枚举群元素（Schreier 余陪集代表乘积；小群用）。
    pub fn all_elements(&self) -> Vec<RawPerm> {
        let mut elems = vec![RawPerm::identity(self.degree)];
        for trans in &self.transversals {
            let mut next = Vec::new();
            for e in &elems {
                for (_, t) in trans {
                    next.push(e.compose(t).expect("same degree"));
                }
            }
            elems = next;
        }
        elems
    }

    /// 成员判定（sift）。
    pub fn contains(&self, element: &RawPerm) -> bool {
        if element.degree() != self.degree {
            return false;
        }
        let mut h = element.owning_copy();
        for (k, beta) in self.base.iter().enumerate() {
            let img = h.apply(*beta);
            let Some(rep) = self.transversals[k].get(&img)
            else {
                return false;
            };
            h = rep.inverse().compose(&h).unwrap_or_else(|_| RawPerm::identity(self.degree));
        }
        h.is_identity()
    }
}

fn generators_with_inverses(generators: &[RawPerm]) -> Vec<RawPerm> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for g in generators {
        for p in [g.owning_copy(), g.inverse()] {
            if seen.insert(p.images().to_vec()) {
                out.push(p);
            }
        }
    }
    out
}

fn dedupe_perms(perms: Vec<RawPerm>) -> Vec<RawPerm> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for p in perms {
        if seen.insert(p.images().to_vec()) {
            out.push(p);
        }
    }
    out
}

fn next_base_point(base: &[u32], gens: &[RawPerm], degree: u32) -> Option<u32> {
    for p in 0..degree {
        if base.contains(&p) {
            continue;
        }
        if orbit(p, gens, degree).len() > 1 {
            return Some(p);
        }
    }
    None
}

fn orbit(point: u32, gens: &[RawPerm], _degree: u32) -> HashSet<u32> {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([point]);
    while let Some(v) = queue.pop_front() {
        if !seen.insert(v) {
            continue;
        }
        for g in gens {
            let w = g.apply(v);
            if !seen.contains(&w) {
                queue.push_back(w);
            }
        }
    }
    seen
}

fn filter_stabilizer(gens: &[RawPerm], base: &[u32], _degree: u32) -> Vec<RawPerm> {
    let filtered: Vec<RawPerm> = gens.iter().filter(|g| base.iter().all(|&b| g.apply(b) == b)).map(RawPerm::owning_copy).collect();
    dedupe_perms(filtered)
}

fn schreier_tree(beta: u32, gens: &[RawPerm], degree: u32) -> (HashMap<u32, RawPerm>, Vec<RawPerm>) {
    let mut trans: HashMap<u32, RawPerm> = HashMap::new();
    trans.insert(beta, RawPerm::identity(degree));
    let mut queue = VecDeque::from([beta]);
    let mut schreier = Vec::new();

    while let Some(v) = queue.pop_front() {
        let tv = trans.get(&v).map(RawPerm::owning_copy).unwrap_or_else(|| RawPerm::identity(degree));
        for g in gens {
            let w = g.apply(v);
            if !trans.contains_key(&w) {
                let tw = g.compose(&tv).expect("same degree");
                trans.insert(w, tw);
                queue.push_back(w);
            }
            let tw = trans.get(&w).map(RawPerm::owning_copy).unwrap_or_else(|| RawPerm::identity(degree));
            let s = tw.inverse().compose(g).expect("same degree").compose(&tv).expect("same degree");
            if !s.is_identity() {
                schreier.push(s);
            }
        }
    }
    (trans, dedupe_perms(schreier))
}
