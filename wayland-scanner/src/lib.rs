use std::io;

use anyhow::Context as _;
use xml_iterator::Element;

// https://gitlab.freedesktop.org/wayland/wayland/-/blob/9cb3d7aa9dc995ffafdbdef7ab86a949d0fb0e7d/protocol/wayland.dtd

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ArgType {
    #[default]
    NewId,
    Int,
    Uint,
    Fixed,
    String,
    Object,
    Array,
    Fd,
}

#[derive(Debug, Default)]
pub struct Arg<'a> {
    pub name: &'a str,
    pub r#type: ArgType,
    pub interface: Option<&'a str>,
    pub allow_null: bool,
    pub r#enum: Option<&'a str>,
}

#[derive(Debug, Default)]
pub struct Message<'a> {
    pub name: &'a str,
    pub r#type: Option<&'a str>,
    pub since: Option<u32>,
    pub deprecated_since: Option<u32>,
    pub args: Vec<Arg<'a>>,
}

#[derive(Debug, Default)]
pub struct Entry<'a> {
    pub name: &'a str,
    pub value: u32,
    pub since: Option<u32>,
    pub deprecated_since: Option<u32>,
}

#[derive(Debug, Default)]
pub struct Enum<'a> {
    pub name: &'a str,
    pub since: Option<u32>,
    pub bitfield: bool,
    pub entries: Vec<Entry<'a>>,
}

#[derive(Debug, Default)]
pub struct Interface<'a> {
    pub name: &'a str,
    pub version: u32,
    pub requests: Vec<Message<'a>>,
    pub events: Vec<Message<'a>>,
    pub enums: Vec<Enum<'a>>,
}

#[derive(Debug, Default)]
pub struct Protocol<'a> {
    pub name: &'a str,
    pub interfaces: Vec<Interface<'a>>,
}

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

// is_nullable_type
fn is_arg_type_nullable(arg_type: ArgType) -> bool {
    arg_type == ArgType::String || arg_type == ArgType::Object
}

// from emit_messages
fn emit_message_signature<W: io::Write>(w: &mut W, msg: &Message) -> io::Result<()> {
    if msg.since.is_some_and(|since| since > 1) {
        write!(w, "{}", msg.since.unwrap())?;
    }

    for arg in msg.args.iter() {
        if is_arg_type_nullable(arg.r#type) && arg.allow_null {
            write!(w, "?")?;
        }

        match arg.r#type {
            ArgType::Int => write!(w, "i")?,
            ArgType::Uint => write!(w, "u")?,
            ArgType::Fixed => write!(w, "f")?,
            ArgType::String => write!(w, "s")?,
            ArgType::Object => write!(w, "o")?,
            ArgType::Array => write!(w, "a")?,
            ArgType::NewId => {
                if arg.interface.is_none() {
                    write!(w, "su")?;
                }
                write!(w, "n")?;
            }
            ArgType::Fd => write!(w, "h")?,
        }
    }

    Ok(())
}

// emit_type
fn emit_arg_type<W: io::Write>(w: &mut W, arg: &Arg) -> io::Result<()> {
    match arg.r#type {
        ArgType::Int | ArgType::Fd => write!(w, "i32"),
        ArgType::NewId | ArgType::Uint => write!(w, "u32"),
        ArgType::Fixed => write!(w, "super::wl_fixed"),
        ArgType::String => write!(w, "*const std::ffi::c_char"),
        ArgType::Object => write!(w, "*mut {}", arg.interface.expect("interface name")),
        ArgType::Array => write!(w, "*mut super::wl_array"),
    }
}

enum MessageKind {
    Request,
    Event,
}

fn normalize_message_name(name: &str) -> &str {
    match name {
        "type" => "r#type",
        other => other,
    }
}

