use std::collections::HashSet;
use std::io;
use std::str::FromStr;

use anyhow::{Context as _, bail};

const BOILERPLATE: &str = r#"
use std::ffi::{c_char, c_double, c_float, c_int, c_short, c_uchar, c_uint, c_ushort, c_void};
use std::mem::transmute;

#[cold]
#[inline(never)]
fn null_fn_ptr_panic() -> ! {
    panic!("function was not loaded")
}

struct FnPtr {
    ptr: *const c_void,
}

impl FnPtr {
    fn new(ptr: *const c_void) -> FnPtr {
        if ptr.is_null() {
            FnPtr { ptr: null_fn_ptr_panic as *const c_void }
        } else {
            FnPtr { ptr }
        }
    }
}  

pub type GLbitfield = c_uint;
pub type GLboolean = c_uchar;
pub type GLbyte = c_char;
pub type GLchar = c_char;
pub type GLdouble = c_double;
pub type GLenum = c_uint;
pub type GLfloat = c_float;
pub type GLint = c_int;
pub type GLint64 = i64;
pub type GLintptr = isize;
pub type GLshort = c_short;
pub type GLsizei = c_int;
pub type GLsizeiptr = isize;
pub type GLsync = *mut c_void;
pub type GLubyte = c_uchar;
pub type GLuint = c_uint;
pub type GLuint64 = u64;
pub type GLushort = c_ushort;

pub type GLDEBUGPROC = Option<extern "system" fn(
    source: GLenum,
    type_: GLenum,
    id: GLuint,
    severity: GLenum,
    length: GLsizei,
    message: *const GLchar,
    userParam: *mut c_void,
)>;
"#;

// xml spec:
// https://github.com/KhronosGroup/OpenGL-Registry/blob/8e772a3b0c9e8a85ccb6f471b4cdbf94c8bcd71d/xml/readme.pdf

#[derive(Debug)]
pub struct Enum<'a> {
    pub value: &'a str,
    pub name: &'a str,
    pub api: Option<&'a str>,
    pub ty: &'a str,
    pub group: Option<&'a str>,
    pub alias: Option<&'a str>,
    pub comment: Option<&'a str>,
}

#[derive(Debug)]
pub enum CommandTypePart<'a> {
    Defined(&'a str),
    Other(&'a str),
}

#[derive(Debug)]
pub struct CommandProto<'a> {
    pub return_type_parts: Vec<CommandTypePart<'a>>,
    pub name: &'a str,
}

#[derive(Debug)]
pub struct CommandParam<'a> {
    pub type_parts: Vec<CommandTypePart<'a>>,
    pub name: &'a str,
}

#[derive(Debug)]
pub struct Command<'a> {
    pub proto: CommandProto<'a>,
    pub params: Vec<CommandParam<'a>>,
}

#[derive(Debug)]
pub struct Interface<'a> {
    pub enums: Vec<&'a str>,
    pub commands: Vec<&'a str>,
}

#[derive(Debug)]
pub struct Feature<'a> {
    pub api: &'a str,
    pub name: &'a str,
    pub number: &'a str,
    pub requires: Vec<Interface<'a>>,
    pub removes: Vec<Interface<'a>>,
}

#[derive(Debug)]
pub struct Extension<'a> {
    pub name: &'a str,
    pub supported: &'a str,
    pub requires: Vec<Interface<'a>>,
}

#[derive(Debug)]
pub struct Registry<'a> {
    pub enums: Vec<Enum<'a>>,
    pub commands: Vec<Command<'a>>,
    pub features: Vec<Feature<'a>>,
    pub extensions: Vec<Extension<'a>>,
}

fn expect_text<'a>(
    element_iterator: &mut xml_iterator::ElementIterator<'a>,
) -> anyhow::Result<&'a str> {
    let Some(element) = element_iterator.next() else {
        bail!("unexpected eof");
    };
    let xml_iterator::Element::Text(text) = element else {
        bail!("unexpected element (got {element:?}, want text)");
    };
    Ok(&text)
}

fn expect_end_tag<'a>(
    element_iterator: &mut xml_iterator::ElementIterator<'a>,
) -> anyhow::Result<()> {
    let Some(element) = element_iterator.next() else {
        bail!("unexpected eof");
    };
    let xml_iterator::Element::EndTag(_) = element else {
        bail!("unexpected element (got {element:?}, want end)");
    };
    Ok(())
}

