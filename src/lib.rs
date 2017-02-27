extern crate dot;

use std::borrow::Cow;
use std::io::Write;
use std::collections::HashMap;
use std::collections::HashSet;

#[test]
fn it_works() {
}

type Nd = String;
type Ed = (String, String);
struct Edges(Vec<Ed>);

pub fn render_to<W: Write>(dependency: &HashMap<String, Vec<String>>, output: &mut W) {
    /*
    let mut all = HashSet::new();
    for (k, vs) in map {
        all.insert(k.clone());
        for v in vs {
            all.insert(v.clone());
        }
    }
    */

    let edges = Edges(flatten(dependency));
    dot::render(&edges, output).unwrap()
}

fn flatten(map: &HashMap<String, Vec<String>>) -> Vec<Ed> {
    let mut edges = Vec::new();
    for (k, vs) in map {
        for v in vs {
            edges.push((k.clone(), v.clone()));
        }
    }
    edges
}

impl<'a> dot::Labeller<'a, Nd, Ed> for Edges {
    fn graph_id(&'a self) -> dot::Id<'a> {
        dot::Id::new("Dependency").unwrap()
    }

    fn node_id(&'a self, n: &Nd) -> dot::Id<'a> {
        let name = n.replace(".", "_").replace("$", "_");
        let res = dot::Id::new(name);
        if res.is_err() {
            println!("Failed to create Id: {:?}", n);
            return dot::Id::new(format!("addr_{:p}", n)).unwrap()
        } else {
            res.unwrap()
        }
    }
}

impl<'a> dot::GraphWalk<'a, Nd, Ed> for Edges {
    fn nodes(&self) -> dot::Nodes<'a, Nd> {
        // (assumes that |N| \approxeq |E|)
        let &Edges(ref v) = self;
        let mut nodes = Vec::with_capacity(v.len());
        for (s, t) in v.clone() {
            nodes.push(s);
            nodes.push(t);
        }
        nodes.sort();
        nodes.dedup();
        Cow::Owned(nodes)
    }

    fn edges(&'a self) -> dot::Edges<'a,Ed> {
        let &Edges(ref edges) = self;
        Cow::Borrowed(&edges[..])
    }

    fn source(&self, e: &Ed) -> Nd { e.0.clone() }

    fn target(&self, e: &Ed) -> Nd { e.1.clone() }
}