fn emit_structs<W: io::Write>(
    w: &mut W,
    interface: &Interface,
    kind: MessageKind,
) -> io::Result<()> {
    write!(w, "#[repr(C)]\n")?;
    write!(w, "pub struct {} {{\n", interface.name)?;
    write!(w, "    _data: [u8; 0],\n")?;
    write!(
        w,
        "    _marker: std::marker::PhantomData<(*mut u8, std::marker::PhantomPinned)>,\n"
    )?;
    write!(w, "}}\n\n")?;

    let messages = match kind {
        MessageKind::Request => &interface.requests,
        MessageKind::Event => &interface.events,
    };

    if messages.is_empty() {
        return Ok(());
    }

    write!(w, "pub struct {}_listener {{\n", interface.name)?;
    for msg in messages.iter() {
        let name = normalize_message_name(msg.name);
        write!(w, "    pub {name}: unsafe extern \"C\" fn(\n")?;
        write!(w, "        data: *mut std::ffi::c_void,\n")?;
        write!(w, "        {}: *mut {},\n", interface.name, interface.name)?;
        for arg in msg.args.iter() {
            write!(w, "        {}: ", arg.name)?;
            match arg.r#type {
                ArgType::Object if arg.interface.is_none() => write!(w, "*mut std::ffi::c_void")?,
                ArgType::NewId => write!(w, "*mut {}", arg.interface.expect("interface name"))?,
                _ => emit_arg_type(w, arg)?,
            }
            write!(w, ",\n")?;
        }
        write!(w, "    ),\n")?;
    }
    write!(w, "}}\n\n")?;

    Ok(())
}

fn emit_messages<W: io::Write>(
    w: &mut W,
    interface: &Interface,
    kind: MessageKind,
) -> io::Result<()> {
    let (kind_name, messages) = match kind {
        MessageKind::Request => ("requests", &interface.requests),
        MessageKind::Event => ("events", &interface.events),
    };

    if messages.is_empty() {
        return Ok(());
    }

    for msg in messages.iter() {
        write!(
            w,
            "static {}_{}_types: SyncWrapper<[*const super::wl_interface; {}]> = SyncWrapper([\n",
            interface.name,
            msg.name,
            msg.args.len(),
        )?;
        for arg in msg.args.iter() {
            match arg.r#type {
                ArgType::NewId | ArgType::Object if arg.interface.is_some() => {
                    write!(
                        w,
                        "    &{}_interface as *const super::wl_interface,\n",
                        arg.interface.unwrap()
                    )?;
                }
                _ => write!(w, "    std::ptr::null(),\n")?,
            }
        }
        write!(w, "]);\n\n")?;
    }

    write!(
        w,
        "static {}_{kind_name}: [super::wl_message; {}] = [\n",
        interface.name,
        messages.len()
    )?;
    for msg in messages.iter() {
        write!(w, "    super::wl_message {{\n")?;

        write!(w, "        name: c\"{}\".as_ptr(),\n", msg.name)?;

        write!(w, "        signature: c\"")?;
        emit_message_signature(w, msg)?;
        write!(w, "\".as_ptr(),\n",)?;

        write!(
            w,
            "        types: {}_{}_types.0.as_ptr(),\n",
            interface.name, msg.name
        )?;

        write!(w, "    }},\n")?;
    }
    write!(w, "];\n\n")?;

    Ok(())
}

fn emit_enums<W: io::Write>(w: &mut W, interface: &Interface) -> io::Result<()> {
    for e in interface.enums.iter() {
        for entry in e.entries.iter() {
            write!(
                w,
                "pub const {}_{}_{}: u32 = {};\n",
                interface.name.to_uppercase(),
                e.name.to_uppercase(),
                entry.name.to_uppercase(),
                entry.value,
            )?;
        }

        write!(w, "\n")?;
    }

    Ok(())
}

fn emit_opcodes<W: io::Write>(
    w: &mut W,
    interface: &Interface,
    kind: MessageKind,
) -> io::Result<()> {
    let messages = match kind {
        MessageKind::Request => &interface.requests,
        MessageKind::Event => &interface.events,
    };

    if messages.is_empty() {
        return Ok(());
    }

    for (opcode, msg) in messages.iter().enumerate() {
        write!(
            w,
            "const {}_{}: u32 = {};\n",
            interface.name.to_uppercase(),
            msg.name.to_uppercase(),
            opcode
        )?;
    }

    write!(w, "\n")
}

