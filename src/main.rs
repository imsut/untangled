extern crate argparse;
extern crate classreader;
extern crate regex;

#[macro_use]
extern crate lazy_static;

use argparse::{ArgumentParser, Collect, Store, StoreTrue, StoreOption};
use classreader::{ConstantPoolInfo, Class, ClassReader};
use regex::Regex;
use std::fs::File;
use std::collections::HashMap;
use std::collections::HashSet;

pub mod lib;

lazy_static! {
    static ref CLASSNAME_REGEX: Regex = Regex::new("L([^<;]+)[<;]").unwrap();
}

fn main() {
    let mut verbose = false;
    let mut package_internal = false;
    let mut output = "out.dot".to_string();
    let mut use_fqcn = false;
    let mut exclude_regex_pattern: Option<String> = None;
    let mut classfiles: Vec<String> = Vec::new();

    {  // this block limits scope of borrows by ap.refer() method
        let mut ap = ArgumentParser::new();
        ap.set_description("Read JVM class files and visualize their dependencies.");
        ap.refer(&mut verbose).add_option(
            &["-v", "--verbose"], StoreTrue,
            "Be verbose");
        ap.refer(&mut output).add_option(
            &["-o", "--output"], Store,
            "Output file name");
        ap.refer(&mut package_internal).add_option(
            &["--package-internal"], StoreTrue,
            "extract package internal dependency only");
        ap.refer(&mut use_fqcn).add_option(
            &["--use-fqcn"], StoreTrue,
            "Use fully-qualified-class-name for display instead of short (but unique) name");
        ap.refer(&mut exclude_regex_pattern).add_option(
            &["-e", "--exclude-regex"], StoreOption,
            "Regex for classes to be excluded");
        ap.refer(&mut classfiles).add_argument(
            "classfiles", Collect,
            "Java .class files to parse");
        ap.parse_args_or_exit();
    }

    if verbose {
        println!("Output will be written to {}...", output);
    }

    let exclude_regex = exclude_regex_pattern.map(|p| {
        Regex::new(&p).expect(format!("Invalid regex pattern: {}", p).as_str())
    });


    let mut excluded = HashSet::new();

    let mut dependency: HashMap<String, HashSet<String>> = HashMap::new();
    for cls in &classfiles {
        let mut file = File::open(cls).unwrap();
        let class = ClassReader::new_from_reader(&mut file).unwrap();

        let myname = my_name(&class);

        if exclude_regex.as_ref().map(|r| r.is_match(&myname)).unwrap_or(false) {
            continue;
        }

        let referents = extract_referents(&class);

        let dependents: HashSet<String> = referents.iter()
            .filter(|c| myname != **c)
            .filter(|c| {
                let b = !exclude_regex.as_ref().map(|r| r.is_match(c)).unwrap_or(false);
                if !b {
                    excluded.insert((*c).clone());
                }
                b
            })
            .map(|c| c.clone())
            .collect();

        if package_internal {
            let package = package_of(&myname);
            dependency.insert(myname, filter_external_class(dependents, &package));
        } else {
            dependency.insert(myname, dependents);
        }
    }

    for c in excluded.iter() {
        println!("excluded: {}", c);
    }

    let deps = if use_fqcn {
        dependency
    } else {
        shorten_names(&dependency)
    };

    if verbose {
        for (k, v) in &deps {
            let vec: Vec<&String> = v.iter().collect();
            println!("{} depends on {:?}", k, vec);
        }
    }

    lib::render_to(&deps, &mut File::create(output).unwrap());
}

fn shorten_names<'a>(dependency: &HashMap<String, HashSet<String>>) -> HashMap<String, HashSet<String>> {
    let mut all = HashSet::new();
    for (k, vs) in dependency {
        all.insert(k);
        for v in vs.iter() {
            all.insert(v);
        }
    }

    let short_names = short_name_map(&all);

    let mut m = HashMap::new();
    for (k, v) in dependency {
        let new_k = short_names.get(k).unwrap();
        let new_v = v.iter()
            .map(|s| short_names.get(s).unwrap())
            .map(|r| r.clone())
            .collect();
        m.insert(new_k.clone(), new_v);
    }

    m
}

/// Given a FQCN, first remove everything but a simple class name plus a dot prepended
/// (e.g. "com.example.foo.bar.Baz" -> ".Baz")
/// If no other classes end with the string, use it with the dot removed. (e.g. "Baz")
/// If not, get back one level of package name with a dot prepended (e.g. ".bar.Baz"),
/// check uniqueness. If ok, use it with the dot removed (e.g. "bar.Baz")
fn short_name_map<'a>(names: &HashSet<&'a String>) -> HashMap<&'a String, String> {
    let mut short_names = HashMap::new();

    for fqcn in names {
        let mut i = 0;
        loop {
            i += 1;
            let proposed = create_short_name(fqcn, i);
            if is_unique(&proposed, names, fqcn) {
                short_names.insert(*fqcn, proposed);
                break;
            }
        }
    }

    short_names
}


fn create_short_name(name: &String, depth: usize) -> String {
    let tokens: Vec<&str> = name.split(".").collect();
    tokens.split_at(tokens.len() - depth).1.join(".")
}

fn is_unique(name: &String, names: &HashSet<&String>, original: &String) -> bool {
    let dot_prepended = format!(".{}", name);
    for n in names {
        if *n != original && n.ends_with(&dot_prepended) {
            return false;
        }
    }

    true
}

fn package_of(cls: &String) -> String {
    let last_dot = cls.rfind(".");
    match last_dot {
        Some(idx) => cls[..idx].to_string(),
        None => "".to_string()
    }
}

fn filter_external_class<'a>(mut classes: HashSet<String>, package: &String) -> HashSet<String> {
    classes.drain()
        .filter(|c| *package == package_of(c))
        .collect()
}


fn canonicalize(name: &String) -> String {
    let fqcn = name.replace("/", ".");
    match fqcn.find("$") {
        // make enclosed class same as its enclosing class
        Some(idx) => fqcn.split_at(idx).0.to_string(),
        None => fqcn
    }
}

fn class_name(cp_info: &ConstantPoolInfo, class: &Class) -> Vec<String> {
    match cp_info {
        &ConstantPoolInfo::Class(ref idx) =>
            match class.constant_pool[(idx - 1) as usize] {
                ConstantPoolInfo::Utf8(ref s) => vec![canonicalize(s)],
                _ => vec![]
            },
        &ConstantPoolInfo::Utf8(ref s) => extract_classnames(s),
        _ => vec![]
    }
}

fn extract_classnames(text: &String) -> Vec<String> {
    CLASSNAME_REGEX.captures_iter(text)
        .filter_map(|c| c.get(1).map(|m| String::from(m.as_str())))
        .map(|s| canonicalize(&s))
        .collect()
}

fn my_name(class: &Class) -> String {
    let mut names = class_name(&class.constant_pool[(class.this_class - 1) as usize], class);
    if names.len() > 0 {
        names.swap_remove(0)
    } else {
        panic!("Failed to find this class name in {:?}", class)
    }
}

fn extract_referents(cls: &Class) -> HashSet<String> {
    let mut referents: HashSet<String> = HashSet::new();
    for c in &cls.constant_pool {
        referents.extend(class_name(c, cls));
    }
    referents
}
