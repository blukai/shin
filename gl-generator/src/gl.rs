use std::borrow::Cow;
use std::collections::HashSet;
use std::io;
use std::str::FromStr;

use anyhow::{Context as _, bail};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

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
pub struct Enum {
    pub value: String,
    pub name: String,
    pub api: Option<String>,
    pub ty: String,
    pub group: Option<String>,
    pub alias: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug)]
pub enum CommandTypePart {
    Defined(String),
    Other(String),
}

#[derive(Debug)]
pub struct CommandProto {
    pub return_type_parts: Vec<CommandTypePart>,
    pub name: String,
}

#[derive(Debug)]
pub struct CommandParam {
    pub type_parts: Vec<CommandTypePart>,
    pub name: String,
}

#[derive(Debug)]
pub struct Command {
    pub proto: CommandProto,
    pub params: Vec<CommandParam>,
}

#[derive(Debug)]
pub struct Interface {
    pub enums: Vec<String>,
    pub commands: Vec<String>,
}

#[derive(Debug)]
pub struct Feature {
    pub api: String,
    pub name: String,
    pub number: String,
    pub requires: Vec<Interface>,
    pub removes: Vec<Interface>,
}

#[derive(Debug)]
pub struct Extension {
    pub name: String,
    pub supported: String,
    pub requires: Vec<Interface>,
}

#[derive(Debug)]
pub struct Registry {
    pub enums: Vec<Enum>,
    pub commands: Vec<Command>,
    pub features: Vec<Feature>,
    pub extensions: Vec<Extension>,
}

fn bytes_to_string(bytes: &[u8]) -> String {
    unsafe { String::from_utf8_unchecked(bytes.to_vec()) }
}

fn text_or_err(event: Event) -> anyhow::Result<String> {
    let Event::Text(text) = event else {
        bail!("unexpected event (got {event:?}, want text)");
    };
    Ok(bytes_to_string(&text))
}

fn end_or_err(event: Event) -> anyhow::Result<()> {
    let Event::End(_) = event else {
        bail!("unexpected event (got {event:?}, want end)");
    };
    Ok(())
}

fn get_enum_type(bytes_start: BytesStart) -> anyhow::Result<Cow<'static, str>> {
    let ty = bytes_start.try_get_attribute(b"type")?;
    match ty.as_ref().map(|attr| attr.value.as_ref()) {
        Some(b"bitmask") => Ok(Cow::from("GLbitfield")),
        None => Ok(Cow::from("GLenum")),
        other => bail!("unknown enum block ty: {other:?}"),
    }
}

fn parse_enum_token_attrs(
    bytes_start: BytesStart,
    block_type: &Cow<'static, str>,
) -> anyhow::Result<Enum> {
    let mut value: Option<String> = None;
    let mut name: Option<String> = None;
    let mut api: Option<String> = None;
    let mut ty: Option<String> = None;
    let mut group: Option<String> = None;
    let mut alias: Option<String> = None;
    let mut comment: Option<String> = None;
    for attr in bytes_start.attributes() {
        let attr = attr?;
        let prev = match attr.key.as_ref() {
            b"value" => value.replace(bytes_to_string(&attr.value)),
            b"name" => name.replace(bytes_to_string(&attr.value)),
            b"api" => api.replace(bytes_to_string(&attr.value)),
            b"type" => ty.replace(bytes_to_string(&attr.value)),
            b"group" => group.replace(bytes_to_string(&attr.value)),
            b"alias" => alias.replace(bytes_to_string(&attr.value)),
            b"comment" => comment.replace(bytes_to_string(&attr.value)),
            other => bail!("unexpected attr: {:?}", bytes_to_string(other)),
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
            .map(|ty| match ty.as_str() {
                "u" => "GLuint".to_string(),
                "ull" => "GLuint64".to_string(),
                _ => ty,
            })
            .unwrap_or_else(|| block_type.to_string()),
        group,
        alias,
        comment,
    })
}

fn parse_enum_block<R>(
    bytes_start: BytesStart,
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
) -> anyhow::Result<Vec<Enum>>
where
    R: io::BufRead,
{
    let block_type = get_enum_type(bytes_start)?;
    let mut enums: Vec<Enum> = Vec::new();
    loop {
        let event = reader.read_event_into(buf)?;
        match event {
            Event::Empty(empty) => match empty.name().as_ref() {
                b"enum" => {
                    let token = parse_enum_token_attrs(empty, &block_type)
                        .context("could not parse enum token attrs")?;
                    enums.push(token);
                }
                b"unused" => {}
                other => bail!("unexpected empty: {:?}", bytes_to_string(other)),
            },
            Event::End(end) if end.name().as_ref().eq(b"enums") => break,
            Event::Text(text) if text.iter().all(|x| (*x as char).is_whitespace()) => {}
            Event::Comment(_) => {}
            other => bail!("unexpected event: {other:?}"),
        }
    }
    Ok(enums)
}

