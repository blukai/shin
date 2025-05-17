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
