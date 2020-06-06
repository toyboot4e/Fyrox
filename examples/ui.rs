//! Example 04. User Interface
//!
//! Difficulty: Easy
//!
//! This example shows how to use user interface system of engine. It is
//! based on simple.rs example because UI will be used to operate on
//! model.

extern crate rg3d;

use std::{
    time::Instant,
    sync::{Arc, Mutex},
};
use rg3d::{
    scene::{
        base::BaseBuilder,
        transform::TransformBuilder,
        camera::CameraBuilder,
        node::Node,
        Scene,
    },
    engine::resource_manager::ResourceManager,
    event::{
        Event,
        WindowEvent,
        DeviceEvent,
        VirtualKeyCode,
        ElementState,
    },
    event_loop::{
        EventLoop,
        ControlFlow,
    },
    core::{
        color::Color,
        pool::Handle,
        math::{
            vec3::Vec3,
            quat::Quat,
            vec2::Vec2,
        },
    },
    window::Fullscreen,
    monitor::VideoMode,
    animation::Animation,
    utils::translate_event,
    gui::{
        stack_panel::StackPanelBuilder,
        grid::{GridBuilder, Column, Row},
        scroll_bar::{ScrollBarBuilder},
        Thickness,
        VerticalAlignment,
        HorizontalAlignment,
        window::{WindowBuilder, WindowTitle},
        button::ButtonBuilder,
        message::{
            UiMessageData,
            ScrollBarMessage,
            ButtonMessage,
            ListViewMessage,
        },
        widget::WidgetBuilder,
        text::TextBuilder,
        node::StubNode,
        dropdown_list::DropdownListBuilder,
        decorator::DecoratorBuilder,
        border::BorderBuilder,
        Orientation
    }
};

const DEFAULT_MODEL_ROTATION: f32 = 180.0;
const DEFAULT_MODEL_SCALE: f32 = 0.05;

// Create our own engine type aliases. These specializations are needed
// because engine provides a way to extend UI with custom nodes and messages.
type GameEngine = rg3d::engine::Engine<(), StubNode>;
type UiNode = rg3d::gui::node::UINode<(), StubNode>;

struct Interface {
    debug_text: Handle<UiNode>,
    yaw: Handle<UiNode>,
    scale: Handle<UiNode>,
    reset: Handle<UiNode>,
    video_modes: Vec<VideoMode>,
    resolutions: Handle<UiNode>,
}

