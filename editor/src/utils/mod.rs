use fyrox::{
    core::{algebra::Vector2, pool::ErasedHandle, pool::Handle},
    gui::{
        file_browser::{FileBrowserMode, FileSelectorBuilder, Filter},
        message::MessageDirection,
        widget::{WidgetBuilder, WidgetMessage},
        window::{Window, WindowBuilder},
        BuildContext, UiNode, UserInterface,
    },
    resource::texture::{CompressionOptions, Texture},
    scene::camera::{SkyBox, SkyBoxBuilder},
};

pub mod path_fixer;

pub fn is_slice_equal_permutation<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    if a.is_empty() && !b.is_empty() {
        false
    } else {
        // TODO: Find a way to do this faster.
        for source in a.iter() {
            let mut found = false;
            for other in b.iter() {
                if other == source {
                    found = true;
                    break;
                }
            }
            if !found {
                return false;
            }
        }
        true
    }
}

pub fn window_content(window: Handle<UiNode>, ui: &UserInterface) -> Handle<UiNode> {
    ui.node(window)
        .cast::<Window>()
        .map(|w| w.content())
        .unwrap_or_default()
}

pub fn enable_widget(handle: Handle<UiNode>, state: bool, ui: &UserInterface) {
    ui.send_message(WidgetMessage::enabled(
        handle,
        MessageDirection::ToWidget,
        state,
    ));
}

pub fn create_file_selector(
    ctx: &mut BuildContext,
    extension: &'static str,
    mode: FileBrowserMode,
) -> Handle<UiNode> {
    FileSelectorBuilder::new(
        WindowBuilder::new(WidgetBuilder::new().with_width(300.0).with_height(400.0)).open(false),
    )
    .with_filter(Filter::new(move |path| {
        if let Some(ext) = path.extension() {
            ext.to_string_lossy().as_ref() == extension
        } else {
            path.is_dir()
        }
    }))
    .with_mode(mode)
    .build(ctx)
}

pub fn fetch_node_center(handle: Handle<UiNode>, ctx: &BuildContext) -> Vector2<f32> {
    ctx.try_get_node(handle)
        .map(|node| node.center())
        .unwrap_or_default()
}

pub fn fetch_node_screen_center(handle: Handle<UiNode>, ctx: &BuildContext) -> Vector2<f32> {
    ctx.try_get_node(handle)
        .map(|node| node.screen_bounds().center())
        .unwrap_or_default()
}

pub fn fetch_node_screen_center_ui(handle: Handle<UiNode>, ui: &UserInterface) -> Vector2<f32> {
    ui.try_get_node(handle)
        .map(|node| node.screen_bounds().center())
        .unwrap_or_default()
}

fn load_texture(data: &[u8]) -> Texture {
    Texture::load_from_memory(data, CompressionOptions::NoCompression, false)
        .ok()
        .unwrap()
}

pub fn built_in_skybox() -> SkyBox {
    let front = load_texture(include_bytes!("../../resources/embed/skybox/front.png"));
    let back = load_texture(include_bytes!("../../resources/embed/skybox/back.png"));
    let top = load_texture(include_bytes!("../../resources/embed/skybox/top.png"));
    let bottom = load_texture(include_bytes!("../../resources/embed/skybox/bottom.png"));
    let left = load_texture(include_bytes!("../../resources/embed/skybox/left.png"));
    let right = load_texture(include_bytes!("../../resources/embed/skybox/right.png"));

    SkyBoxBuilder {
        front: Some(front),
        back: Some(back),
        left: Some(left),
        right: Some(right),
        top: Some(top),
        bottom: Some(bottom),
    }
    .build()
    .unwrap()
}

pub fn make_node_name(name: &str, handle: ErasedHandle) -> String {
    format!("{} ({}:{})", name, handle.index(), handle.generation())
}

pub fn apply_visibility_filter<F>(root: Handle<UiNode>, ui: &UserInterface, filter: F)
where
    F: Fn(&UiNode) -> Option<bool>,
{
    fn apply_filter_recursive<F>(node: Handle<UiNode>, ui: &UserInterface, filter: &F) -> bool
    where
        F: Fn(&UiNode) -> Option<bool>,
    {
        let node_ref = ui.node(node);

        let mut is_any_match = false;
        for &child in node_ref.children() {
            is_any_match |= apply_filter_recursive(child, ui, filter)
        }

        if let Some(has_match) = filter(node_ref) {
            is_any_match |= has_match;

            ui.send_message(WidgetMessage::visibility(
                node,
                MessageDirection::ToWidget,
                is_any_match,
            ));
        }

        is_any_match
    }

    apply_filter_recursive(root, ui, &filter);
}