fn get_enum_type<'a>(start_tag: xml_iterator::StartTag<'a>) -> anyhow::Result<&'static str> {
    let ty = start_tag
        .iter_attrs()
        .find(|attr| attr.key == "type")
        .map(|attr| attr.value);
    match ty {
        Some("bitmask") => Ok("GLbitfield"),
        None => Ok("GLenum"),
        other => bail!("unknown enum block ty: {other:?}"),
    }
}

fn parse_enum_token_attrs<'a>(
    empty_tag: xml_iterator::EmptyTag<'a>,
    block_type: &'static str,
) -> anyhow::Result<Enum<'a>> {
    let mut value: Option<&'a str> = None;
    let mut name: Option<&'a str> = None;
    let mut api: Option<&'a str> = None;
    let mut ty: Option<&'a str> = None;
    let mut group: Option<&'a str> = None;
    let mut alias: Option<&'a str> = None;
    let mut comment: Option<&'a str> = None;
    for attr in empty_tag.iter_attrs() {
        let prev = match attr.key {
            "value" => value.replace(attr.value),
            "name" => name.replace(attr.value),
            "api" => api.replace(attr.value),
            "type" => ty.replace(attr.value),
            "group" => group.replace(attr.value),
            "alias" => alias.replace(attr.value),
            "comment" => comment.replace(attr.value),
            other => bail!("unexpected attr: {:?}", other),
        };
        if prev.is_some() {
            bail!("duplicate attr: {prev:?}");
        }
    }
    Ok(Enum {
        value: value.context("value is missing")?,
        name: name.context("name is missing")?,
        api,
        ty: ty
            .map(|ty| match ty {
                "u" => "GLuint",
                "ull" => "GLuint64",
                _ => ty,
            })
            .unwrap_or(block_type),
        group,
        alias,
        comment,
    })
}

fn parse_enum_block_into<'a>(
    start_tag: xml_iterator::StartTag<'a>,
    element_iterator: &mut xml_iterator::ElementIterator<'a>,
    enums: &mut Vec<Enum<'a>>,
) -> anyhow::Result<()> {
    let block_type = get_enum_type(start_tag)?;
    while let Some(element) = element_iterator.next() {
        match element {
            xml_iterator::Element::EmptyTag(empty) => match empty.name {
                "enum" => {
                    let token = parse_enum_token_attrs(empty, block_type)
                        .context("could not parse enum token attrs")?;
                    enums.push(token);
                }
                "unused" => {}
                other => bail!("unexpected empty: {:?}", other),
            },
            xml_iterator::Element::Text(text) if text.chars().all(|c| c.is_whitespace()) => {}
            xml_iterator::Element::EndTag(end) if end.name == "enums" => break,
            xml_iterator::Element::Comment(_) => {}
            other => bail!("unexpected element: {other:?}"),
        }
    }
    Ok(())
}

fn parse_command_proto<'a>(
    element_iterator: &mut xml_iterator::ElementIterator<'a>,
) -> anyhow::Result<CommandProto<'a>> {
    let mut return_ty_parts: Vec<CommandTypePart<'a>> = Vec::new();
    let mut name: Option<&'a str> = None;
    while let Some(element) = element_iterator.next() {
        match element {
            xml_iterator::Element::Text(text) => {
                if !text.chars().all(|c| c.is_whitespace()) {
                    return_ty_parts.push(CommandTypePart::Other(text.trim()));
                }
            }
            xml_iterator::Element::StartTag(start) => match start.name {
                "name" => {
                    assert!(name.is_none());
                    name = Some(expect_text(element_iterator)?);
                    expect_end_tag(element_iterator)?;
                }
                "ptype" => {
                    return_ty_parts.push(CommandTypePart::Defined(expect_text(element_iterator)?));
                    expect_end_tag(element_iterator)?;
                }
                other => bail!("unexpected start: {other}"),
            },
            xml_iterator::Element::EndTag(end) if end.name == "proto" => break,
            other => bail!("unexpected element: {other:?}"),
        }
    }
    Ok(CommandProto {
        return_type_parts: return_ty_parts,
        name: name.context("proto name is missing")?,
    })
}