// User interface in the engine build up on graph data structure, on tree to be
// more precise. Each UI element can has single parent and multiple children.
// UI uses complex layout system which automatically organizes your widgets.
// In this example we'll use Grid and StackPanel layout controls. Grid can be
// divided in rows and columns, its child element can set their desired column
// and row and grid will automatically put them in correct position. StackPanel
// will "stack" UI elements either on top of each other or in one line. Such
// complex layout system was borrowed from WPF framework. You can read more here:
// https://docs.microsoft.com/en-us/dotnet/framework/wpf/advanced/layout
fn create_ui(engine: &mut GameEngine) -> Interface {
    let window_width = engine.renderer.get_frame_size().0 as f32;

    // Gather all suitable video modes, we'll use them to fill combo box of
    // available resolutions.
    let video_modes =
        engine.get_window()
            .primary_monitor()
            .video_modes()
            .filter(|vm| {
                // Leave only modern video modes, we are not in 1998.
                vm.size().width > 800 &&
                    vm.size().height > 600 &&
                    vm.bit_depth() == 32
            })
            .collect::<Vec<_>>();

    let ui = &mut engine.user_interface;

    // First of all create debug text that will show title of example and current FPS.
    let debug_text = TextBuilder::new(WidgetBuilder::new())
        .build(ui);

    // Then create model options window.
    let yaw;
    let scale;
    let reset;
    WindowBuilder::new(WidgetBuilder::new()
        // We want the window to be anchored at right top corner at the beginning
        .with_desired_position(Vec2::new(window_width - 300.0, 0.0))
        .with_width(300.0))
        // Window can have any content you want, in this example it is Grid with other
        // controls. The layout looks like this:
        //  ______________________________
        // | Yaw         | Scroll bar    |
        // |_____________|_______________|
        // | Scale       | Scroll bar    |
        // |_____________|_______________|
        // |             | Reset button  |
        // |_____________|_______________|
        //
        .with_content(GridBuilder::new(WidgetBuilder::new()
            .with_child(TextBuilder::new(WidgetBuilder::new()
                .on_row(0)
                .on_column(0)
                .with_vertical_alignment(VerticalAlignment::Center))
                .with_text("Yaw")
                .build(ui))
            .with_child({
                yaw = ScrollBarBuilder::new(WidgetBuilder::new()
                    .on_row(0)
                    .on_column(1)
                    // Make sure scroll bar will stay in center of available space.
                    .with_vertical_alignment(VerticalAlignment::Center)
                    // Add some margin so ui element won't be too close to each other.
                    .with_margin(Thickness::uniform(2.0)))
                    .with_min(0.0)
                    // Our max rotation is 360 degrees.
                    .with_max(360.0)
                    // Set some initial value
                    .with_value(DEFAULT_MODEL_ROTATION)
                    // Set step by which value will change when user will click on arrows.
                    .with_step(5.0)
                    // Make sure scroll bar will show its current value on slider.
                    .show_value(true)
                    // Turn off all decimal places.
                    .with_value_precision(0)
                    .build(ui);
                yaw
            })
            .with_child(TextBuilder::new(WidgetBuilder::new()
                .on_row(1)
                .on_column(0)
                .with_vertical_alignment(VerticalAlignment::Center))
                .with_text("Scale")
                .build(ui))
            .with_child({
                scale = ScrollBarBuilder::new(WidgetBuilder::new()
                    .on_row(1)
                    .on_column(1)
                    .with_vertical_alignment(VerticalAlignment::Center)
                    .with_margin(Thickness::uniform(2.0)))
                    .with_min(0.01)
                    .with_max(0.1)
                    .with_step(0.01)
                    .with_value(DEFAULT_MODEL_SCALE)
                    .show_value(true)
                    .build(ui);
                scale
            })
            .with_child(StackPanelBuilder::new(WidgetBuilder::new()
                .on_row(2)
                .on_column(1)
                .with_horizontal_alignment(HorizontalAlignment::Right)
                .with_child({
                    reset = ButtonBuilder::new(WidgetBuilder::new())
                        .with_text("Reset")
                        .build(ui);
                    reset
                }))
                .with_orientation(Orientation::Horizontal)
                .build(ui)))
            .add_column(Column::strict(100.0))
            .add_column(Column::stretch())
            .add_row(Row::strict(30.0))
            .add_row(Row::strict(30.0))
            .add_row(Row::strict(30.0))
            .build(ui))
        .with_title(WindowTitle::text("Model Options"))
        .can_close(false)
        .build(ui);

    // Create another window which will show some graphics options.
    let resolutions;
    WindowBuilder::new(WidgetBuilder::new()
        .with_desired_position(Vec2::new(window_width - 670.0, 0.0))
        .with_width(350.0))
        .with_content(GridBuilder::new(WidgetBuilder::new()
            .with_child(TextBuilder::new(WidgetBuilder::new()
                .on_column(0)
                .on_row(0))
                .with_text("Resolution")
                .build(ui))
            .with_child({
                resolutions = DropdownListBuilder::new(WidgetBuilder::new()
                    .on_row(0)
                    .on_column(1))
                    // Set combo box items - each item will represent video mode value.
                    // When user will select something, we'll receive SelectionChanged
                    // message and will use received index to switch to desired video
                    // mode.
                    .with_items({
                        let mut items = Vec::new();
                        for video_mode in video_modes.iter() {
                            let size = video_mode.size();
                            let rate = video_mode.refresh_rate();
                            let item = DecoratorBuilder::new(BorderBuilder::new(WidgetBuilder::new()
                                .with_height(28.0)
                                .with_child(TextBuilder::new(WidgetBuilder::new()
                                    .with_horizontal_alignment(HorizontalAlignment::Center))
                                    .with_text(format!("{}x{}@{}Hz", size.width, size.height, rate))
                                    .build(ui))))
                                .build(ui);
                            items.push(item);
                        }
                        items
                    })
                    .build(ui);
                resolutions
            }))
            .add_column(Column::strict(120.0))
            .add_column(Column::stretch())
            .add_row(Row::strict(30.0))
            .build(ui))
        .with_title(WindowTitle::text("Graphics Options"))
        .can_close(false)
        .build(ui);

    Interface {
        debug_text,
        yaw,
        scale,
        reset,
        resolutions,
        video_modes,
    }
}

