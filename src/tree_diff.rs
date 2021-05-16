use std::cell::Cell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use ChangeKind::*;
use Syntax::*;

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum ChangeKind {
    Unchanged,
    Added,
    Removed,
    Moved,
}

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum AtomKind {
    String,
    Comment,
    Other,
}

#[derive(Debug, Clone)]
pub enum Syntax {
    List {
        change: Cell<ChangeKind>,
        open_delimiter: String,
        children: Vec<Syntax>,
        close_delimiter: String,
        num_descendants: usize,
    },
    Atom {
        change: Cell<ChangeKind>,
        content: String,
        kind: AtomKind,
    },
}

impl Syntax {
    pub fn new_list(open_delimiter: &str, children: Vec<Syntax>, close_delimiter: &str) -> Syntax {
        let mut num_descendants = 0;
        for child in &children {
            num_descendants += match child {
                List {
                    num_descendants, ..
                } => *num_descendants + 1,
                Atom { .. } => 1,
            };
        }

        List {
            change: Cell::new(Unchanged),
            open_delimiter: open_delimiter.into(),
            close_delimiter: close_delimiter.into(),
            children,
            num_descendants,
        }
    }

    pub fn new_atom(content: &str, kind: AtomKind) -> Syntax {
        Atom {
            content: content.into(),
            change: Cell::new(Unchanged),
            kind,
        }
    }

    fn set_change(&self, ck: ChangeKind) {
        match self {
            List { change, .. } => {
                change.set(ck);
            }
            Atom { change, .. } => {
                change.set(ck);
            }
        }
    }

    fn set_change_deep(&self, ck: ChangeKind) {
        self.set_change(ck);
        if let List { children, .. } = self {
            for child in children {
                child.set_change_deep(ck);
            }
        }
    }
}

impl PartialEq for Syntax {
    fn eq(&self, other: &Self) -> bool {
        match (&self, other) {
            (
                Atom {
                    content: lhs_content,
                    kind: lhs_kind,
                    ..
                },
                Atom {
                    content: rhs_content,
                    kind: rhs_kind,
                    ..
                },
            ) => lhs_content == rhs_content && lhs_kind == rhs_kind,
            (
                List {
                    open_delimiter: lhs_start_content,
                    close_delimiter: lhs_end_content,
                    children: lhs_children,
                    ..
                },
                List {
                    open_delimiter: rhs_start_content,
                    close_delimiter: rhs_end_content,
                    children: rhs_children,
                    ..
                },
            ) => {
                lhs_start_content == rhs_start_content
                    && lhs_end_content == rhs_end_content
                    && lhs_children == rhs_children
            }
            _ => false,
        }
    }
}
impl Eq for Syntax {}

impl Hash for Syntax {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            List {
                open_delimiter: start_content,
                close_delimiter: end_content,
                children,
                ..
            } => {
                start_content.hash(state);
                end_content.hash(state);
                for child in children {
                    child.hash(state);
                }
            }
            Atom { content, .. } => {
                content.hash(state);
            }
        }
    }
}

/// Extremely dumb top-level comparison of `lhs` and `rhs`.
pub fn set_changed(lhs: &[Syntax], rhs: &[Syntax]) {
    let mut lhs_subtrees = HashMap::new();
    for s in lhs.iter() {
        build_subtrees(s, &mut lhs_subtrees);
    }

    let mut rhs_subtrees = HashMap::new();
    for s in rhs.iter() {
        build_subtrees(s, &mut rhs_subtrees);
    }

    walk_nodes_ordered(lhs, rhs, &mut lhs_subtrees, &mut rhs_subtrees);
}

/// Decrement the count of `node` from `counts`, along with all its children.
fn decrement<'a>(node: &'a Syntax, counts: &mut HashMap<&'a Syntax, i64>) {
    let count = if let Some(count) = counts.get(node) {
        *count
    } else {
        panic!("Called decrement on a node that isn't in counts")
    };

    assert!(count > 0);
    counts.insert(node, count - 1);
    match node {
        List { children, .. } => {
            for child in children {
                decrement(child, counts);
            }
        }
        Atom { .. } => {}
    }
}