fn parse_command_proto<R>(reader: &mut Reader<R>, buf: &mut Vec<u8>) -> anyhow::Result<CommandProto>
where
    R: io::BufRead,
{
    let mut return_ty_parts: Vec<CommandTypePart> = Vec::new();
    let mut name: Option<String> = None;
    loop {
        let event = reader.read_event_into(buf)?;
        match event {
            Event::Text(text) => {
                if !text.iter().all(|x| x.is_ascii_whitespace()) {
                    return_ty_parts
                        .push(CommandTypePart::Other(bytes_to_string(text.trim_ascii())));
                }
            }
            Event::Start(start) => match start.name().as_ref() {
                b"ptype" => {
                    return_ty_parts.push(CommandTypePart::Defined(text_or_err(
                        reader.read_event_into(buf)?,
                    )?));
                    end_or_err(reader.read_event_into(buf)?)?;
                }
                b"name" => {
                    assert!(name.is_none());
                    name = Some(text_or_err(reader.read_event_into(buf)?)?);
                    end_or_err(reader.read_event_into(buf)?)?;
                }
                other => bail!("unexpected start: {:?}", bytes_to_string(other)),
            },
            Event::End(end) if end.name().as_ref().eq(b"proto") => break,
            other => bail!("unexpected event: {other:?}"),
        }
    }
    Ok(CommandProto {
        return_type_parts: return_ty_parts,
        name: name.context("proto name is missing")?,
    })
}

fn parse_command_param<R>(reader: &mut Reader<R>, buf: &mut Vec<u8>) -> anyhow::Result<CommandParam>
where
    R: io::BufRead,
{
    let mut ty_parts: Vec<CommandTypePart> = Vec::new();
    let mut name: Option<String> = None;
    loop {
        let event = reader.read_event_into(buf)?;
        match event {
            Event::Text(text) => {
                if !text.iter().all(|x| x.is_ascii_whitespace()) {
                    ty_parts.push(CommandTypePart::Other(bytes_to_string(text.trim_ascii())));
                }
            }
            Event::Start(start) => match start.name().as_ref() {
                b"ptype" => {
                    ty_parts.push(CommandTypePart::Defined(text_or_err(
                        reader.read_event_into(buf)?,
                    )?));
                    end_or_err(reader.read_event_into(buf)?)?;
                }
                b"name" => {
                    assert!(name.is_none());
                    name = Some(text_or_err(reader.read_event_into(buf)?)?);
                    end_or_err(reader.read_event_into(buf)?)?;
                }
                other => bail!("unexpected start: {:?}", bytes_to_string(other)),
            },
            Event::End(end) if end.name().as_ref().eq(b"param") => {
                return Ok(CommandParam {
                    type_parts: ty_parts,
                    name: name.take().context("param name is missing")?,
                });
            }
            other => bail!("unexpected event: {other:?}"),
        }
    }
}

fn parse_command<R>(reader: &mut Reader<R>, buf: &mut Vec<u8>) -> anyhow::Result<Command>
where
    R: io::BufRead,
{
    let mut proto: Option<CommandProto> = None;
    let mut params: Vec<CommandParam> = Vec::new();
    loop {
        let event = reader.read_event_into(buf)?;
        match event {
            Event::Start(start) => match start.name().as_ref() {
                b"proto" => {
                    assert!(proto.is_none());
                    proto = Some(
                        parse_command_proto(reader, buf)
                            .context("could not parse command proto")?,
                    );
                }
                b"param" => {
                    params.push(
                        parse_command_param(reader, buf)
                            .context("could not parse command param")?,
                    );
                }
                other => bail!("unexpected start: {:?}", bytes_to_string(other)),
            },
            Event::End(end) => match end.name().as_ref() {
                b"command" => break,
                other => bail!("unexpected end: {:?}", bytes_to_string(other)),
            },
            Event::Text(text) if text.iter().all(|x| x.is_ascii_whitespace()) => {}
            Event::Comment(_) => {}
            Event::Empty(empty)
                if matches!(empty.name().as_ref(), b"glx" | b"alias" | b"vecequiv") => {}
            other => bail!("unexpected event: {other:?}"),
        }
    }
    Ok(Command {
        proto: proto.context("proto is missing")?,
        params,
    })
}