struct GameScene {
    scene: Scene,
    model_handle: Handle<Node>,
    walk_animation: Handle<Animation>,
}

fn create_scene(resource_manager: Arc<Mutex<ResourceManager>>) -> GameScene {
    let mut scene = Scene::new();

    let mut resource_manager = resource_manager.lock().unwrap();

    // Camera is our eyes in the world - you won't see anything without it.
    let camera = CameraBuilder::new(BaseBuilder::new()
        .with_local_transform(TransformBuilder::new()
            .with_local_position(Vec3::new(0.0, 6.0, -12.0))
            .build()))
        .build();

    scene.graph.add_node(Node::Camera(camera));

    // Load model resource. Is does *not* adds anything to our scene - it just loads a
    // resource then can be used later on to instantiate models from it on scene. Why
    // loading of resource is separated from instantiation? Because there it is too
    // inefficient to load a resource every time you trying to create instance of it -
    // much more efficient is to load it one and then make copies of it. In case of
    // models it is very efficient because single vertex and index buffer can be used
    // for all models instances, so memory footprint on GPU will be lower.
    let model_resource = resource_manager.request_model("examples/data/mutant.FBX").unwrap();

    // Instantiate model on scene - but only geometry, without any animations.
    // Instantiation is a process of embedding model resource data in desired scene.
    let model_handle = model_resource.lock()
        .unwrap()
        .instantiate_geometry(&mut scene);

    // Add simple animation for our model. Animations are loaded from model resources -
    // this is because animation is a set of skeleton bones with their own transforms.
    let walk_animation_resource = resource_manager.request_model("examples/data/walk.fbx").unwrap();

    // Once animation resource is loaded it must be re-targeted to our model instance.
    // Why? Because animation in *resource* uses information about *resource* bones,
    // not model instance bones, retarget_animations maps animations of each bone on
    // model instance so animation will know about nodes it should operate on.
    let walk_animation = *walk_animation_resource
        .lock()
        .unwrap()
        .retarget_animations(model_handle, &mut scene)
        .get(0)
        .unwrap();

    GameScene {
        scene,
        model_handle,
        walk_animation,
    }
}