// Greedy tree differ.
fn walk_nodes_ordered<'a>(
    lhs: &'a [Syntax],
    rhs: &'a [Syntax],
    lhs_counts: &mut HashMap<&'a Syntax, i64>,
    rhs_counts: &mut HashMap<&'a Syntax, i64>,
) {
    let mut lhs_i = 0;
    let mut rhs_i = 0;
    loop {
        match (lhs.get(lhs_i), rhs.get(rhs_i)) {
            (Some(ref lhs_node), Some(ref rhs_node)) => {
                let lhs_count = *lhs_counts.get(lhs_node).unwrap_or(&0);
                let rhs_count = *rhs_counts.get(lhs_node).unwrap_or(&0);

                // If they're equal, nothing to do.
                if lhs_node == rhs_node && lhs_count > 0 && rhs_count > 0 {
                    lhs_node.set_change_deep(Unchanged);
                    rhs_node.set_change_deep(Unchanged);

                    decrement(lhs_node, lhs_counts);
                    decrement(rhs_node, rhs_counts);
                    lhs_i += 1;
                    rhs_i += 1;
                    continue;
                }

                // Not equal. Do we have more instances of the LHS
                // node? If so, we've removed some instances on the
                // RHS, so assume this is a removal.
                if lhs_count > rhs_count && rhs_count > 0 {
                    lhs_node.set_change_deep(Removed);
                    decrement(lhs_node, lhs_counts);
                    lhs_i += 1;
                    continue;
                }

                // Do we have more instances of the RHS
                // node? If so, we've added some instances on the
                // RHS, so assume this is an addition.
                let lhs_count = *lhs_counts.get(rhs_node).unwrap_or(&0);
                let rhs_count = *rhs_counts.get(rhs_node).unwrap_or(&0);
                if rhs_count > lhs_count && lhs_count > 0 {
                    rhs_node.set_change_deep(Added);
                    decrement(rhs_node, rhs_counts);
                    rhs_i += 1;
                    continue;
                }

                // Same number: reordered nodes, or both nodes are
                // novel to a single side.
                let mut lhs_node = lhs_node;
                let mut rhs_node = rhs_node;
                match (&mut lhs_node, &mut rhs_node) {
                    (
                        List {
                            open_delimiter: lhs_start_content,
                            close_delimiter: lhs_end_content,
                            children: lhs_children,
                            change: lhs_change,
                            ..
                        },
                        List {
                            open_delimiter: rhs_start_content,
                            close_delimiter: rhs_end_content,
                            children: rhs_children,
                            change: rhs_change,
                            ..
                        },
                    ) => {
                        // Both sides are lists, so check the
                        // delimiters for the list node themselves, then
                        // recurse.

                        if lhs_start_content == rhs_start_content
                            && lhs_end_content == rhs_end_content
                        {
                            // We didn't see either the LHS or RHS
                            // node on the other side, but they have
                            // the same start/end, so only the
                            // children are different.
                            lhs_change.set(Unchanged);
                            rhs_change.set(Unchanged);
                        } else {
                            // Children are different and the wrapping
                            // has changed (e.g. from {} to []).
                            lhs_change.set(Removed);
                            rhs_change.set(Added);
                        }
                        walk_nodes_ordered(
                            &lhs_children[..],
                            &rhs_children[..],
                            lhs_counts,
                            rhs_counts,
                        );
                    }
                    (
                        List {
                            children: lhs_children,
                            change: lhs_change,
                            ..
                        },
                        Atom { .. },
                    ) => {
                        lhs_change.set(Removed);
                        walk_nodes_ordered(
                            &lhs_children[..],
                            std::slice::from_ref(*rhs_node),
                            lhs_counts,
                            rhs_counts,
                        );
                    }
                    (
                        Atom { .. },
                        List {
                            children: rhs_children,
                            change: rhs_change,
                            ..
                        },
                    ) => {
                        rhs_change.set(Added);
                        walk_nodes_ordered(
                            std::slice::from_ref(*lhs_node),
                            &rhs_children[..],
                            lhs_counts,
                            rhs_counts,
                        );
                    }
                    (
                        Atom {
                            change: lhs_change, ..
                        },
                        Atom {
                            change: rhs_change, ..
                        },
                    ) => {
                        lhs_change.set(Removed);
                        rhs_change.set(Added);
                    }
                }
                lhs_i += 1;
                rhs_i += 1;
            }
            (Some(lhs_node), None) => {
                let rhs_count = *rhs_counts.get(lhs_node).unwrap_or(&0);
                if rhs_count > 0 {
                    lhs_node.set_change_deep(Moved);
                    decrement(lhs_node, rhs_counts);
                } else {
                    lhs_node.set_change_deep(Removed);
                }
                lhs_i += 1;
            }
            (None, Some(rhs_node)) => {
                let lhs_count = *lhs_counts.get(rhs_node).unwrap_or(&0);
                if lhs_count > 0 {
                    rhs_node.set_change_deep(Moved);
                    decrement(rhs_node, lhs_counts);
                } else {
                    rhs_node.set_change_deep(Added);
                }
                rhs_i += 1;
            }
            (None, None) => break,
        }
    }
}

