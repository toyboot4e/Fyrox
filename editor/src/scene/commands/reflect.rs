use std::{any, marker::PhantomData};

use fyrox::{
    core::{
        pool::Handle,
        reflect::{GetField, Reflect},
    },
    scene::node::{Node, NodeTrait},
    utils::log::Log,
};

use crate::scene::commands::{Command, SceneContext};

/// A [`SceneCommand`](crate::SceneCommand) that sets target [`Node`] s data using reflection
#[derive(Debug)]
pub struct ReflectCommand<N> {
    handle: Handle<Node>,
    path: String,
    action: ReflectAction,
    _ty: PhantomData<N>,
}

#[derive(Debug)]
pub enum ReflectAction {
    Swap(Box<dyn Reflect + Send>),
    InsertDefault,
    Remove(usize),
}

impl ReflectAction {
    pub fn execute(&mut self, target: &mut dyn Reflect) {
        match self {
            ReflectAction::Swap(v) => todo!(),
            // we don't know if the target type implements `Default`
            ReflectAction::InsertDefault => todo!(),
            ReflectAction::Remove(i) => todo!(),
        }
    }

    pub fn revert(&mut self, target: &mut dyn Reflect) {
        match self {
            ReflectAction::Swap(v) => todo!(),
            ReflectAction::InsertDefault => todo!(),
            ReflectAction::Remove(i) => todo!(),
        }
    }
}

impl<N: NodeTrait> ReflectCommand<N> {
    pub fn new(handle: Handle<Node>, path: String, action: ReflectAction) -> Self {
        Self {
            handle,
            path,
            action,
            _ty: PhantomData,
        }
    }
}

impl<N: NodeTrait> Command for ReflectCommand<N> {
    fn name(&mut self, _context: &SceneContext) -> String {
        // TODO: use type name in Title Case
        self.path.clone()
    }

    fn execute(&mut self, context: &mut SceneContext) {
        let node = &mut context.scene.graph[self.handle];

        let target = match self::fetch_target::<N>(node, &self.path) {
            Some(x) => x,
            None => return,
        };

        self.action.revert(target);
    }

    fn revert(&mut self, context: &mut SceneContext) {
        let node = &mut context.scene.graph[self.handle];

        let target = match self::fetch_target::<N>(node, &self.path) {
            Some(x) => x,
            None => return,
        };

        self.action.revert(target);
    }
}

fn fetch_target<'a, N: NodeTrait>(node: &'a mut Node, path: &str) -> Option<&'a mut dyn Reflect> {
    let node = match node.cast_mut::<N>() {
        Some(x) => x,
        None => {
            Log::err(format!(
                "Failed to cast `ReflectCommand` node of type `{}`",
                any::type_name::<N>(),
            ));
            return None;
        }
    };

    let target = match node.field_mut(path) {
        Some(x) => x,
        None => {
            Log::err(format!(
                "Failed to cast `ReflectCommand` value for node of type `{}`",
                any::type_name::<N>(),
            ));
            return None;
        }
    };

    Some(target)
}