fn parse_command_param<'a>(
    element_iterator: &mut xml_iterator::ElementIterator<'a>,
) -> anyhow::Result<CommandParam<'a>> {
    let mut ty_parts: Vec<CommandTypePart<'a>> = Vec::new();
    let mut name: Option<&'a str> = None;
    while let Some(element) = element_iterator.next() {
        match element {
            xml_iterator::Element::StartTag(start) => match start.name {
                "ptype" => {
                    ty_parts.push(CommandTypePart::Defined(expect_text(element_iterator)?));
                    expect_end_tag(element_iterator)?;
                }
                "name" => {
                    assert!(name.is_none());
                    name = Some(expect_text(element_iterator)?);
                    expect_end_tag(element_iterator)?;
                }
                other => bail!("unexpected start: {other}"),
            },
            xml_iterator::Element::Text(text) => {
                if !text.chars().all(|c| c.is_whitespace()) {
                    ty_parts.push(CommandTypePart::Other(text.trim()));
                }
            }
            xml_iterator::Element::EndTag(end) if end.name == "param" => break,
            other => bail!("unexpected element: {other:?}"),
        }
    }
    return Ok(CommandParam {
        type_parts: ty_parts,
        name: name.take().context("param name is missing")?,
    });
}

fn parse_command<'a>(
    element_iterator: &mut xml_iterator::ElementIterator<'a>,
) -> anyhow::Result<Command<'a>> {
    let mut proto: Option<CommandProto<'a>> = None;
    let mut params: Vec<CommandParam<'a>> = Vec::new();
    while let Some(element) = element_iterator.next() {
        match element {
            xml_iterator::Element::StartTag(start) => match start.name {
                "proto" => {
                    assert!(proto.is_none());
                    proto = Some(
                        parse_command_proto(element_iterator)
                            .context("could not parse command proto")?,
                    );
                }
                "param" => {
                    params.push(
                        parse_command_param(element_iterator)
                            .context("could not parse command param")?,
                    );
                }
                other => bail!("unexpected start: {other}"),
            },
            xml_iterator::Element::EndTag(end) => match end.name {
                "command" => break,
                other => bail!("unexpected end: {other}"),
            },
            xml_iterator::Element::Text(text) if text.chars().all(|c| c.is_whitespace()) => {}
            xml_iterator::Element::EmptyTag(empty)
                if matches!(empty.name, "glx" | "alias" | "vecequiv") => {}
            other => bail!("unexpected element: {other:?}"),
        }
    }
    Ok(Command {
        proto: proto.context("proto is missing")?,
        params,
    })
}

fn parse_interface<'a>(
    tag_name: &str,
    element_iterator: &mut xml_iterator::ElementIterator<'a>,
) -> anyhow::Result<Interface<'a>> {
    let mut enums: Vec<&'a str> = Vec::new();
    let mut commands: Vec<&'a str> = Vec::new();
    while let Some(element) = element_iterator.next() {
        match element {
            xml_iterator::Element::Text(text) if text.chars().all(|c| c.is_whitespace()) => {}
            xml_iterator::Element::EmptyTag(empty) => match empty.name {
                "type" => {}
                "enum" => {
                    let name = empty
                        .iter_attrs()
                        .find(|attr| attr.key == "name")
                        .context("name is missing")?;
                    enums.push(name.value);
                }
                "command" => {
                    let name = empty
                        .iter_attrs()
                        .find(|attr| attr.key == "name")
                        .context("name is missing")?;
                    commands.push(name.value);
                }
                other => bail!("unexpected empty: {other}"),
            },
            xml_iterator::Element::EndTag(end) if end.name == tag_name => break,
            xml_iterator::Element::Comment(_) => {}
            other => bail!("unexpected element: {other:?}"),
        }
    }
    Ok(Interface { enums, commands })
}

fn parse_feature_attrs<'a>(start_tag: xml_iterator::StartTag<'a>) -> anyhow::Result<Feature<'a>> {
    let mut api: Option<&'a str> = None;
    let mut name: Option<&'a str> = None;
    let mut number: Option<&'a str> = None;
    for attr in start_tag.iter_attrs() {
        let prev = match attr.key {
            "api" => api.replace(attr.value),
            "name" => name.replace(attr.value),
            "number" => number.replace(attr.value),
            other => bail!("unexpected attr: {other}"),
        };
        if prev.is_some() {
            bail!("duplicate attr: {prev:?}");
        }
    }
    Ok(Feature {
        api: api.context("api is missing")?,
        name: name.context("name is missing")?,
        number: number.context("number is missing")?,
        requires: Vec::new(),
        removes: Vec::new(),
    })
}

