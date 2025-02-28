#![warn(missing_docs)]

//! Script is used to add custom logic to scene nodes. See [ScriptTrait] for more info.

use crate::{
    core::{
        pool::Handle,
        reflect::{FieldInfo, Reflect, ReflectArray, ReflectList},
        uuid::Uuid,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::{resource_manager::ResourceManager, ScriptMessageDispatcher},
    event::Event,
    plugin::Plugin,
    scene::{node::Node, Scene},
    utils::{component::ComponentProvider, log::Log},
};
use std::{
    any::{Any, TypeId},
    fmt::{Debug, Formatter},
    ops::{Deref, DerefMut},
    sync::mpsc::Sender,
};

pub mod constructor;

/// A script message's payload.
pub trait ScriptMessagePayload: Any + Send {
    /// Returns `self` as `&dyn Any`
    fn as_any_ref(&self) -> &dyn Any;

    /// Returns `self` as `&dyn Any`
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl dyn ScriptMessagePayload {
    /// Tries to cast the payload to a particular type.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.as_any_ref().downcast_ref::<T>()
    }

    /// Tries to cast the payload to a particular type.
    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut::<T>()
    }
}

impl<T> ScriptMessagePayload for T
where
    T: 'static + Send,
{
    fn as_any_ref(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Defines how a script message will be delivered for each node in a hierarchy.
pub enum RoutingStrategy {
    /// An message will be passed to the specified root node and then to every node up in the hierarchy.
    Up,
    /// An message will be passed to every node down the tree in the hierarchy.
    Down,
}

/// A script message of a particular kind.
pub struct ScriptMessage {
    /// Actual message payload.
    pub payload: Box<dyn ScriptMessagePayload>,
    /// Actual script message kind.
    pub kind: ScriptMessageKind,
}

/// An message for a node with a script.
pub enum ScriptMessageKind {
    /// An message for a specific scene node. It will be delivered only if the node is subscribed to receive
    /// messages of a particular type.
    Targeted(Handle<Node>),

    /// An message for a hierarchy of nodes.
    Hierarchical {
        /// Starting node in a scene graph. Message will be delivered to each node in hierarchy in the order
        /// defined by `routing` if the node is subscribed to receive messages of a particular type.
        root: Handle<Node>,

        /// [Routing strategy](RoutingStrategy) for the message.
        routing: RoutingStrategy,
    },

    /// An message that will be delivered for **every** scene node that is subscribed to receive messages
    /// of a particular type.
    Global,
}

/// A script message sender.
#[derive(Clone)]
pub struct ScriptMessageSender {
    pub(crate) sender: Sender<ScriptMessage>,
}

impl Debug for ScriptMessageSender {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ScriptMessageSender")
    }
}

impl ScriptMessageSender {
    /// Send a generic script message.
    pub fn send(&self, message: ScriptMessage) {
        if self.sender.send(message).is_err() {
            Log::err("Failed to send script message, it means the scene is already deleted!");
        }
    }

    /// Sends a targeted script message with the given payload.
    pub fn send_to_target<T>(&self, target: Handle<Node>, payload: T)
    where
        T: 'static + Send,
    {
        self.send(ScriptMessage {
            payload: Box::new(payload),
            kind: ScriptMessageKind::Targeted(target),
        })
    }

    /// Sends a global script message with the given payload.
    pub fn send_global<T>(&self, payload: T)
    where
        T: 'static + Send,
    {
        self.send(ScriptMessage {
            payload: Box::new(payload),
            kind: ScriptMessageKind::Global,
        })
    }

    /// Sends a hierarchical script message with the given payload.
    pub fn send_hierarchical<T>(&self, root: Handle<Node>, routing: RoutingStrategy, payload: T)
    where
        T: 'static + Send,
    {
        self.send(ScriptMessage {
            payload: Box::new(payload),
            kind: ScriptMessageKind::Hierarchical { root, routing },
        })
    }
}

/// Base script trait is used to automatically implement some trait to reduce amount of boilerplate code.
pub trait BaseScript: Visit + Reflect + Send + Debug + 'static {
    /// Creates exact copy of the script.
    fn clone_box(&self) -> Box<dyn ScriptTrait>;

    /// Casts self as `Any`
    fn as_any_ref(&self) -> &dyn Any;

    /// Casts self as `Any`
    fn as_any_ref_mut(&mut self) -> &mut dyn Any;
}

