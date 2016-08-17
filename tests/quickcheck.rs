#![feature(custom_derive, plugin, test)]
#![feature(custom_attribute)]
#![plugin(serde_macros)]

#[macro_use]
extern crate log;

extern crate test;
extern crate serde;
extern crate serde_xml;
extern crate glob;

use serde_xml::from_str;
use serde_xml::value::Element;

#[macro_use]
extern crate quickcheck;

#[derive(Clone, Debug)]
struct XmlConf(String);

impl quickcheck::Arbitrary for XmlConf {
    fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> Self {
        use std::io::Read;
        let n = glob::glob("xmlconf/**/*.xml").expect("Failed to read glob pattern").filter(|e| e.is_ok()).count();
        loop {
            let path = glob::glob("xmlconf/**/*.xml").expect("Failed to read glob pattern").filter(|e| e.is_ok()).nth(g.gen_range(0, n)).unwrap().unwrap();
            let mut f = std::fs::File::open(&path).unwrap();
            let mut s = String::new();
            if f.read_to_string(&mut s).is_err() {
                // there are files in the xmlconf suite that aren't utf8
                continue;
            }
            return XmlConf(s);
        }
    }

    fn shrink(&self) -> Box<Iterator<Item=Self>> {
        Box::new(self.0.shrink().map(XmlConf))
    }
}

quickcheck! {
    fn dont_panic(s: String) -> bool {
        let _: Result<Element, _> = from_str(&s);
        // just check that we don't panic on any input
        true
    }
    fn dont_panic2(s: XmlConf) -> bool {
        let _: Result<Element, _> = from_str(&s.0);
        // just check that we don't panic on any input
        true
    }
}
