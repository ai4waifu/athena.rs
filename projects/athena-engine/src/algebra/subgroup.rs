//! 子群、同态与商群（Living `18` Phase 7）。

use std::collections::{HashMap, HashSet, VecDeque};

use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::{bsgs::BsgsChain, permutation::RawPerm};

/// 右陪集代表 `{Hg : g ∈ G}`，每陪集取一个代表元。
pub fn coset_representatives(parent: &BsgsChain, subgroup: &BsgsChain) -> Vec<RawPerm> {
    let mut reps = Vec::new();
    for g in parent.all_elements() {
        let in_known = reps.iter().any(|r: &RawPerm| {
            let diff = r.inverse().compose(&g).expect("same degree");
            subgroup.contains(&diff)
        });
        if !in_known {
            reps.push(g);
        }
    }
    reps
}

/// 陪集索引：`Hg` 中 `g = reps[i]` 时返回 `i`。
pub fn coset_index(reps: &[RawPerm], subgroup: &BsgsChain, element: &RawPerm) -> Option<usize> {
    reps.iter().position(|r| {
        let diff = r.inverse().compose(element).expect("same degree");
        subgroup.contains(&diff)
    })
}

/// 正规子群判定：对父群生成元 `g` 与子群生成元 `h` 检验 `g h g⁻¹ ∈ H`。
pub fn is_normal(_parent: &BsgsChain, parent_generators: &[RawPerm], subgroup: &BsgsChain) -> bool {
    for g in parent_generators {
        for h in subgroup.all_elements() {
            let conj = g.compose(&h).expect("same degree").compose(&g.inverse()).expect("same degree");
            if !subgroup.contains(&conj) {
                return false;
            }
        }
    }
    true
}

/// 商群 `G/N` 的生成元（右陪集乘法 `Hg · g' = H(g g')`）。
pub fn quotient_generators(
    parent: &BsgsChain,
    parent_generators: &[RawPerm],
    subgroup: &BsgsChain,
) -> Result<(Vec<RawPerm>, u32)> {
    let reps = coset_representatives(parent, subgroup);
    let index = reps.len() as u32;
    if index == 0 {
        return Err(subgroup_error("empty_quotient"));
    }
    let mut gens = Vec::new();
    for generator in parent_generators {
        let mut images = vec![0u32; index as usize];
        for (i, r) in reps.iter().enumerate() {
            let product = r.compose(generator)?;
            let j = coset_index(&reps, subgroup, &product).ok_or_else(|| subgroup_error("coset_not_found"))?;
            images[i] = j as u32;
        }
        gens.push(RawPerm::new(images, index)?);
    }
    Ok((gens, index))
}

/// 验证生成元像定义同态，并构造元素像缓存。
pub fn verify_homomorphism_and_cache(
    source: &BsgsChain,
    source_generators: &[RawPerm],
    target: &BsgsChain,
    generator_images: &[RawPerm],
) -> Result<HashMap<Vec<u32>, RawPerm>> {
    if source_generators.len() != generator_images.len() {
        return Err(subgroup_error("generator_image_count"));
    }
    for img in generator_images {
        if !target.contains(img) {
            return Err(subgroup_error("image_not_in_target"));
        }
    }
    let mut cache: HashMap<Vec<u32>, RawPerm> = HashMap::new();
    let id_src = RawPerm::identity(source.degree);
    let id_tgt = RawPerm::identity(target.degree);
    cache.insert(id_src.images().to_vec(), id_tgt.clone());

    let mut queue = VecDeque::from([(id_src.clone(), id_tgt)]);
    let mut seen = HashSet::from([id_src.images().to_vec()]);

    let steps: Vec<(RawPerm, RawPerm)> = source_generators
        .iter()
        .zip(generator_images)
        .flat_map(|(g, im)| {
            let gi = g.inverse();
            let imi = im.inverse();
            [(g.clone(), im.clone()), (gi, imi)]
        })
        .collect();

    while let Some((s, t)) = queue.pop_front() {
        for (step_gen, img) in &steps {
            let s2 = s.compose(step_gen)?;
            let key = s2.images().to_vec();
            let t2 = t.compose(img)?;
            if let Some(t_expected) = cache.get(&key) {
                if t2 != *t_expected {
                    return Err(subgroup_error("homomorphism_relation_failed"));
                }
                continue;
            }
            if !target.contains(&t2) {
                return Err(subgroup_error("image_not_in_target"));
            }
            cache.insert(key.clone(), t2.clone());
            seen.insert(key);
            queue.push_back((s2, t2));
        }
    }

    if cache.len() != source.all_elements().len() {
        return Err(subgroup_error("homomorphism_incomplete"));
    }
    Ok(cache)
}

fn subgroup_error(operation: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::GroupElementInvalid).detail("domain", "group").detail("operation", operation)
}