impl<T> BaseScript for T
where
    T: Clone + ScriptTrait + Any,
{
    fn clone_box(&self) -> Box<dyn ScriptTrait> {
        Box::new(self.clone())
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }

    fn as_any_ref_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A set of data, that provides contextual information for script methods.
pub struct ScriptContext<'a, 'b, 'c> {
    /// Amount of time that passed from last call. It has valid values only when called from `on_update`.
    pub dt: f32,

    /// Amount of time (in seconds) that passed from creation of the engine. Keep in mind, that
    /// this value is **not** guaranteed to match real time. A user can change delta time with
    /// which the engine "ticks" and this delta time affects elapsed time.
    pub elapsed_time: f32,

    /// A reference to the plugin which the script instance belongs to. You can use it to access plugin data
    /// inside script methods. For example you can store some "global" data in the plugin - for example a
    /// controls configuration, some entity managers and so on.
    pub plugins: &'a mut [Box<dyn Plugin>],

    /// Handle of a node to which the script instance belongs to. To access the node itself use `scene` field:
    ///
    /// ```rust
    /// # use fyrox::script::ScriptContext;
    /// # fn foo(context: ScriptContext) {
    /// let node_mut = &mut context.scene.graph[context.handle];
    /// # }
    /// ```
    pub handle: Handle<Node>,

    /// A reference to a scene the script instance belongs to. You have full mutable access to scene content
    /// in most of the script methods.
    pub scene: &'b mut Scene,

    /// A reference to resource manager, use it to load resources.
    pub resource_manager: &'a ResourceManager,

    /// An message sender. Every message sent via this sender will be then passed to every [`ScriptTrait::on_message`]
    /// method of every script.
    pub message_sender: &'c ScriptMessageSender,

    /// A message dispatcher. If you need to receive messages of a particular type, you must subscribe to a type
    /// explicitly. See [`ScriptTrait::on_message`] for more examples.
    pub message_dispatcher: &'c mut ScriptMessageDispatcher,
}

/// A set of data, that provides contextual information for script methods.
pub struct ScriptMessageContext<'a, 'b, 'c> {
    /// Amount of time that passed from last call. It has valid values only when called from `on_update`.
    pub dt: f32,

    /// Amount of time (in seconds) that passed from creation of the engine. Keep in mind, that
    /// this value is **not** guaranteed to match real time. A user can change delta time with
    /// which the engine "ticks" and this delta time affects elapsed time.
    pub elapsed_time: f32,

    /// A reference to the plugin which the script instance belongs to. You can use it to access plugin data
    /// inside script methods. For example you can store some "global" data in the plugin - for example a
    /// controls configuration, some entity managers and so on.
    pub plugins: &'a mut [Box<dyn Plugin>],

    /// Handle of a node to which the script instance belongs to. To access the node itself use `scene` field:
    ///
    /// ```rust
    /// # use fyrox::script::ScriptContext;
    /// # fn foo(context: ScriptContext) {
    /// let node_mut = &mut context.scene.graph[context.handle];
    /// # }
    /// ```
    pub handle: Handle<Node>,

    /// A reference to a scene the script instance belongs to. You have full mutable access to scene content
    /// in most of the script methods.
    pub scene: &'b mut Scene,

    /// A reference to resource manager, use it to load resources.
    pub resource_manager: &'a ResourceManager,

    /// An message sender. Every message sent via this sender will be then passed to every [`ScriptTrait::on_message`]
    /// method of every script.
    pub message_sender: &'c ScriptMessageSender,
}

/// A set of data that will be passed to a script instance just before its destruction.
pub struct ScriptDeinitContext<'a, 'b, 'c> {
    /// Amount of time (in seconds) that passed from creation of the engine. Keep in mind, that
    /// this value is **not** guaranteed to match real time. A user can change delta time with
    /// which the engine "ticks" and this delta time affects elapsed time.
    pub elapsed_time: f32,

    /// A reference to the plugin which the script instance belongs to. You can use it to access plugin data
    /// inside script methods. For example you can store some "global" data in the plugin - for example a
    /// controls configuration, some entity managers and so on.
    pub plugins: &'a mut [Box<dyn Plugin>],

    /// A reference to resource manager, use it to load resources.
    pub resource_manager: &'a ResourceManager,

    /// A reference to a scene the script instance was belonging to. You have full mutable access to scene content
    /// in most of the script methods.
    pub scene: &'b mut Scene,

    /// Handle to a parent scene node. Use it with caution because parent node could be deleted already and
    /// any unchecked borrowing using the handle will cause panic!
    pub node_handle: Handle<Node>,

    /// An message sender. Every message sent via this sender will be then passed to every [`ScriptTrait::on_message`]
    /// method of every script.
    pub message_sender: &'c ScriptMessageSender,
}

