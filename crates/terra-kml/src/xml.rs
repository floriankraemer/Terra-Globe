//! A small XML DOM used by the KML reader.
//!
//! A DOM (rather than streaming events) keeps the KML code structurally close
//! to the TypeScript parser it replaces, including MultiGeometry recursion and
//! opaque round-tripping of views and regions.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::KmlError;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Element {
    /// Qualified name as written, e.g. `gx:Track` or `Folder`.
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// Character data directly inside this element.
    pub text: String,
}

impl Element {
    pub fn local_name(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name || key.rsplit(':').next() == Some(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.matches(name))
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children
            .iter()
            .filter(move |child| child.matches(name))
    }

    pub fn matches(&self, name: &str) -> bool {
        self.name == name || self.local_name() == name
    }

    pub fn text_trimmed(&self) -> &str {
        self.text.trim()
    }

    pub fn child_text(&self, name: &str) -> Option<String> {
        self.child(name)
            .map(|child| child.text_trimmed().to_string())
    }

    pub fn child_f64(&self, name: &str) -> Option<f64> {
        self.child_text(name)
            .and_then(|text| text.parse::<f64>().ok())
    }

    /// Flattened `parent.child` key/value map, used for opaque KML views and
    /// regions. Attributes are ignored, matching the web app.
    pub fn flatten(
        &self,
        prefix: &str,
        out: &mut std::collections::BTreeMap<String, serde_json::Value>,
    ) {
        for child in &self.children {
            let key = if prefix.is_empty() {
                child.local_name().to_string()
            } else {
                format!("{prefix}.{}", child.local_name())
            };
            if child.children.is_empty() {
                let text = child.text_trimmed();
                let value = match text.parse::<f64>() {
                    Ok(number) => serde_json::Value::from(number),
                    Err(_) => serde_json::Value::from(text),
                };
                out.insert(key, value);
            } else {
                child.flatten(&key, out);
            }
        }
    }
}

/// Parses a document into its root element. Comments and declarations are
/// skipped; namespace prefixes are preserved on element names.
pub fn parse_document(xml: &str) -> Result<Element, KmlError> {
    let mut reader = Reader::from_str(xml);

    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                let name = start.name().as_ref().to_string();
                let attributes = decode_attributes(&start)?;
                stack.push(Element {
                    name,
                    attributes,
                    children: Vec::new(),
                    text: String::new(),
                });
            }
            Ok(Event::Empty(start)) => {
                let name = start.name().as_ref().to_string();
                let attributes = decode_attributes(&start)?;
                let element = Element {
                    name,
                    attributes,
                    children: Vec::new(),
                    text: String::new(),
                };
                attach(&mut stack, &mut root, element);
            }
            Ok(Event::End(_)) => {
                let Some(element) = stack.pop() else {
                    return Err(KmlError::Xml("unexpected closing tag".to_string()));
                };
                attach(&mut stack, &mut root, element);
            }
            Ok(Event::Text(text)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(text.xml10_content().as_ref());
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if let Some(current) = stack.last_mut() {
                    match reference.resolve_char_ref() {
                        Ok(Some(character)) => current.text.push(character),
                        _ => {
                            let name = reference.into_inner();
                            let character = match name.as_ref() {
                                "amp" => Some('&'),
                                "lt" => Some('<'),
                                "gt" => Some('>'),
                                "quot" => Some('"'),
                                "apos" => Some('\''),
                                _ => None,
                            };
                            if let Some(character) = character {
                                current.text.push(character);
                            }
                        }
                    }
                }
            }
            Ok(Event::CData(data)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(data.as_ref());
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(KmlError::Xml(format!(
                    "at position {}: {error}",
                    reader.buffer_position()
                )));
            }
        }
    }

    if !stack.is_empty() {
        return Err(KmlError::Xml("unclosed element".to_string()));
    }
    root.ok_or_else(|| KmlError::Xml("document has no root element".to_string()))
}

fn attach(stack: &mut [Element], root: &mut Option<Element>, element: Element) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(element),
        None => {
            if root.is_none() {
                *root = Some(element);
            }
        }
    }
}

fn decode_attributes(
    start: &quick_xml::events::BytesStart<'_>,
) -> Result<Vec<(String, String)>, KmlError> {
    let mut attributes = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| KmlError::Xml(error.to_string()))?;
        let key = attribute.key.as_ref().to_string();
        let value = attribute
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|error| KmlError::Xml(error.to_string()))?
            .into_owned();
        attributes.push((key, value));
    }
    Ok(attributes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_elements_attributes_and_prefixes() {
        let xml = r#"<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
            <Placemark id="p1"><name>Berlin</name><ExtendedData><Data name="a"><value>1</value></Data></ExtendedData>
            <gx:Track><when>t1</when><gx:coord>1 2 3</gx:coord></gx:Track></Placemark>
        </Document></kml>"#;
        let root = parse_document(xml).expect("valid xml");
        assert_eq!(root.local_name(), "kml");
        let document = root.child("Document").expect("document");
        let placemark = document.child("Placemark").expect("placemark");
        assert_eq!(placemark.attr("id"), Some("p1"));
        assert_eq!(placemark.child_text("name").as_deref(), Some("Berlin"));
        let data = placemark
            .child("ExtendedData")
            .and_then(|ext| ext.child("Data"))
            .expect("data");
        assert_eq!(data.attr("name"), Some("a"));
        assert_eq!(data.child_text("value").as_deref(), Some("1"));
        let track = placemark.child("gx:Track").expect("track");
        assert_eq!(track.children_named("when").count(), 1);
        assert!(track.child("coord").is_some());
        assert!(track.child("gx:coord").is_some());
    }

    #[test]
    fn keeps_entities_decoded_and_text_trimmed() {
        let xml = "<a><b>  a &amp; b  </b></a>";
        let root = parse_document(xml).expect("valid");
        assert_eq!(root.child_text("b").as_deref(), Some("a & b"));
    }

    #[test]
    fn flattens_regions_with_numeric_values() {
        let xml = "<Region><LatLonAltBox><north>52.5</north></LatLonAltBox><Lod><minLodPixels>128</minLodPixels></Lod></Region>";
        let root = parse_document(xml).expect("valid");
        let mut out = std::collections::BTreeMap::new();
        root.flatten("", &mut out);
        assert_eq!(
            out.get("LatLonAltBox.north"),
            Some(&serde_json::json!(52.5))
        );
        assert_eq!(out.get("Lod.minLodPixels"), Some(&serde_json::json!(128.0)));
    }

    #[test]
    fn rejects_malformed_documents() {
        assert!(parse_document("<a><b></a>").is_err());
        assert!(parse_document("").is_err());
    }
}
