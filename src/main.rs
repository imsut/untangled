extern crate argparse;
extern crate classreader;

use argparse::{ArgumentParser, Collect, StoreTrue, Store};
use classreader::{ConstantPoolInfo, Class, ClassReader};
use std::fs::File;
use std::collections::HashMap;
use std::collections::HashSet;

pub mod lib;

fn main() {
    let mut verbose = false;
    let mut package_internal = false;
    let mut output = "out.dot".to_string();
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
        ap.refer(&mut classfiles).add_argument(
            "classfiles", Collect,
            "Java .class files to parse");
        ap.parse_args_or_exit();
    }

    if verbose {
        println!("Output will be written to {}...", output);
    }

    let mut dependency: HashMap<String, HashSet<String>> = HashMap::new();
    for cls in &classfiles {
        let mut file = File::open(cls).unwrap();
        let class = ClassReader::new_from_reader(&mut file).unwrap();

        let myname = my_name(&class);
        let mut referents = extract_referents(&class);
        referents.remove(&myname);
        let package = package_of(&myname);

        if package_internal {
            dependency.insert(myname, filter_external_class(referents, &package));
        } else {
            dependency.insert(myname, referents);
        }
    }

    if verbose {
        println!("dependency: {:?}", dependency);
    }

    lib::render_to(&dependency, &mut File::create(output).unwrap());
}

fn package_of(cls: &String) -> String {
    let last_dot = cls.rfind(".");
    match last_dot {
        Some(idx) => cls[..idx].to_string(),
        None => "".to_string()
    }
}

fn filter_external_class(classes: HashSet<String>, package: &String) -> HashSet<String> {
    let mut retained = HashSet::new();
    for c in classes {
        if *package == package_of(&c) {
            retained.insert(c.clone());
        }
    }
    retained
}


fn canonicalize(name: &String) -> String {
    name.replace("/", ".")
}

fn class_name(cp_info: &ConstantPoolInfo, class: &Class) -> Option<String> {
    match cp_info {
        &ConstantPoolInfo::Class(ref idx) => Some(idx),
        _ => None
    }.map(|idx| {
        match class.constant_pool[(idx - 1) as usize] {
            ConstantPoolInfo::Utf8(ref s) => s.clone(),
            _ => panic!("Invalid class file")
        }}
    ).map(|name| canonicalize(&name))
}

fn my_name(class: &Class) -> String {
    class_name(&class.constant_pool[(class.this_class - 1) as usize], class)
        .expect(format!("Failed to find this class name in {:?}", class).as_str())
}

fn extract_referents(cls: &Class) -> HashSet<String> {
    let mut referents: HashSet<String> = HashSet::new();
    for c in &cls.constant_pool {
        class_name(c, cls).map(|name| {
            referents.insert(name);
            ()
        });
    }
    referents
}
