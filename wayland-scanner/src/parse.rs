use anyhow::Context as _;
use xml_iterator::Element;

use crate::protocol::*;

fn parse_num(s: &str) -> anyhow::Result<u32> {
    if s.len() < 2 || !s.starts_with("0x") {
        s.parse()
    } else {
        u32::from_str_radix(&s[2..], 16)
    }
    .with_context(|| format!("could not parse num {s}"))
}

fn parse_arg_type(bytes: &[u8]) -> Option<ArgType> {
    match bytes {
        b"new_id" => Some(ArgType::NewId),
        b"int" => Some(ArgType::Int),
        b"uint" => Some(ArgType::Uint),
        b"fixed" => Some(ArgType::Fixed),
        b"string" => Some(ArgType::String),
        b"object" => Some(ArgType::Object),
        b"array" => Some(ArgType::Array),
        b"fd" => Some(ArgType::Fd),
        _ => None,
    }
}

pub fn parse_protocol<'a>(input: &'a str) -> anyhow::Result<Protocol<'a>> {
    let mut element_iterator = xml_iterator::ElementIterator::new(input);

    let mut protocol = Protocol::default();

    let mut cur_interface: Option<Interface> = None;
    let mut cur_message: Option<Message> = None;
    let mut cur_enum: Option<Enum> = None;

    while let Some(element) = element_iterator.next() {
        match element {
            Element::StartTag(e) | Element::EmptyTag(e) if e.name == "entry" => {
                let mut entry = Entry::default();
                for attr in e.iter_attrs() {
                    match attr.key {
                        "name" => entry.name = attr.value,
                        "value" => entry.value = parse_num(attr.value)?,
                        "since" => entry.since = Some(parse_num(attr.value)?),
                        "deprecated-since" => {
                            entry.deprecated_since = Some(parse_num(&attr.value)?)
                        }
                        _ => {}
                    }
                }
                let r#enum = cur_enum.as_mut().unwrap();
                r#enum.entries.push(entry);
            }
            Element::EmptyTag(e) if e.name == "arg" => {
                let mut arg = Arg::default();
                for attr in e.iter_attrs() {
                    match attr.key {
                        "name" => arg.name = attr.value,
                        "type" => {
                            arg.r#type = parse_arg_type(attr.value.as_ref()).with_context(|| {
                                format!("unknown arg type: {}", unsafe {
                                    std::str::from_utf8_unchecked(attr.value.as_ref())
                                })
                            })?
                        }
                        "interface" => arg.interface = Some(attr.value),
                        "allow_null" => arg.allow_null = attr.value == "true",
                        "enum" => arg.r#enum = Some(attr.value),
                        _ => {}
                    }
                }
                let message = cur_message.as_mut().unwrap();
                message.args.push(arg);
            }
            Element::StartTag(e) => match e.name {
                "protocol" => {
                    for attr in e.iter_attrs() {
                        #[expect(clippy::single_match)]
                        match attr.key {
                            "name" => protocol.name = attr.value,
                            _ => {}
                        }
                    }
                }
                "interface" => {
                    assert!(cur_interface.is_none());
                    let interface = cur_interface.insert(Interface::default());
                    for attr in e.iter_attrs() {
                        match attr.key {
                            "name" => interface.name = attr.value,
                            "version" => interface.version = parse_num(&attr.value)?,
                            _ => {}
                        }
                    }
                }
                "request" | "event" => {
                    assert!(cur_message.is_none());
                    let message = cur_message.insert(Message::default());
                    for attr in e.iter_attrs() {
                        match attr.key {
                            "name" => message.name = attr.value,
                            "type" => message.r#type = Some(attr.value),
                            "since" => message.since = Some(parse_num(&attr.value)?),
                            "deprecated-since" => {
                                message.deprecated_since = Some(parse_num(&attr.value)?)
                            }
                            _ => {}
                        }
                    }
                }
                "enum" => {
                    assert!(cur_enum.is_none());
                    let r#enum = cur_enum.insert(Enum::default());
                    for attr in e.iter_attrs() {
                        match attr.key {
                            "name" => r#enum.name = attr.value,
                            "since" => r#enum.since = Some(parse_num(&attr.value)?),
                            "bitfield" => r#enum.bitfield = attr.value == "true",
                            _ => {}
                        }
                    }
                }
                _ => {}
            },
            Element::EndTag(e) => match e.name {
                "interface" => {
                    let interface = cur_interface.take().unwrap();
                    protocol.interfaces.push(interface);
                }
                "request" => {
                    let interface = cur_interface.as_mut().unwrap();
                    interface.requests.push(cur_message.take().unwrap());
                }
                "event" => {
                    let interface = cur_interface.as_mut().unwrap();
                    interface.events.push(cur_message.take().unwrap());
                }
                "enum" => {
                    let interface = cur_interface.as_mut().unwrap();
                    interface.enums.push(cur_enum.take().unwrap());
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(protocol)
}
