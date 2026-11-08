use std::collections::{HashMap, HashSet};
use crate::compiler::Sym;

#[derive(Copy, Clone)]
enum VisitState {
    Unseen,
    Observed,
    Seen,
}

pub(crate) fn topological_sort(population: Vec<(Sym, HashSet<Sym>)>) -> Result<Vec<Sym>, (Sym, Sym)> {
    let mut tree = HashMap::with_capacity(population.len());
    tree.extend(population.iter().map(|(sym, deps)| (*sym, (deps, VisitState::Unseen))));

    let mut order = Vec::with_capacity(population.len());
    for (sym, _) in &population {
        if let Err(sym2) = visit_tree_downtop(*sym, &mut tree, &mut order) {
            return Err((*sym, sym2.unwrap_or(*sym)))
        }
    }

    Ok(order)
}

fn visit_tree_downtop(sym: Sym, tree: &mut HashMap<Sym, (&HashSet<Sym>, VisitState)>, buff: &mut Vec<Sym>) -> Result<(), Option<Sym>> {
    let (parasites, state) = *tree.get(&sym).unwrap();
    match state {
        VisitState::Unseen => tree.entry(sym).and_modify(|(_, state)| *state = VisitState::Observed),
        VisitState::Observed => return Err(None),
        VisitState::Seen => return Ok(()),
    };

    for parasite in parasites.iter() {
        if let Err(o) = visit_tree_downtop(*parasite, tree, buff) {
            return Err(o.or(Some(sym)));
        }
    }

    buff.push(sym);
    tree.entry(sym).and_modify(|(_, state)| *state = VisitState::Seen);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sort(pat: Vec<(Sym, Vec<Sym>)>) -> Result<Vec<Sym>, (Sym, Sym)> {
        topological_sort(pat.iter()
            .map(|(sym, deps)| {
                let set: HashSet<Sym> = deps.iter().cloned().collect();
                (*sym, set)
            })
            .collect())
    }

    #[test]
    fn topsort() {
        // a+b-c where a=2c, b=4c+d, c=2, d=3
        // 1+2-3 where 1 -> 3, 2 -> 3 4, 3, 4
        assert_eq!(
            sort(vec![
                (1, vec![3]),
                (2, vec![3, 4]),
                (3, vec![]),
                (4, vec![]),
            ]).unwrap(),
            vec![3, 1, 4, 2],
        );

        // a+b-e where a=2c+b+e, b=3d, c=3b, d=4, e=5
        // 1+2-5 where 1 -> 3 2 5, 2 -> 4, 3 -> 2, 4, 5
        assert_eq!(
            sort(vec![
                (1, vec![3, 2, 5]),
                (2, vec![4]),
                (3, vec![2]),
                (4, vec![]),
                (5, vec![]),
            ]).unwrap(),
            vec![4, 2, 3, 5, 1],
        );

        // a where a=a
        // 1 where 1 -> 1
        assert_eq!(
            sort(vec![
                (1, vec![1]),
            ]).expect_err("undetected recursion"),
            (1, 1),
        );

        // a+b where a=2b, b=3c, c=a
        // 1+2 where 1 -> 2, 2 -> 3, 3 -> 1
        assert_eq!(
            sort(vec![
                (1, vec![2]),
                (2, vec![3]),
                (3, vec![1]),
            ]).expect_err("undetected recursion"),
            (1, 3),
        );
    }
}