fn emit_stubs<W: io::Write>(w: &mut W, interface: &Interface) -> io::Result<()> {
    for msg in interface.requests.iter() {
        let new_id_count = msg
            .args
            .iter()
            .fold(0, |acc, arg| acc + (arg.r#type == ArgType::NewId) as usize);
        assert!(new_id_count <= 1);

        let ret = msg.args.iter().find(|arg| arg.r#type == ArgType::NewId);

        write!(w, "#[inline]\n",)?;
        write!(w, "pub unsafe fn {}_{}(\n", interface.name, msg.name,)?;

        // args

        write!(w, "    lib: &super::Lib,\n",)?;
        write!(w, "    {}: *mut {},\n", interface.name, interface.name,)?;

        for arg in msg.args.iter() {
            if arg.r#type == ArgType::NewId {
                if arg.interface.is_none() {
                    write!(w, "    interface: *const super::wl_interface,\n")?;
                    write!(w, "    version: u32,\n")?;
                }
                continue;
            }

            write!(w, "    {}: ", arg.name)?;
            emit_arg_type(w, arg)?;
            write!(w, ",\n")?;
        }

        // return

        if let Some(ret) = ret {
            if let Some(interface_name) = ret.interface {
                write!(w, ") -> *mut {} {{\n", interface_name)?;
            } else {
                write!(w, ") -> *mut std::ffi::c_void {{\n")?;
            }
        } else {
            write!(w, ") {{ \n")?;
        }

        // body

        write!(w, "    unsafe {{\n")?;
        write!(w, "        (lib.wl_proxy_marshal_flags)(\n",)?;

        // proxy: *mut super::wl_proxy,
        write!(
            w,
            "            {} as *mut super::wl_proxy,\n",
            interface.name
        )?;
        // opcode: u32,
        write!(
            w,
            "            {}_{},\n",
            interface.name.to_uppercase(),
            msg.name.to_uppercase()
        )?;
        // interface: *const wl_interface,
        if let Some(ret) = ret {
            if let Some(interface_name) = ret.interface {
                write!(w, "            &{}_interface,\n", interface_name)?;
            } else {
                write!(w, "            interface,\n")?;
            }
        } else {
            write!(w, "            std::ptr::null(),\n")?;
        }
        // version: u32,
        if ret.is_some_and(|arg| arg.interface.is_none()) {
            write!(w, "            version,\n")?;
        } else {
            write!(
                w,
                "            (lib.wl_proxy_get_version)({} as *mut super::wl_proxy),\n",
                interface.name
            )?;
        }
        // flags: u32,
        if msg.r#type.is_some_and(|r#type| r#type == "destructor") {
            write!(w, "            super::WL_MARSHAL_FLAG_DESTROY,\n")?;
        } else {
            write!(w, "            0,\n")?;
        }
        // ...
        for arg in msg.args.iter() {
            if arg.r#type == ArgType::NewId {
                if arg.interface.is_none() {
                    write!(w, "            (*interface).name,\n")?;
                    write!(w, "            version,\n")?;
                }
                write!(w, "            std::ptr::null::<std::ffi::c_void>(),\n")?;
            } else {
                write!(w, "            {},\n", arg.name)?;
            }
        }

        if ret.is_some() {
            write!(w, "        ) as _\n")?;
        } else {
            write!(w, "        );\n")?;
        }

        write!(w, "    }}\n")?;
        write!(w, "}}\n\n")?;
    }

    Ok(())
}

fn emit_interface<W: io::Write>(w: &mut W, interface: &Interface) -> io::Result<()> {
    emit_structs(w, interface, MessageKind::Event)?;
    emit_messages(w, interface, MessageKind::Request)?;
    emit_messages(w, interface, MessageKind::Event)?;

    // TODO: consider wrapping this into whatever proxy struct as consts. and do the same with
    // _events and _requests.
    write!(
        w,
        "pub static {}_interface: super::wl_interface = super::wl_interface {{\n",
        interface.name,
    )?;
    write!(w, "    name: c\"{}\".as_ptr(),\n", interface.name)?;
    write!(w, "    version: {},\n", interface.version)?;
    write!(w, "    method_count: {},\n", interface.requests.len())?;
    if !interface.requests.is_empty() {
        write!(w, "    methods: {}_requests.as_ptr(),\n", interface.name)?;
    } else {
        write!(w, "    methods: std::ptr::null(),\n")?;
    }
    write!(w, "    event_count: {},\n", interface.events.len())?;
    if !interface.events.is_empty() {
        write!(w, "    events: {}_events.as_ptr(),\n", interface.name)?;
    } else {
        write!(w, "    events: std::ptr::null(),\n")?;
    }
    write!(w, "}};\n\n")?;

    emit_enums(w, interface)?;
    emit_opcodes(w, interface, MessageKind::Request)?;
    emit_stubs(w, interface)?;

    Ok(())
}

// TODO: consider introducing some kind of generate_header or something method that would write,
// well, header. with:
//     struct SyncWrapper<T>(T);
//     unsafe impl<T> Sync for SyncWrapper<T> {}
pub fn generate_protocol<W: io::Write>(w: &mut W, protocol: &Protocol) -> io::Result<()> {
    for interface in protocol.interfaces.iter() {
        emit_interface(w, interface)?;
    }

    Ok(())
}
