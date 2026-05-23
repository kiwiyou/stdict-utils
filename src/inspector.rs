use std::collections::{HashMap, HashSet};

use quick_xml::events::Event as XmlEvent;

fn main() {
    let mut samples: HashMap<String, Samples> = HashMap::new();
    for read_dir in std::fs::read_dir("vendor/stdict").unwrap() {
        let entry = read_dir.unwrap();
        let mut xml_reader = quick_xml::Reader::from_file(entry.path()).unwrap();
        let mut work_buffer = vec![];
        let mut current_xml_path = vec![];
        let mut xml_version = None;
        loop {
            match xml_reader.read_event_into(&mut work_buffer).unwrap() {
                XmlEvent::Eof => break,
                XmlEvent::Decl(bytes_decl) => {
                    if let Ok(version) = bytes_decl.xml_version() {
                        xml_version = Some(version);
                    }
                }
                XmlEvent::Start(bytes_start) => {
                    let name: String = String::from_utf8_lossy(bytes_start.name().0).into();
                    current_xml_path.push(name);
                }
                XmlEvent::End(_) => {
                    current_xml_path.pop();
                }
                XmlEvent::Text(mut bytes_text) => {
                    bytes_text.inplace_trim_start();
                    if bytes_text.inplace_trim_end() {
                        continue;
                    }
                    let content = bytes_text.xml_content(xml_version.unwrap()).unwrap().into();
                    samples
                        .entry(join_path(&current_xml_path))
                        .or_default()
                        .observe(content);
                }
                XmlEvent::CData(bytes_cdata) => {
                    let content: String = bytes_cdata
                        .xml_content(xml_version.unwrap())
                        .unwrap()
                        .into();
                    samples
                        .entry(join_path(&current_xml_path))
                        .or_default()
                        .observe(content);
                }
                XmlEvent::Empty(_) => todo!(),
                XmlEvent::Comment(_) => todo!(),
                XmlEvent::PI(_) => todo!(),
                XmlEvent::DocType(_) => todo!(),
                XmlEvent::GeneralRef(_) => todo!(),
            }
        }
        let file_name = entry
            .file_name()
            .into_string()
            .unwrap_or_else(|_| "<error>".into());
        eprintln!("done reading {file_name}");
    }
    let mut samples: Vec<(String, Samples)> = samples.into_iter().collect();
    samples.sort_by(|(path1, _), (path2, _)| path1.cmp(path2));
    for (path, sample) in samples {
        println!("{path}");
        match sample.kind {
            SampleKind::Number => println!("- number"),
            SampleKind::Text => {
                if sample.is_observed_full() {
                    println!("- text: {}", sample.observed.iter().next().unwrap());
                } else {
                    println!("- variants:");
                    for variant in sample.observed {
                        println!("  - {variant}")
                    }
                }
            }
        }
    }
}

fn join_path(components: &[String]) -> String {
    components.iter().fold(String::new(), |mut joined, item| {
        joined.push('.');
        joined.push_str(item);
        joined
    })
}

struct Samples {
    kind: SampleKind,
    observed: HashSet<String>,
}

impl Default for Samples {
    fn default() -> Self {
        Self {
            kind: SampleKind::Number,
            observed: HashSet::new(),
        }
    }
}

impl Samples {
    const MAX_OBSERVE_COUNT: usize = 100;

    fn observe(&mut self, text: String) {
        if self.is_observed_full() {
            return;
        }
        let text = text;
        if self.kind == SampleKind::Number && !text.bytes().all(|byte| byte.is_ascii_digit()) {
            self.kind = SampleKind::Text;
        }
        self.observed.insert(text);
    }

    fn is_observed_full(&self) -> bool {
        self.observed.len() >= Samples::MAX_OBSERVE_COUNT
    }
}

#[derive(PartialEq, Eq)]
enum SampleKind {
    Number,
    Text,
}