fn main() {
    let event_loop = EventLoop::new();

    let window_builder = rg3d::window::WindowBuilder::new()
        .with_title("Example - Model")
        .with_resizable(true);

    let mut engine = GameEngine::new(window_builder, &event_loop).unwrap();

    // Prepare resource manager - it must be notified where to search textures. When engine
    // loads model resource it automatically tries to load textures it uses. But since most
    // model formats store absolute paths, we can't use them as direct path to load texture
    // instead we telling engine to search textures in given folder.
    engine.resource_manager.lock().unwrap().set_textures_path("examples/data");

    // Create simple user interface that will show some useful info.
    let interface = create_ui(&mut engine);

    // Create test scene.
    let GameScene { scene, model_handle, walk_animation } = create_scene(engine.resource_manager.clone());

    // Add scene to engine - engine will take ownership over scene and will return
    // you a handle to scene which can be used later on to borrow it and do some
    // actions you need.
    let scene_handle = engine.scenes.add(scene);

    // Set ambient light.
    engine.renderer.set_ambient_color(Color::opaque(200, 200, 200));

    let clock = Instant::now();
    let fixed_timestep = 1.0 / 60.0;
    let mut elapsed_time = 0.0;

    // We will rotate model using keyboard input.
    let mut model_angle = DEFAULT_MODEL_ROTATION;
    let mut model_scale = DEFAULT_MODEL_SCALE;

    // Finally run our event loop which will respond to OS and window events and update
    // engine state accordingly. Engine lets you to decide which event should be handled,
    // this is minimal working example if how it should be.
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::MainEventsCleared => {
                // This main game loop - it has fixed time step which means that game
                // code will run at fixed speed even if renderer can't give you desired
                // 60 fps.
                let mut dt = clock.elapsed().as_secs_f32() - elapsed_time;
                while dt >= fixed_timestep {
                    dt -= fixed_timestep;
                    elapsed_time += fixed_timestep;

                    // ************************
                    // Put your game logic here.
                    // ************************

                    // Use stored scene handle to borrow a mutable reference of scene in
                    // engine.
                    let scene = &mut engine.scenes[scene_handle];

                    // Our animation must be applied to scene explicitly, otherwise
                    // it will have no effect.
                    scene.animations
                        .get_mut(walk_animation)
                        .get_pose()
                        .apply(&mut scene.graph);

                    scene.graph[model_handle]
                        .local_transform_mut()
                        .set_scale(Vec3::new(model_scale, model_scale, model_scale))
                        .set_rotation(Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), model_angle.to_radians()));

                    if let UiNode::Text(text) = engine.user_interface.node_mut(interface.debug_text) {
                        let fps = engine.renderer.get_statistics().frames_per_second;
                        text.set_text(format!("Example 04 - User Interface\nFPS: {}", fps));
                    }

                    engine.update(fixed_timestep);
                }

                // It is very important to "pump" messages from UI. This our main point where we communicate
                // with user interface. As you saw earlier, there is no callbacks on UI elements, instead we
                // use messages to get information from UI elements. This provides perfect decoupling of logic
                // from UI elements and works well with borrow checker.
                while let Some(ui_message) = engine.user_interface.poll_message() {
                    match &ui_message.data {
                        UiMessageData::ScrollBar(sb) => {
                            // Some of our scroll bars has changed its value. Check which one.
                            if let &ScrollBarMessage::Value(value) = sb {
                                // Each message has source - a handle of UI element that created this message.
                                // It is used to understand from which UI element message has come.
                                if ui_message.destination == interface.scale {
                                    model_scale = value;
                                } else if ui_message.destination == interface.yaw {
                                    model_angle = value;
                                }
                            }
                        }
                        UiMessageData::Button(btn) => {
                            if let ButtonMessage::Click = btn {
                                // Once we received Click event from Reset button, we have to reset angle and scale
                                // of model. To do that we borrow each UI element in engine and set its value directly.
                                // This is not ideal because there is tight coupling between UI code and model values,
                                // but still good enough for example.
                                if ui_message.destination == interface.reset {
                                    if let UiNode::ScrollBar(scale_sb) = engine.user_interface.node_mut(interface.scale) {
                                        scale_sb.set_value(DEFAULT_MODEL_SCALE);
                                    }
                                    if let UiNode::ScrollBar(rotation_sb) = engine.user_interface.node_mut(interface.yaw) {
                                        rotation_sb.set_value(DEFAULT_MODEL_ROTATION);
                                    }
                                }
                            }
                        }
                        UiMessageData::ListView(ic) => {
                            if let ListViewMessage::SelectionChanged(idx) = ic {
                                // Video mode has changed and we must change video mode to what user wants.
                                if let &Some(idx) = idx {
                                    if ui_message.destination == interface.resolutions {
                                        let video_mode = interface.video_modes.get(idx).unwrap();
                                        engine.get_window().set_fullscreen(Some(Fullscreen::Exclusive(video_mode.clone())));

                                        // Due to some weird bug in winit it does not send Resized event.
                                        engine.renderer.set_frame_size((video_mode.size().width, video_mode.size().height));
                                    }
                                }
                            }
                        }
                        _ => ()
                    }
                }

                // Rendering must be explicitly requested and handled after RedrawRequested event is received.
                engine.get_window().request_redraw();
            }
            Event::RedrawRequested(_) => {
                // Run renderer at max speed - it is not tied to game code.
                engine.render(fixed_timestep).unwrap();
            }
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit
                    }
                    WindowEvent::Resized(size) => {
                        // It is very important to handle Resized event from window, because
                        // renderer knows nothing about window size - it must be notified
                        // directly when window size has changed.
                        engine.renderer.set_frame_size(dbg!(size.into()));
                    }
                    _ => ()
                }

                // It is very important to "feed" user interface (UI) with events coming
                // from main window, otherwise UI won't respond to mouse, keyboard, or any
                // other event.
                if let Some(os_event) = translate_event(&event) {
                    engine.user_interface.process_os_event(&os_event);
                }
            }
            Event::DeviceEvent { event, .. } => {
                if let DeviceEvent::Key(key) = event {
                    if let Some(key_code) = key.virtual_keycode {
                        if key.state == ElementState::Pressed && key_code == VirtualKeyCode::Escape {
                            *control_flow = ControlFlow::Exit;
                        }
                    }
                }
            }
            _ => *control_flow = ControlFlow::Poll,
        }
    });
}