fn parse_feature<'a>(
    start_tag: xml_iterator::StartTag<'a>,
    element_iterator: &mut xml_iterator::ElementIterator<'a>,
) -> anyhow::Result<Feature<'a>> {
    let mut feature = parse_feature_attrs(start_tag).context("could not parse feature attrs")?;
    while let Some(element) = element_iterator.next() {
        match element {
            xml_iterator::Element::Text(text) if text.chars().all(|c| c.is_whitespace()) => {}
            xml_iterator::Element::StartTag(start) => match start.name {
                "require" => {
                    let require = parse_interface("require", element_iterator)
                        .context("could not parse feature require")?;
                    feature.requires.push(require);
                }
                "remove" => {
                    let remove = parse_interface("remove", element_iterator)
                        .context("could not parse feature remove")?;
                    feature.removes.push(remove);
                }
                other => bail!("unexpected start: {other}"),
            },
            xml_iterator::Element::EmptyTag(empty) if empty.name == "require" => {}
            xml_iterator::Element::Comment(_) => {}
            xml_iterator::Element::EndTag(end) if end.name == "feature" => break,
            other => bail!("unexpected event: {other:?}"),
        }
    }
    Ok(feature)
}

fn parse_extension_attrs<'a>(
    start_tag: xml_iterator::StartTag<'a>,
) -> anyhow::Result<Extension<'a>> {
    let mut name: Option<&'a str> = None;
    let mut supported: Option<&'a str> = None;
    for attr in start_tag.iter_attrs() {
        let prev = match attr.key {
            "name" => name.replace(attr.value),
            "supported" => supported.replace(attr.value),
            "comment" => None,
            other => bail!("unexpected attr: {other}"),
        };
        if prev.is_some() {
            bail!("duplicate attr: {prev:?}");
        }
    }
    Ok(Extension {
        name: name.context("name is missing")?,
        supported: supported.context("supported is missing")?,
        requires: Vec::new(),
    })
}

fn parse_extension<'a>(
    start_tag: xml_iterator::StartTag<'a>,
    element_iterator: &mut xml_iterator::ElementIterator<'a>,
) -> anyhow::Result<Extension<'a>> {
    let mut extension =
        parse_extension_attrs(start_tag).context("could not parse extension attrs")?;
    while let Some(element) = element_iterator.next() {
        match element {
            xml_iterator::Element::Text(text) if text.chars().all(|c| c.is_whitespace()) => {}
            xml_iterator::Element::StartTag(start) => match start.name {
                "require" => {
                    let require = parse_interface("require", element_iterator)
                        .context("could not parse extension requirs")?;
                    extension.requires.push(require);
                }
                other => bail!("unexpected start: {other}"),
            },
            xml_iterator::Element::EndTag(end) if end.name == "extension" => break,
            other => bail!("unexpected event: {other:?}"),
        }
    }
    Ok(extension)
}

