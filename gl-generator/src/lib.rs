use std::collections::HashSet;
use std::io;
use std::str::FromStr;

use anyhow::{Context as _, bail};
use xml_iterator::{Element, ElementIterator, StartTag};

#[derive(Debug)]
pub enum Api {
    Gl,
    Egl,
}

impl Api {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gl => "gl",
            Self::Egl => "egl",
        }
    }

    fn enum_prefix(&self) -> &'static str {
        match self {
            Self::Gl => "GL_",
            Self::Egl => "EGL_",
        }
    }

    fn command_prefix(&self) -> &'static str {
        match self {
            Self::Gl => "gl",
            Self::Egl => "egl",
        }
    }
}

#[derive(Debug)]
pub struct Version(pub u32, pub u32);

impl FromStr for Version {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('.');
        let major: u32 = parts.next().context("missing major")?.parse()?;
        let minor: u32 = parts.next().context("missing minor")?.parse()?;
        assert!(parts.next().is_none());
        Ok(Version(major, minor))
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(&other.0).then(self.1.cmp(&other.1)))
    }
}

#[test]
fn test_version() {
    let a = Version(3, 0);
    let b = Version(4, 6);
    assert!(a < b);
}

// xml spec:
// https://github.com/KhronosGroup/OpenGL-Registry/blob/8e772a3b0c9e8a85ccb6f471b4cdbf94c8bcd71d/xml/readme.pdf

#[derive(Debug)]
pub struct Enum<'a> {
    pub value: &'a str,
    pub name: &'a str,
    pub api: Option<&'a str>,
    pub r#type: Option<&'a str>,
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
pub struct CommandPart<'a> {
    pub type_parts: Vec<CommandTypePart<'a>>,
    pub name: &'a str,
}

#[derive(Debug)]
pub struct Command<'a> {
    pub proto: CommandPart<'a>,
    pub params: Vec<CommandPart<'a>>,
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

fn expect_text<'a>(element_iterator: &mut ElementIterator<'a>) -> anyhow::Result<&'a str> {
    let Some(element) = element_iterator.next() else {
        bail!("unexpected eof");
    };
    let Element::Text(text) = element else {
        bail!("unexpected element (got {element:?}, want text)");
    };
    Ok(&text)
}

fn expect_end_tag<'a>(element_iterator: &mut ElementIterator<'a>) -> anyhow::Result<()> {
    let Some(element) = element_iterator.next() else {
        bail!("unexpected eof");
    };
    let Element::EndTag(_) = element else {
        bail!("unexpected element (got {element:?}, want end)");
    };
    Ok(())
}

fn parse_enum_token_attrs<'a>(
    empty_tag: StartTag<'a>,
    block_type: Option<&'a str>,
) -> anyhow::Result<Enum<'a>> {
    let mut value: Option<&'a str> = None;
    let mut name: Option<&'a str> = None;
    let mut api: Option<&'a str> = None;
    let mut r#type: Option<&'a str> = None;
    let mut group: Option<&'a str> = None;
    let mut alias: Option<&'a str> = None;
    let mut comment: Option<&'a str> = None;
    for attr in empty_tag.iter_attrs() {
        let prev = match attr.key {
            "value" => value.replace(attr.value),
            "name" => name.replace(attr.value),
            "api" => api.replace(attr.value),
            "type" => r#type.replace(attr.value),
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
        r#type: r#type.or(block_type),
        group,
        alias,
        comment,
    })
}

fn parse_enum_block_into<'a>(
    start_tag: StartTag<'a>,
    element_iterator: &mut ElementIterator<'a>,
    enums: &mut Vec<Enum<'a>>,
) -> anyhow::Result<()> {
    let block_type = start_tag
        .iter_attrs()
        .find(|attr| attr.key == "type")
        .map(|attr| attr.value);
    while let Some(element) = element_iterator.next() {
        match element {
            Element::EmptyTag(empty) => match empty.name {
                "enum" => {
                    let token = parse_enum_token_attrs(empty, block_type)
                        .context("could not parse enum token attrs")?;
                    enums.push(token);
                }
                "unused" => {}
                other => bail!("unexpected empty: {:?}", other),
            },
            Element::Text(text) if text.chars().all(|c| c.is_whitespace()) => {}
            Element::EndTag(end) if end.name == "enums" => break,
            Element::Comment(_) => {}
            other => bail!("unexpected element: {other:?}"),
        }
    }
    Ok(())
}