fn parse_interface<R>(
    tag: &[u8],
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
) -> anyhow::Result<Interface>
where
    R: io::BufRead,
{
    let mut enums: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    loop {
        let event = reader.read_event_into(buf)?;
        match event {
            Event::End(end) if end.name().as_ref().eq(tag) => break,
            Event::Text(text) if text.iter().all(|x| (*x as char).is_whitespace()) => {}
            Event::Empty(empty) => match empty.name().as_ref() {
                b"type" => {}
                b"enum" => {
                    let name = empty
                        .try_get_attribute(b"name")?
                        .context("name is missing")?;
                    enums.push(bytes_to_string(&name.value))
                }
                b"command" => {
                    let name = empty
                        .try_get_attribute(b"name")?
                        .context("name is missing")?;
                    commands.push(bytes_to_string(&name.value))
                }
                other => bail!("unexpected empty: {:?}", bytes_to_string(other)),
            },
            Event::Comment(_) => {}
            other => bail!("unexpected event: {other:?}"),
        }
    }
    Ok(Interface { enums, commands })
}

fn parse_feature_attrs(bytes_start: BytesStart) -> anyhow::Result<Feature> {
    let mut api: Option<String> = None;
    let mut name: Option<String> = None;
    let mut number: Option<String> = None;
    for attr in bytes_start.attributes() {
        let attr = attr?;
        let prev = match attr.key.as_ref() {
            b"api" => api.replace(bytes_to_string(&attr.value)),
            b"name" => name.replace(bytes_to_string(&attr.value)),
            b"number" => number.replace(bytes_to_string(&attr.value)),
            other => bail!("unexpected attr: {:?}", bytes_to_string(other)),
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

fn parse_feature<R>(
    bytes_start: BytesStart,
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
) -> anyhow::Result<Feature>
where
    R: io::BufRead,
{
    let mut feature = parse_feature_attrs(bytes_start).context("could not parse feature attrs")?;
    loop {
        let event = reader.read_event_into(buf)?;
        match event {
            Event::End(end) if end.name().as_ref().eq(b"feature") => break,
            Event::Text(text) if text.iter().all(|x| (*x as char).is_whitespace()) => {}
            Event::Start(start) => match start.name().as_ref() {
                b"require" => {
                    let require = parse_interface(b"require", reader, buf)
                        .context("could not parse feature requires")?;
                    feature.requires.push(require);
                }
                b"remove" => {
                    let remove = parse_interface(b"remove", reader, buf)
                        .context("could not parse feature removes")?;
                    feature.removes.push(remove);
                }
                other => bail!("unexpected start: {:?}", bytes_to_string(other)),
            },
            Event::Comment(_) => {}
            Event::Empty(_) => {}
            other => bail!("unexpected event: {other:?}"),
        }
    }
    Ok(feature)
}

fn parse_extension_attrs(bytes_start: BytesStart) -> anyhow::Result<Extension> {
    let mut name: Option<String> = None;
    let mut supported: Option<String> = None;
    for attr in bytes_start.attributes() {
        let attr = attr?;
        let prev = match attr.key.as_ref() {
            b"name" => name.replace(bytes_to_string(&attr.value)),
            b"supported" => supported.replace(bytes_to_string(&attr.value)),
            b"comment" => None,
            other => bail!("unexpected attr: {:?}", bytes_to_string(other)),
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

fn parse_extension<R>(
    bytes_start: BytesStart,
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
) -> anyhow::Result<Extension>
where
    R: io::BufRead,
{
    let mut extension =
        parse_extension_attrs(bytes_start).context("could not parse extension attrs")?;
    loop {
        let event = reader.read_event_into(buf)?;
        match event {
            Event::End(end) if end.name().as_ref().eq(b"extension") => break,
            Event::Text(text) if text.iter().all(|x| (*x as char).is_whitespace()) => {}
            Event::Start(start) => match start.name().as_ref() {
                b"require" => {
                    let require = parse_interface(b"require", reader, buf)
                        .context("could not parse extension requires")?;
                    extension.requires.push(require);
                }
                other => bail!("unexpected start: {:?}", bytes_to_string(other)),
            },
            Event::Comment(_) => {}
            Event::Empty(_) => {}
            other => bail!("unexpected event: {other:?}"),
        }
    }
    Ok(extension)
}

pub fn parse_registry<R>(reader: R) -> anyhow::Result<Registry>
where
    R: io::BufRead,
{
    let mut reader = Reader::from_reader(reader);

    let mut buf = Vec::new();
    let mut buffuckyou = Vec::new();

    let mut enums: Vec<Enum> = Vec::new();
    let mut commands: Vec<Command> = Vec::new();
    let mut features: Vec<Feature> = Vec::new();
    let mut extensions: Vec<Extension> = Vec::new();

    loop {
        let event = reader.read_event_into(&mut buf)?;
        match event {
            Event::Eof => break,
            Event::Start(start) => match start.name().as_ref() {
                b"enums" => {
                    let e = parse_enum_block(start, &mut reader, &mut buffuckyou)
                        .context("could not parse enum block")?;
                    enums.extend(e);
                }
                b"command" => {
                    let command = parse_command(&mut reader, &mut buffuckyou)
                        .context("could not parse command")?;
                    commands.push(command);
                }
                b"feature" => {
                    let feature = parse_feature(start, &mut reader, &mut buffuckyou)
                        .context("could not parse feature")?;
                    features.push(feature);
                }
                b"extension" => {
                    let extension = parse_extension(start, &mut reader, &mut buffuckyou)
                        .context("could not parse extension")?;
                    extensions.push(extension);
                }
                _ => {}
            },
            _ => {}
        }
        buf.clear();
        buffuckyou.clear();
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

pub fn filter_registry(
    mut registry: Registry,
    api: &str,
    version: (u32, u32),
    extensions: &[&str],
) -> anyhow::Result<Registry> {
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
            wanted_enums.extend(require.enums.iter().map(|string| string.as_str()));
            wanted_commands.extend(require.commands.iter().map(|string| string.as_str()));
        }
        for remove in feat.removes.iter() {
            for e in remove.enums.iter() {
                wanted_enums.remove(e.as_str());
            }
            for e in remove.commands.iter() {
                wanted_commands.remove(e.as_str());
            }
        }
    }
    if !found_feature {
        bail!("could not find {api} {version:?}");
    }

    for ext in registry.extensions.iter() {
        if !extensions.contains(&ext.name.as_str()) {
            continue;
        }
        if !ext.supported.split("|").any(|part| part == api) {
            bail!("{} is not supported on {api} {version:?}", &ext.name);
        }
        for require in ext.requires.iter() {
            wanted_enums.extend(require.enums.iter().map(|string| string.as_str()));
            wanted_commands.extend(require.commands.iter().map(|string| string.as_str()));
        }
    }

    registry
        .enums
        .retain(|e| wanted_enums.contains(e.name.as_str()));
    registry
        .commands
        .retain(|c| wanted_commands.contains(c.proto.name.as_str()));

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
        1 => match &parts[0] {
            Other(other) => match other.as_str() {
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
            (Defined(defined), Other(pointer)) if pointer == "*" => {
                write!(w, "*mut {defined}")?;
            }
            _ => unimplemented!("{parts:?}"),
        },
        3 => match (&parts[0], &parts[1], &parts[2]) {
            (Other(qualifier), Defined(defined), Other(pointer)) => {
                match (qualifier.as_str(), defined.as_str(), pointer.as_str()) {
                    ("const", _, "*") => write!(w, "*const {defined}")?,
                    ("const", _, "*const*") => write!(w, "*const *const {defined}")?,
                    ("const", _, "**") => write!(w, "*mut *const {defined}")?,
                    _ => unimplemented!("{parts:?}"),
                }
            }
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
        let name = normalize_command_name(cmd.proto.name.as_str());
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
            normalize_command_name(cmd.proto.name.as_str()),
            cmd.proto.name.as_str(),
        )?;
    }
    write!(w, "        }}\n")?;
    write!(w, "    }}\n")?;

    for cmd in commands.iter() {
        write!(w, "\n    #[inline]\n")?;

        let name = normalize_command_name(cmd.proto.name.as_str());
        write!(w, "    pub unsafe fn {name}(\n")?;
        write!(w, "        &self,\n")?;

        if !cmd.params.is_empty() {
            for param in cmd.params.iter() {
                let name = normalize_command_param_name(param.name.as_str());
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
                let name = normalize_command_param_name(param.name.as_str());
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