pub fn parse_registry<'a>(input: &'a str) -> anyhow::Result<Registry<'a>> {
    let mut enums: Vec<Enum> = Vec::new();
    let mut commands: Vec<Command> = Vec::new();
    let mut features: Vec<Feature> = Vec::new();
    let mut extensions: Vec<Extension> = Vec::new();

    let mut element_iterator = xml_iterator::ElementIterator::new(input);
    loop {
        let Some(element) = element_iterator.next() else {
            break;
        };
        match element {
            xml_iterator::Element::StartTag(start) => match start.name {
                "enums" => {
                    parse_enum_block_into(start, &mut element_iterator, &mut enums)
                        .context("could not parse enum block")?;
                }
                "command" => {
                    let command =
                        parse_command(&mut element_iterator).context("could not parse command")?;
                    commands.push(command);
                }
                "feature" => {
                    let feature = parse_feature(start, &mut element_iterator)
                        .context("could not parse feature")?;
                    features.push(feature);
                }
                "extension" => {
                    let extension = parse_extension(start, &mut element_iterator)
                        .context("could not parse extension")?;
                    extensions.push(extension);
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(Registry {
        enums,
        commands,
        features,
        extensions,
    })
}

#[derive(Debug)]
struct Version {
    major: u32,
    minor: u32,
}

impl Version {
    fn from_tuple(t: (u32, u32)) -> Self {
        Self {
            major: t.0,
            minor: t.1,
        }
    }
}

impl FromStr for Version {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('.');
        let major: u32 = parts.next().context("missing major")?.parse()?;
        let minor: u32 = parts.next().context("missing minor")?.parse()?;
        assert!(parts.next().is_none());
        Ok(Version { major, minor })
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major && self.minor == other.minor
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(
            self.major
                .cmp(&other.major)
                .then(self.minor.cmp(&other.minor)),
        )
    }
}

#[test]
fn test_version() {
    let a = Version { major: 3, minor: 0 };
    let b = Version { major: 4, minor: 6 };
    assert!(a < b);
}

pub fn filter_registry<'a>(
    mut registry: Registry<'a>,
    api: &str,
    version: (u32, u32),
    extensions: &[&str],
) -> anyhow::Result<Registry<'a>> {
    let version = Version::from_tuple(version);

    let mut wanted_enums: HashSet<&str> = HashSet::new();
    let mut wanted_commands: HashSet<&str> = HashSet::new();

    let mut found_feature = false;
    for feat in registry.features.iter() {
        if feat.api != api {
            continue;
        }

        let feat_version = Version::from_str(&feat.number)?;
        if feat_version > version {
            continue;
        }
        if feat_version == version {
            found_feature = true;
        }

        for require in feat.requires.iter() {
            wanted_enums.extend(require.enums.iter().map(|string| string));
            wanted_commands.extend(require.commands.iter().map(|string| string));
        }
        for remove in feat.removes.iter() {
            for it in remove.enums.iter() {
                wanted_enums.remove(it);
            }
            for it in remove.commands.iter() {
                wanted_commands.remove(it);
            }
        }
    }
    if !found_feature {
        bail!("could not find {api} {version:?}");
    }

    for ext in registry.extensions.iter() {
        if !extensions.contains(&ext.name) {
            continue;
        }
        if !ext.supported.split("|").any(|part| part == api) {
            bail!("{} is not supported on {api} {version:?}", &ext.name);
        }
        for require in ext.requires.iter() {
            wanted_enums.extend(require.enums.iter().map(|string| string));
            wanted_commands.extend(require.commands.iter().map(|string| string));
        }
    }

    registry.enums.retain(|e| wanted_enums.contains(e.name));
    registry
        .commands
        .retain(|c| wanted_commands.contains(c.proto.name));

    Ok(registry)
}

fn emit_enums<W: io::Write>(mut w: W, enums: &[Enum]) -> anyhow::Result<()> {
    for e in enums.iter() {
        assert!(e.name.starts_with("GL_"));
        let name = &e.name[3..];
        write!(w, "pub const {name}: {} = {};\n", e.ty, &e.value)?;
    }
    write!(w, "\n")?;

    Ok(())
}

fn emit_command_type_parts<W: io::Write>(
    mut w: W,
    parts: &[CommandTypePart],
) -> anyhow::Result<()> {
    use CommandTypePart::*;

    match parts.len() {
        0 => unreachable!(),
        1 => match parts[0] {
            Other(other) => match other {
                "void" => {}
                "void *" => write!(w, "*mut std::ffi::c_void")?,
                "const void *" => write!(w, "*const std::ffi::c_void")?,
                "const void **" => write!(w, "*mut *const std::ffi::c_void")?,
                "void **" => write!(w, "*mut *mut std::ffi::c_void")?,
                "const void *const*" => write!(w, "*const *const std::ffi::c_void")?,
                other => unimplemented!("{other:?}"),
            },
            Defined(defined) => {
                write!(w, "{defined}")?;
            }
        },
        2 => match (&parts[0], &parts[1]) {
            (Defined(defined), Other(pointer)) if *pointer == "*" => {
                write!(w, "*mut {defined}")?;
            }
            _ => unimplemented!("{parts:?}"),
        },
        3 => match (&parts[0], &parts[1], &parts[2]) {
            (Other(qualifier), Defined(defined), Other(pointer)) => match (*qualifier, *pointer) {
                ("const", "*") => write!(w, "*const {defined}")?,
                ("const", "*const*") => write!(w, "*const *const {defined}")?,
                ("const", "**") => write!(w, "*mut *const {defined}")?,
                _ => unimplemented!("{parts:?}"),
            },
            _ => unimplemented!("{parts:?}"),
        },
        _ => unimplemented!("{parts:?}"),
    }

    Ok(())
}