fn parse_command_part<'a>(
    tag_name: &str,
    element_iterator: &mut ElementIterator<'a>,
) -> anyhow::Result<CommandPart<'a>> {
    let mut type_parts: Vec<CommandTypePart<'a>> = Vec::new();
    let mut name: Option<&'a str> = None;
    while let Some(element) = element_iterator.next() {
        match element {
            Element::Text(text) => {
                if !text.chars().all(|c| c.is_whitespace()) {
                    type_parts.push(CommandTypePart::Other(text.trim()));
                }
            }
            Element::StartTag(start) => match start.name {
                "name" => {
                    assert!(name.is_none());
                    name = Some(expect_text(element_iterator)?);
                    expect_end_tag(element_iterator)?;
                }
                "ptype" => {
                    type_parts.push(CommandTypePart::Defined(expect_text(element_iterator)?));
                    expect_end_tag(element_iterator)?;
                }
                other => bail!("unexpected start: {other}"),
            },
            Element::EndTag(end) if end.name == tag_name => break,
            other => bail!("unexpected element: {other:?}"),
        }
    }
    Ok(CommandPart {
        type_parts,
        name: name.context("proto name is missing")?,
    })
}

fn parse_command<'a>(element_iterator: &mut ElementIterator<'a>) -> anyhow::Result<Command<'a>> {
    let mut proto: Option<CommandPart<'a>> = None;
    let mut params: Vec<CommandPart<'a>> = Vec::new();
    while let Some(element) = element_iterator.next() {
        match element {
            Element::StartTag(start) => match start.name {
                "proto" => {
                    assert!(proto.is_none());
                    proto = Some(
                        parse_command_part("proto", element_iterator)
                            .context("could not parse command proto")?,
                    );
                }
                "param" => {
                    params.push(
                        parse_command_part("param", element_iterator)
                            .context("could not parse command param")?,
                    );
                }
                other => bail!("unexpected start: {other}"),
            },
            Element::EndTag(end) => match end.name {
                "command" => break,
                other => bail!("unexpected end: {other}"),
            },
            Element::Text(text) if text.chars().all(|c| c.is_whitespace()) => {}
            Element::EmptyTag(empty) if matches!(empty.name, "glx" | "alias" | "vecequiv") => {}
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
    element_iterator: &mut ElementIterator<'a>,
) -> anyhow::Result<Interface<'a>> {
    let mut enums: Vec<&'a str> = Vec::new();
    let mut commands: Vec<&'a str> = Vec::new();
    while let Some(element) = element_iterator.next() {
        match element {
            Element::Text(text) if text.chars().all(|c| c.is_whitespace()) => {}
            Element::EmptyTag(empty) => match empty.name {
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
            Element::EndTag(end) if end.name == tag_name => break,
            Element::Comment(_) => {}
            other => bail!("unexpected element: {other:?}"),
        }
    }
    Ok(Interface { enums, commands })
}

fn parse_feature_attrs<'a>(start_tag: StartTag<'a>) -> anyhow::Result<Feature<'a>> {
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
    start_tag: StartTag<'a>,
    element_iterator: &mut ElementIterator<'a>,
) -> anyhow::Result<Feature<'a>> {
    let mut feature = parse_feature_attrs(start_tag).context("could not parse feature attrs")?;
    while let Some(element) = element_iterator.next() {
        match element {
            Element::Text(text) if text.chars().all(|c| c.is_whitespace()) => {}
            Element::StartTag(start) => match start.name {
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
            Element::EmptyTag(empty) if empty.name == "require" => {}
            Element::Comment(_) => {}
            Element::EndTag(end) if end.name == "feature" => break,
            other => bail!("unexpected event: {other:?}"),
        }
    }
    Ok(feature)
}

