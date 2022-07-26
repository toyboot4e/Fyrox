use crate::{scene::commands::reflect::ReflectCommand, SceneCommand};
use fyrox::{
    core::pool::Handle,
    gui::inspector::{FieldKind, PropertyChanged},
    scene::node::{Node, NodeTrait},
    utils::log::Log,
};
use std::any;

pub fn handle_reflect_swap<T: NodeTrait + Clone>(
    args: &PropertyChanged,
    handle: Handle<Node>,
) -> Option<SceneCommand> {
    todo!()
}