fn build_subtrees<'a>(s: &'a Syntax, subtrees: &mut HashMap<&'a Syntax, i64>) {
    let entry = subtrees.entry(s).or_insert(0);
    *entry += 1;
    match s {
        List { children, .. } => {
            for child in children {
                build_subtrees(child, subtrees);
            }
        }
        Atom { .. } => {}
    }
}

#[cfg(test)]
pub(crate) fn assert_syntaxes(actual: &[Syntax], expected: &[Syntax]) {
    if actual.len() != expected.len() {
        dbg!(actual, expected);
        assert!(false);
    } else {
        for (lhs_child, rhs_child) in actual.iter().zip(expected.iter()) {
            assert_syntax(lhs_child, rhs_child);
        }
    }
}

/// Compare all the fields in a Syntax value, not just
/// those used in its Eq implementation.
#[cfg(test)]
pub(crate) fn assert_syntax(actual: &Syntax, expected: &Syntax) {
    let mut matches = true;
    match (actual, expected) {
        (
            List {
                open_delimiter: lhs_start_content,
                close_delimiter: lhs_end_content,
                children: lhs_children,
                change: lhs_change,
                num_descendants: lhs_num_descendants,
            },
            List {
                open_delimiter: rhs_start_content,
                close_delimiter: rhs_end_content,
                children: rhs_children,
                change: rhs_change,
                num_descendants: rhs_num_descendants,
            },
        ) => {
            assert_syntaxes(lhs_children, rhs_children);
            if lhs_start_content != rhs_start_content {
                dbg!(lhs_start_content, rhs_start_content);
                matches = false;
            }
            if lhs_end_content != rhs_end_content {
                dbg!(lhs_end_content, rhs_end_content);
                matches = false;
            }
            if lhs_change != rhs_change {
                dbg!(lhs_change, rhs_change);
                matches = false;
            }
            if lhs_num_descendants != rhs_num_descendants {
                dbg!(lhs_num_descendants, rhs_num_descendants);
                matches = false;
            }
        }
        (
            Atom {
                content: lhs_content,
                change: lhs_change,
                kind: lhs_kind,
            },
            Atom {
                content: rhs_content,
                change: rhs_change,
                kind: rhs_kind,
            },
        ) => {
            if lhs_content != rhs_content {
                dbg!(lhs_content, rhs_content);
                matches = false;
            }
            if lhs_change != rhs_change {
                dbg!(lhs_change, rhs_change);
                matches = false;
            }
            if lhs_kind != rhs_kind {
                dbg!(lhs_kind, rhs_kind);
                matches = false;
            }
        }
        _ => {
            matches = false;
        }
    }
    if !matches {
        dbg!(actual, expected);
        assert!(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AtomKind::Other;

    #[test]
    fn test_atom_equality_ignores_change() {
        assert_eq!(
            Atom {
                content: "foo".into(),
                change: Cell::new(Added),
                kind: Other,
            },
            Atom {
                content: "foo".into(),
                change: Cell::new(Moved),
                kind: Other,
            }
        );
    }
}