fn parse_extension_attrs<'a>(start_tag: StartTag<'a>) -> anyhow::Result<Extension<'a>> {
    let mut name: Option<&'a str> = None;
    let mut supported: Option<&'a str> = None;
    for attr in start_tag.iter_attrs() {
        let prev = match attr.key {
            "name" => name.replace(attr.value),
            "supported" => supported.replace(attr.value),
            "comment" | "protect" => None,
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
    start_tag: StartTag<'a>,
    element_iterator: &mut ElementIterator<'a>,
) -> anyhow::Result<Extension<'a>> {
    let mut extension =
        parse_extension_attrs(start_tag).context("could not parse extension attrs")?;
    while let Some(element) = element_iterator.next() {
        match element {
            Element::Text(text) if text.chars().all(|c| c.is_whitespace()) => {}
            Element::StartTag(start) => match start.name {
                "require" => {
                    let require = parse_interface("require", element_iterator)
                        .context("could not parse extension requirs")?;
                    extension.requires.push(require);
                }
                other => bail!("unexpected start: {other}"),
            },
            Element::EndTag(end) if end.name == "extension" => break,
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

    let mut element_iterator = ElementIterator::new(input);
    loop {
        let Some(element) = element_iterator.next() else {
            break;
        };
        match element {
            Element::StartTag(start) => match start.name {
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

pub fn filter_registry<'a>(
    mut registry: Registry<'a>,
    api: &Api,
    version: &Version,
    extensions: &[&str],
) -> anyhow::Result<Registry<'a>> {
    let version = Version(version.0, version.1);

    let mut wanted_enums: HashSet<&str> = HashSet::new();
    let mut wanted_commands: HashSet<&str> = HashSet::new();

    let mut found_feature = false;
    for feat in registry.features.iter() {
        if feat.api != api.as_str() {
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
        bail!("could not find {api:?} {version:?}");
    }

    for ext in registry.extensions.iter() {
        if !extensions.contains(&ext.name) {
            continue;
        }
        if !ext.supported.split("|").any(|part| part == api.as_str()) {
            bail!("{} is not supported on {api:?} {version:?}", &ext.name);
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

const GL_TYPES: &str = "pub type GLbitfield = std::ffi::c_uint;
pub type GLboolean = std::ffi::c_uchar;
pub type GLbyte = std::ffi::c_char;
pub type GLchar = std::ffi::c_char;
pub type GLdouble = std::ffi::c_double;
pub type GLenum = std::ffi::c_uint;
pub type GLfloat = std::ffi::c_float;
pub type GLint = std::ffi::c_int;
pub type GLint64 = i64;
pub type GLintptr = isize;
pub type GLshort = std::ffi::c_short;
pub type GLsizei = std::ffi::c_int;
pub type GLsizeiptr = isize;
pub type GLsync = *mut std::ffi::c_void;
pub type GLubyte = std::ffi::c_uchar;
pub type GLuint = std::ffi::c_uint;
pub type GLuint64 = u64;
pub type GLushort = std::ffi::c_ushort;

pub type GLDEBUGPROC = Option<extern \"C\" fn(
    source: GLenum,
    r#type: GLenum,
    id: GLuint,
    severity: GLenum,
    length: GLsizei,
    message: *const GLchar,
    userParam: *mut std::ffi::c_void,
)>;
";

const EGL_TYPES: &str = "pub type khronos_int32_t = i32;
pub type khronos_utime_nanoseconds_t = u64;

// https://registry.khronos.org/EGL/api/EGL/eglplatform.h

pub type EGLNativeDisplayType = *mut std::ffi::c_void;
pub type EGLNativePixmapType = *mut std::ffi::c_void;
pub type EGLNativeWindowType = *mut std::ffi::c_void;

pub type EGLint = khronos_int32_t;

// https://registry.khronos.org/EGL/api/EGL/egl.h

// 1.0

pub type EGLBoolean = std::ffi::c_uint;
pub type EGLDisplay = *mut std::ffi::c_void;

pub type EGLConfig = *mut std::ffi::c_void;
pub type EGLSurface = *mut std::ffi::c_void;
pub type EGLContext = *mut std::ffi::c_void;
pub type __eglMustCastToProperFunctionPointerType = unsafe extern \"C\" fn();

// 1.2

pub type EGLenum = std::ffi::c_uint;
pub type EGLClientBuffer = *mut std::ffi::c_void;

// 1.5

pub type EGLSync = *mut std::ffi::c_void;
pub type EGLAttrib = isize;
pub type EGLTime = khronos_utime_nanoseconds_t;
pub type EGLImage = *mut std::ffi::c_void;
";

pub fn emit_types<W: io::Write>(w: &mut W, api: &Api) -> anyhow::Result<()> {
    let types = match api {
        Api::Gl => GL_TYPES,
        Api::Egl => EGL_TYPES,
    };
    w.write(types.as_bytes())?;
    Ok(())
}

fn normalize_gl_enum_type<'a>(
    r#type: Option<&'a str>,
    normalized_name: &'a str,
) -> anyhow::Result<&'a str> {
    match r#type {
        Some("u") => Ok("GLuint"),
        Some("ull") => Ok("GLuint64"),
        Some("bitmask") => Ok("GLbitfield"),
        None if normalized_name == "TRUE" || normalized_name == "FALSE" => Ok("GLboolean"),
        None => Ok("GLenum"),
        other => bail!("unknown gl enum type {other:?}"),
    }
}

fn normalize_egl_enum_type<'a>(
    r#type: Option<&'a str>,
    normalized_name: &'a str,
    value: &'a str,
) -> anyhow::Result<&'a str> {
    match r#type {
        Some("u") => Ok("EGLuint"),
        Some("ull") => Ok("u64"),
        Some("bitmask") => Ok("EGLint"),
        None if value.starts_with("-") => Ok("EGLint"),
        None if normalized_name == "TRUE" || normalized_name == "FALSE" => Ok("EGLBoolean"),
        None if value.starts_with("EGL_CAST") => {
            let comma_position = value.find(",").context("egl cast comma")?;
            Ok(&value[9..comma_position])
        }
        None => Ok("EGLenum"),
        other => bail!("unknown gl enum type {other:?}"),
    }
}

fn emit_egl_enum_value<W: io::Write>(w: &mut W, value: &str) -> anyhow::Result<()> {
    if !value.starts_with("EGL_CAST") {
        w.write(value.as_bytes())?;
        return Ok(());
    }

    let comma_position = value.find(",").context("egl cast comma")?;
    let inner_value = &value[comma_position + 1..value.len() - 1];
    let target_type = &value[9..comma_position];
    write!(w, "{inner_value} as {target_type}")?;

    Ok(())
}

pub fn emit_enums<W: io::Write>(w: &mut W, registry: &Registry, api: &Api) -> anyhow::Result<()> {
    let prefix = api.enum_prefix();
    for e in registry.enums.iter() {
        assert!(e.name.starts_with(prefix));
        let name = &e.name[prefix.len()..];
        let r#type = match api {
            Api::Gl => normalize_gl_enum_type(e.r#type, name),
            Api::Egl => normalize_egl_enum_type(e.r#type, name, e.value),
        }?;
        write!(w, "pub const {name}: {type} = ")?;
        match api {
            Api::Gl => write!(w, "{}", &e.value)?,
            Api::Egl => emit_egl_enum_value(w, &e.value)?,
        };
        write!(w, ";\n")?;
    }
    write!(w, "\n")?;
    Ok(())
}

fn emit_command_type_parts<W: io::Write>(
    w: &mut W,
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
                "const char *" => write!(w, "*const std::ffi::c_char")?,
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
fn normalize_command_name<'a>(name: &'a str, api: &Api) -> &'a str {
    let prefix = api.command_prefix();
    assert!(name.starts_with(prefix));
    &name[prefix.len()..]
}

#[inline]
fn normalize_command_param_name(name: &str) -> &str {
    match name {
        "type" => "r#type",
        "ref" => "r#ref",
        ok => ok,
    }
}

fn emit_api_struct<W: io::Write>(w: &mut W, registry: &Registry, api: &Api) -> anyhow::Result<()> {
    write!(w, "pub struct Api {{\n")?;
    for cmd in registry.commands.iter() {
        let name = normalize_command_name(cmd.proto.name, api);
        write!(w, "    {name}: FnPtr,\n")?;
    }
    write!(w, "}}\n")?;
    Ok(())
}

fn emit_api_impl<W: io::Write>(w: &mut W, registry: &Registry, api: &Api) -> anyhow::Result<()> {
    let mut command_type_parts_buf: Vec<u8> = Vec::new();

    write!(
        w,
        "#[cold]
#[inline(never)]
fn null_fn_ptr_panic() -> ! {{
    panic!(\"function was not loaded\")
}}

struct FnPtr {{
    ptr: *const std::ffi::c_void,
}}

impl FnPtr {{
    fn new(ptr: *const std::ffi::c_void) -> FnPtr {{
        if ptr.is_null() {{
            FnPtr {{ ptr: null_fn_ptr_panic as *const std::ffi::c_void }}
        }} else {{
            FnPtr {{ ptr }}
        }}
    }}
}}

impl Api {{
    pub unsafe fn load_with<F>(mut get_proc_address: F) -> Self
    where
        F: FnMut(*const std::ffi::c_char) -> *mut std::ffi::c_void,
    {{
        Self {{
"
    )?;

    for cmd in registry.commands.iter() {
        write!(
            w,
            "            {}: FnPtr::new(get_proc_address(c\"{}\".as_ptr())),\n",
            normalize_command_name(cmd.proto.name, api),
            cmd.proto.name,
        )?;
    }
    write!(w, "        }}\n")?;
    write!(w, "    }}\n")?;

    for cmd in registry.commands.iter() {
        // signature

        write!(w, "\n    #[inline]\n")?;
        let name = normalize_command_name(cmd.proto.name, api);
        write!(w, "    pub unsafe fn {name}(&self")?;
        for param in cmd.params.iter() {
            let name = normalize_command_param_name(param.name);
            write!(w, ", {name}: ")?;
            emit_command_type_parts(w, &param.type_parts)?;
        }
        write!(w, ") ")?;

        emit_command_type_parts(&mut command_type_parts_buf, &cmd.proto.type_parts)?;
        if !command_type_parts_buf.is_empty() {
            write!(w, "-> ")?;
            w.write(&command_type_parts_buf)?;
            write!(w, " ")?;
            command_type_parts_buf.clear();
        }

        // body

        // type
        write!(w, "{{\n")?;
        write!(w, "        type Dst = extern \"C\" fn(")?;
        cmd.params
            .iter()
            .enumerate()
            .try_for_each(|(i, param)| -> anyhow::Result<()> {
                emit_command_type_parts(w, &param.type_parts)?;
                if i < cmd.params.len() - 1 {
                    write!(w, ", ")?;
                }
                Ok(())
            })?;
        write!(w, ")")?;
        emit_command_type_parts(&mut command_type_parts_buf, &cmd.proto.type_parts)?;
        if !command_type_parts_buf.is_empty() {
            write!(w, " -> ")?;
            w.write(&command_type_parts_buf)?;
            command_type_parts_buf.clear();
        }
        write!(w, ";\n")?;
        // call
        write!(
            w,
            "        unsafe {{ std::mem::transmute::<_, Dst>(self.{name}.ptr)("
        )?;
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

    write!(w, "}}\n")?;

    Ok(())
}

pub fn emit_api<W: io::Write>(w: &mut W, registry: &Registry, api: &Api) -> anyhow::Result<()> {
    emit_api_struct(w, &registry, api)?;
    emit_api_impl(w, &registry, api)?;
    Ok(())
}
