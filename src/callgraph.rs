use regex::Regex;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Default)]
/// caller -> callee
pub struct CallGraph<'a>(pub HashMap<&'a str, HashSet<&'a str>>);

/// callee -> caller
#[derive(Debug, Clone, Default)]
pub struct InvCallGraph<'a>(pub HashMap<&'a str, HashSet<&'a str>>);

impl<'a> CallGraph<'a> {
    pub fn invert(&self) -> InvCallGraph<'a> {
        let mut inv: HashMap<&str, HashSet<&str>> = HashMap::new();
        for (&caller, callees) in &self.0 {
            for &callee in callees {
                inv.entry(callee).or_default().insert(caller);
            }
        }
        InvCallGraph(inv)
    }
}

impl<'a> InvCallGraph<'a> {
    pub fn callers_of(&self, re: &Regex) -> HashMap<&'a str, usize> {
        let mut depths: HashMap<&str, usize> = HashMap::new();
        let mut queue: VecDeque<(&str, usize)> = VecDeque::new();

        for &node in self.0.keys() {
            if re.is_match(node) {
                depths.insert(node, 0);
                queue.push_back((node, 0));
            }
        }

        while let Some((node, depth)) = queue.pop_front() {
            if let Some(callers) = self.0.get(node) {
                for &caller in callers {
                    use std::collections::hash_map::Entry;
                    if let Entry::Vacant(e) = depths.entry(caller) {
                        e.insert(depth + 1);
                        queue.push_back((caller, depth + 1));
                    }
                }
            }
        }

        depths
    }
}