/// Script is a set predefined methods that are called on various stages by the engine. It is used to add
/// custom behaviour to game entities.
pub trait ScriptTrait: BaseScript + ComponentProvider {
    /// The method is called when the script wasn't initialized yet. It is guaranteed to be called once,
    /// and before any other methods of the script.
    ///
    /// # Important
    ///
    /// The method **will not** be called in case if you serialized initialized script instance and then
    /// loaded the instance. Internal flag will tell the engine that the script is initialized and this
    /// method **will not** be called. This is intentional design decision to be able to create save files
    /// in games. If you need a method that will be called in any case, use [`ScriptTrait::on_start`].
    fn on_init(&mut self, #[allow(unused_variables)] ctx: &mut ScriptContext) {}

    /// The method is called after [`ScriptTrait::on_init`], but in separate pass, which means that all
    /// script instances are already initialized. However, if implementor of this method creates a new
    /// node with a script, there will be a second pass of initialization. The method is guaranteed to
    /// be called once.
    fn on_start(&mut self, #[allow(unused_variables)] ctx: &mut ScriptContext) {}

    /// The method is called when the script is about to be destroyed. It is guaranteed to be called last.
    fn on_deinit(&mut self, #[allow(unused_variables)] ctx: &mut ScriptDeinitContext) {}

    /// Called when there is an event from the OS. The method allows you to "listen" for events
    /// coming from the main window of your game (or the editor if the game running inside the
    /// editor.
    fn on_os_event(
        &mut self,
        #[allow(unused_variables)] event: &Event<()>,
        #[allow(unused_variables)] ctx: &mut ScriptContext,
    ) {
    }

    /// Performs a single update tick of the script. The method may be called multiple times per
    /// frame, but it is guaranteed that the rate of call is stable and usually it will be called
    /// 60 times per second (this may change in future releases).
    fn on_update(&mut self, #[allow(unused_variables)] ctx: &mut ScriptContext) {}

    /// Allows you to react to certain script messages. It could be used for communication between scripts; to
    /// bypass borrowing issues. If you need to receive messages of a particular type, you must subscribe to a type
    /// explicitly. Usually it is done in [`ScriptTrait::on_start`] method:
    ///
    /// ```rust
    /// use fyrox::{
    ///     core::{reflect::prelude::*, uuid::Uuid, visitor::prelude::*},
    ///     impl_component_provider,
    ///     scene::node::TypeUuidProvider,
    ///     script::ScriptTrait,
    ///     script::{ScriptContext, ScriptMessageContext, ScriptMessagePayload},
    /// };
    ///
    /// struct Message;
    ///
    /// #[derive(Reflect, Visit, Debug, Clone)]
    /// struct MyScript {}
    ///
    /// # impl TypeUuidProvider for MyScript {
    /// #     fn type_uuid() -> Uuid {
    /// #         todo!();
    /// #     }
    /// # }
    ///
    /// # impl_component_provider!(MyScript);
    ///
    /// impl ScriptTrait for MyScript {
    ///     fn on_start(&mut self, ctx: &mut ScriptContext) {
    ///         // Subscription is mandatory to receive any message of the type!
    ///         ctx.message_dispatcher.subscribe_to::<Message>(ctx.handle)
    ///     }
    ///
    ///     fn on_message(
    ///         &mut self,
    ///         message: &mut dyn ScriptMessagePayload,
    ///         ctx: &mut ScriptMessageContext,
    ///     ) {
    ///         if let Some(message) = message.downcast_ref::<Message>() {
    ///             // Do something.
    ///         }
    ///     }
    ///
    ///     # fn id(&self) -> Uuid {
    ///     #     Self::type_uuid()
    ///     # }
    /// }
    /// ```
    fn on_message(
        &mut self,
        #[allow(unused_variables)] message: &mut dyn ScriptMessagePayload,
        #[allow(unused_variables)] ctx: &mut ScriptMessageContext,
    ) {
    }

    /// Script instance type UUID. The value will be used for serialization, to write type
    /// identifier to a data source so the engine can restore the script from data source.
    ///
    /// # Important notes
    ///
    /// Do **not** use [`Uuid::new_v4`] or any other [`Uuid`] methods that generates ids, ids
    /// generated using these methods are **random** and are not suitable for serialization!
    ///
    /// # Example
    ///
    /// All you need to do in the method is to return `Self::type_uuid`.
    ///
    /// ```rust
    /// use std::str::FromStr;
    /// use fyrox::{
    ///     scene::node::TypeUuidProvider,
    ///     core::visitor::prelude::*,
    ///     core::reflect::prelude::*,
    ///     core::uuid::Uuid,
    ///     script::ScriptTrait,
    ///     core::uuid::uuid, impl_component_provider
    /// };
    ///
    /// #[derive(Reflect, Visit, Debug, Clone)]
    /// struct MyScript { }
    ///
    /// // Implement TypeUuidProvider trait that will return type uuid of the type.
    /// // Every script must implement the trait so the script can be registered in
    /// // serialization context of the engine.
    /// impl TypeUuidProvider for MyScript {
    ///     fn type_uuid() -> Uuid {
    ///         // Use https://www.uuidgenerator.net/ to generate new UUID.
    ///         uuid!("4cfbe65e-a2c1-474f-b123-57516d80b1f8")
    ///     }
    /// }
    ///
    /// impl_component_provider!(MyScript);
    ///
    /// impl ScriptTrait for MyScript {
    ///     fn id(&self) -> Uuid {
    ///         Self::type_uuid()
    ///     }
    /// }
    /// ```
    fn id(&self) -> Uuid;
}

/// A wrapper for actual script instance internals, it used by the engine.
#[derive(Debug)]
pub struct Script {
    instance: Box<dyn ScriptTrait>,
    pub(crate) initialized: bool,
    pub(crate) started: bool,
}

impl Reflect for Script {
    fn type_name(&self) -> &'static str {
        self.instance.type_name()
    }

    fn fields_info(&self, func: &mut dyn FnMut(Vec<FieldInfo>)) {
        self.instance.fields_info(func)
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self.instance.into_any()
    }

    fn as_any(&self, func: &mut dyn FnMut(&dyn Any)) {
        self.instance.deref().as_any(func)
    }

    fn as_any_mut(&mut self, func: &mut dyn FnMut(&mut dyn Any)) {
        self.instance.deref_mut().as_any_mut(func)
    }

    fn as_reflect(&self, func: &mut dyn FnMut(&dyn Reflect)) {
        self.instance.deref().as_reflect(func)
    }

    fn as_reflect_mut(&mut self, func: &mut dyn FnMut(&mut dyn Reflect)) {
        self.instance.deref_mut().as_reflect_mut(func)
    }

    fn set(&mut self, value: Box<dyn Reflect>) -> Result<Box<dyn Reflect>, Box<dyn Reflect>> {
        self.instance.deref_mut().set(value)
    }

    fn fields(&self, func: &mut dyn FnMut(Vec<&dyn Reflect>)) {
        self.instance.deref().fields(func)
    }

    fn fields_mut(&mut self, func: &mut dyn FnMut(Vec<&mut dyn Reflect>)) {
        self.instance.deref_mut().fields_mut(func)
    }

    fn field(&self, name: &str, func: &mut dyn FnMut(Option<&dyn Reflect>)) {
        self.instance.deref().field(name, func)
    }

    fn field_mut(&mut self, name: &str, func: &mut dyn FnMut(Option<&mut dyn Reflect>)) {
        self.instance.deref_mut().field_mut(name, func)
    }

    fn as_array(&self, func: &mut dyn FnMut(Option<&dyn ReflectArray>)) {
        self.instance.deref().as_array(func)
    }

    fn as_array_mut(&mut self, func: &mut dyn FnMut(Option<&mut dyn ReflectArray>)) {
        self.instance.deref_mut().as_array_mut(func)
    }

    fn as_list(&self, func: &mut dyn FnMut(Option<&dyn ReflectList>)) {
        self.instance.deref().as_list(func)
    }

    fn as_list_mut(&mut self, func: &mut dyn FnMut(Option<&mut dyn ReflectList>)) {
        self.instance.deref_mut().as_list_mut(func)
    }
}

impl Deref for Script {
    type Target = dyn ScriptTrait;

    fn deref(&self) -> &Self::Target {
        &*self.instance
    }
}

impl DerefMut for Script {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.instance
    }
}

impl Visit for Script {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        let mut region_guard = visitor.enter_region(name)?;

        // Check for new format first, this branch will fail only on attempt to deserialize
        // scripts in old format.
        if self.instance.visit("Data", &mut region_guard).is_ok() {
            // Visit flags.
            self.initialized.visit("Initialized", &mut region_guard)?;
        } else {
            Log::warn(format!(
                "Unable to load script instance of id {} in new format! Trying to load in old format...",
                self.id()
            ));

            // Leave region and try to load in old format.
            drop(region_guard);

            self.instance.visit(name, visitor)?;

            Log::warn(format!(
                "Script instance of id {} loaded successfully using compatibility loader! Resave the script!",
                self.id()
            ));
        }

        Ok(())
    }
}

impl Clone for Script {
    fn clone(&self) -> Self {
        Self {
            instance: self.instance.clone_box(),
            initialized: false,
            started: false,
        }
    }
}

impl Script {
    /// Creates new script wrapper using given script instance.
    #[inline]
    pub fn new<T: ScriptTrait>(script_object: T) -> Self {
        Self {
            instance: Box::new(script_object),
            initialized: false,
            started: false,
        }
    }