#[inline]
fn normalize_command_name(name: &str) -> &str {
    assert!(name.starts_with("gl"));
    &name[2..]
}

#[inline]
fn normalize_command_param_name(name: &str) -> &str {
    match name {
        "type" => "type_",
        "ref" => "ref_",
        ok => ok,
    }
}

fn emit_api_struct<W: io::Write>(mut w: W, commands: &[Command]) -> anyhow::Result<()> {
    write!(w, "pub struct Api {{\n")?;
    for cmd in commands.iter() {
        let name = normalize_command_name(cmd.proto.name);
        write!(w, "    {name}: FnPtr,\n")?;
    }
    write!(w, "}}\n\n")?;

    Ok(())
}

fn emit_api_impl<W: io::Write>(mut w: W, commands: &[Command]) -> anyhow::Result<()> {
    let mut buf: Vec<u8> = Vec::new();

    write!(w, "impl Api {{")?;
    write!(
        w,
        r#"
    pub unsafe fn load_with<F>(mut get_proc_address: F) -> Self
    where
        F: FnMut(*const c_char) -> *mut c_void,
    {{
        Self {{
"#
    )?;
    for cmd in commands.iter() {
        write!(
            w,
            "            {}: FnPtr::new(get_proc_address(c\"{}\".as_ptr())),\n",
            normalize_command_name(cmd.proto.name),
            cmd.proto.name,
        )?;
    }
    write!(w, "        }}\n")?;
    write!(w, "    }}\n")?;

    for cmd in commands.iter() {
        write!(w, "\n    #[inline]\n")?;

        let name = normalize_command_name(cmd.proto.name);
        write!(w, "    pub unsafe fn {name}(\n")?;
        write!(w, "        &self,\n")?;

        if !cmd.params.is_empty() {
            for param in cmd.params.iter() {
                let name = normalize_command_param_name(param.name);
                write!(w, "        {name}: ")?;
                emit_command_type_parts(&mut w, &param.type_parts)?;
                write!(w, ",\n")?;
            }
        }

        write!(w, "    ) ")?;

        emit_command_type_parts(&mut buf, &cmd.proto.return_type_parts)?;
        if !buf.is_empty() {
            write!(w, "-> ")?;
            w.write(&buf)?;
            buf.clear();
        }

        write!(w, "{{\n")?;
        write!(w, "        type Dst = extern \"C\" fn(")?;
        cmd.params
            .iter()
            .enumerate()
            .try_for_each(|(i, param)| -> anyhow::Result<()> {
                emit_command_type_parts(&mut w, &param.type_parts)?;
                if i < cmd.params.len() - 1 {
                    write!(w, ", ")?;
                }
                Ok(())
            })?;
        write!(w, ")")?;
        emit_command_type_parts(&mut buf, &cmd.proto.return_type_parts)?;
        if !buf.is_empty() {
            write!(w, " -> ")?;
            w.write(&buf)?;
            buf.clear();
        }
        write!(w, ";\n")?;
        write!(w, "        unsafe {{ transmute::<_, Dst>(self.{name}.ptr)(")?;
        cmd.params
            .iter()
            .enumerate()
            .try_for_each(|(i, param)| -> anyhow::Result<()> {
                let name = normalize_command_param_name(param.name);
                write!(w, "{name}")?;
                if i < cmd.params.len() - 1 {
                    write!(w, ", ")?;
                }
                Ok(())
            })?;
        write!(w, ") }}\n")?;
        write!(w, "    }}\n")?;
    }
    write!(w, "}}\n\n")?;

    Ok(())
}

pub fn generate_api<W: io::Write>(mut w: W, registry: &Registry) -> anyhow::Result<()> {
    write!(w, "{}\n\n", BOILERPLATE.trim())?;
    emit_enums(&mut w, &registry.enums)?;
    emit_api_struct(&mut w, &registry.commands)?;
    emit_api_impl(&mut w, &registry.commands)?;
    Ok(())
}
