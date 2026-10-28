use tickora_core::fx::Fx;

pub const MASTER: &str = "master";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubmixId(pub u32);

impl SubmixId {
    pub const MASTER: SubmixId = SubmixId(0);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmixDef {
    pub name: String,
    pub parent: String,
    pub gain: Fx,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixGraph {
    names: Vec<String>,
    parents: Vec<Option<usize>>,
    gains: Vec<Fx>,
    children_first: Vec<usize>,
}

impl MixGraph {
    pub fn new(master_gain: Fx, submixes: &[SubmixDef]) -> Result<MixGraph, String> {
        let mut names = vec![MASTER.to_string()];
        for def in submixes {
            if names.contains(&def.name) {
                return Err(format!("submix {:?} is defined twice", def.name));
            }
            names.push(def.name.clone());
        }
        let mut parents = vec![None];
        for def in submixes {
            let parent = names
                .iter()
                .position(|name| *name == def.parent)
                .ok_or_else(|| {
                    format!(
                        "submix {:?} feeds unknown submix {:?}",
                        def.name, def.parent
                    )
                })?;
            parents.push(Some(parent));
        }
        let mut depths = Vec::with_capacity(names.len());
        for start in 0..names.len() {
            let mut depth = 0;
            let mut at = start;
            while let Some(parent) = parents[at] {
                depth += 1;
                if depth > names.len() {
                    return Err(format!("submix {:?} feeds itself", names[start]));
                }
                at = parent;
            }
            depths.push(depth);
        }
        let mut children_first: Vec<usize> = (1..names.len()).collect();
        children_first.sort_by_key(|&index| (core::cmp::Reverse(depths[index]), index));
        let mut gains = vec![master_gain];
        gains.extend(submixes.iter().map(|def| def.gain));
        Ok(MixGraph {
            names,
            parents,
            gains,
            children_first,
        })
    }

    pub fn id(&self, name: &str) -> Option<SubmixId> {
        self.names
            .iter()
            .position(|known| known == name)
            .map(|index| SubmixId(index as u32))
    }

    pub fn name(&self, id: SubmixId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn gain(&self, id: SubmixId) -> Option<Fx> {
        self.gains.get(id.0 as usize).copied()
    }

    pub(crate) fn children_first(&self) -> impl Iterator<Item = (usize, usize, Fx)> + '_ {
        self.children_first.iter().map(|&index| {
            (
                index,
                self.parents[index].expect("only master has no parent"),
                self.gains[index],
            )
        })
    }
}

impl Default for MixGraph {
    fn default() -> Self {
        MixGraph::new(Fx::ONE, &[]).expect("a graph with only master is valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(name: &str, parent: &str) -> SubmixDef {
        SubmixDef {
            name: name.to_string(),
            parent: parent.to_string(),
            gain: Fx::ONE,
        }
    }

    #[test]
    fn submixes_resolve_by_name_and_mix_children_before_parents() {
        let graph = MixGraph::new(
            Fx::ONE,
            &[
                def("music", "master"),
                def("footsteps", "sfx"),
                def("sfx", "master"),
            ],
        )
        .unwrap();
        assert_eq!(graph.id("master"), Some(SubmixId::MASTER));
        assert_eq!(graph.id("sfx"), Some(SubmixId(3)));
        let order: Vec<(usize, usize)> = graph.children_first().map(|(c, p, _)| (c, p)).collect();
        assert_eq!(order, [(2, 3), (1, 0), (3, 0)]);
    }

    #[test]
    fn a_cycle_is_rejected() {
        let error = MixGraph::new(Fx::ONE, &[def("a", "b"), def("b", "a")]).unwrap_err();
        assert!(error.contains("feeds itself"), "{error}");
        assert!(MixGraph::new(Fx::ONE, &[def("a", "a")]).is_err());
    }

    #[test]
    fn unknown_parents_and_duplicates_are_rejected() {
        assert!(MixGraph::new(Fx::ONE, &[def("a", "nowhere")]).is_err());
        assert!(MixGraph::new(Fx::ONE, &[def("a", "master"), def("a", "master")]).is_err());
        assert!(MixGraph::new(Fx::ONE, &[def("master", "master")]).is_err());
    }
}