    /// Performs downcasting to a particular type.
    #[inline]
    pub fn cast<T: ScriptTrait>(&self) -> Option<&T> {
        self.instance.deref().as_any_ref().downcast_ref::<T>()
    }

    /// Performs downcasting to a particular type.
    #[inline]
    pub fn cast_mut<T: ScriptTrait>(&mut self) -> Option<&mut T> {
        self.instance
            .deref_mut()
            .as_any_ref_mut()
            .downcast_mut::<T>()
    }

    /// Tries to borrow a component of given type.
    #[inline]
    pub fn query_component_ref<T: Any>(&self) -> Option<&T> {
        self.instance
            .query_component_ref(TypeId::of::<T>())
            .and_then(|c| c.downcast_ref())
    }

    /// Tries to borrow a component of given type.
    #[inline]
    pub fn query_component_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.instance
            .query_component_mut(TypeId::of::<T>())
            .and_then(|c| c.downcast_mut())
    }
}

#[cfg(test)]
mod test {
    use crate::{
        core::{
            reflect::prelude::*, uuid::Uuid, variable::try_inherit_properties,
            variable::InheritableVariable, visitor::prelude::*,
        },
        impl_component_provider,
        scene::base::Base,
        script::{Script, ScriptTrait},
    };

    #[derive(Reflect, Visit, Debug, Clone, Default)]
    struct MyScript {
        field: InheritableVariable<f32>,
    }

    impl_component_provider!(MyScript);

    impl ScriptTrait for MyScript {
        fn id(&self) -> Uuid {
            todo!()
        }
    }

    #[test]
    fn test_script_property_inheritance_on_nodes() {
        let mut child = Base::default();

        child.script = Some(Script::new(MyScript {
            field: InheritableVariable::new(1.23),
        }));

        let mut parent = Base::default();

        parent.script = Some(Script::new(MyScript {
            field: InheritableVariable::new(3.21),
        }));

        child.as_reflect_mut(&mut |child| {
            parent.as_reflect(&mut |parent| {
                try_inherit_properties(child, parent, &[]).unwrap();
            })
        });

        assert_eq!(
            *child.script().unwrap().cast::<MyScript>().unwrap().field,
            3.21
        );
    }

    #[test]
    fn test_script_property_inheritance() {
        let mut child = Script::new(MyScript {
            field: InheritableVariable::new(1.23),
        });

        let parent = Script::new(MyScript {
            field: InheritableVariable::new(3.21),
        });

        child.as_reflect_mut(&mut |child| {
            parent.as_reflect(&mut |parent| {
                try_inherit_properties(child, parent, &[]).unwrap();
            })
        });

        assert_eq!(*child.cast::<MyScript>().unwrap().field, 3.21);
    }

    #[test]
    fn test_script_property_inheritance_option() {
        let mut child = Some(Script::new(MyScript {
            field: InheritableVariable::new(1.23),
        }));

        let parent = Some(Script::new(MyScript {
            field: InheritableVariable::new(3.21),
        }));

        child.as_reflect_mut(&mut |child| {
            parent.as_reflect(&mut |parent| {
                try_inherit_properties(child, parent, &[]).unwrap();
            })
        });

        assert_eq!(
            *child.as_ref().unwrap().cast::<MyScript>().unwrap().field,
            3.21
        );
    }
}
