//! A widget for navigating through through a file system. Generally inspired by Finder.
//!
//! Useful for saving widgets that save/load files.
//!
//! - `DirectoryView`: Lists the contents of a single directory. Reacts to events for selection
//! of one or more files, de-selection, deletion and double-clicking.
//! - `FileView`: Displays some basic information about the file.

use {
    color,
    Color,
    Colorable,
    FontSize,
    NodeIndex,
    Positionable,
    Scalar,
    Sizeable,
    Widget,
};
use event;
use std;
use widget;

pub use self::directory_view::DirectoryView;

pub mod directory_view;

/// A widget for navigating and interacting with a file system.
pub struct FileNavigator<'a> {
    common: widget::CommonBuilder,
    /// Unique styling for the widget.
    pub style: Style,
    /// The first directory shown for the `FileNavigator`.
    pub starting_directory: &'a std::path::Path,
    /// Only display files of the given type.
    pub types: Types<'a>,
    /// Whether to show hidden files and directories
    show_hidden: bool,
}

/// A type for specifying the types of files to be shown by a `FileNavigator`.
#[derive(Copy, Clone)]
pub enum Types<'a> {
    /// Indicates that files of all types should be shown.
    All,
    /// A list of types of files that are accepted by the `FileNavigator`.
    ///
    /// i.e. `&["wav", "wave", "aiff"]`.
    WithExtension(&'a [&'a str]),
}

/// Unique state stored within the widget graph for each `FileNavigator`.
#[derive(Debug, PartialEq)]
pub struct State {
    /// A canvas upon which we can scroll the `DirectoryView`s horizontally.
    scrollable_canvas_idx: widget::IndexSlot,
    /// Horizontal scrollbar for manually scrolling the canvas.
    scrollbar_idx: widget::IndexSlot,
    /// The starting directory displayed by the `FileNavigator`.
    starting_directory: std::path::PathBuf,
    /// The stack of currently displayed directories.
    ///
    /// Directories are laid out left to right, where the left-most directory is initially the
    /// `starting_directory`.
    directory_stack: Vec<Directory>,
    /// The first `NodeIndex` is stored for the `DirectoryView` for each directory in the stack.
    ///
    /// The second is for the width-resizing `Rectangle`.
    directory_view_indices: Vec<(NodeIndex, NodeIndex)>,
}

/// Represents the state for a single directory.
#[derive(Debug, PartialEq)]
pub struct Directory {
    /// The path of the directory.
    path: std::path::PathBuf,
    /// The width of the `DirectoryView`.
    column_width: Scalar,
}

widget_style!{
    /// Unique styling for the widget.
    style Style {
        /// Color of the selected entries.
        - color: Color { theme.shape_color }
        /// The color of the unselected entries.
        - unselected_color: Option<Color> { None }
        /// The color of the directory and file names.
        - text_color: Option<Color> { None }
        /// The font size for the directory and file names.
        - font_size: FontSize { theme.font_size_medium }
        /// The default width of a single directory view.
        ///
        /// The first directory will always be initialised to this size.
        - column_width: Scalar { 250.0 }
        /// The width of the bar that separates each directory in the stack and allows for
        /// re-sizing.
        - resize_handle_width: Scalar { 5.0 }
    }
}

/// The kinds of events that the `FileNavigator` may produce.
#[derive(Clone, Debug)]
pub enum Event {
    /// The directory at the top of the stack has changed.
    ChangeDirectory(std::path::PathBuf),
    /// The selection of files in the top of the stack has changed.
    ChangeSelection(Vec<std::path::PathBuf>),
    /// A `Click` event occurred over a selection of entries.
    Click(event::Click, Vec<std::path::PathBuf>),
    /// A file was double clicked.
    DoubleClick(event::DoubleClick, Vec<std::path::PathBuf>),
    /// A `Press` event occurred over a selection of entries.
    Press(event::Press, Vec<std::path::PathBuf>),
    /// A `Release` event occurred over a selection of entries.
    Release(event::Release, Vec<std::path::PathBuf>),
}

impl<'a> FileNavigator<'a> {

    /// Begin building a `FileNavigator` widget that displays only files of the given types.
    pub fn new(starting_directory: &'a std::path::Path, types: Types<'a>) -> Self {
        FileNavigator {
            common: widget::CommonBuilder::new(),
            style: Style::new(),
            starting_directory: starting_directory,
            types: types,
            show_hidden: false,
        }
    }

    /// Begin building a `FileNavigator` that will display all file types.
    pub fn all(starting_directory: &'a std::path::Path) -> Self {
        Self::new(starting_directory, Types::All)
    }

    /// Begin building a `FileNavigator` that will only display files whose extensions match one
    /// of those within the given extension list.
    ///
    /// i.e. A `FileNavigator` used for navigating lossless audio files might use the following
    /// list of extensions: `&["wav", "wave", "aiff"]`.
    pub fn with_extension(starting_directory: &'a std::path::Path, exts: &'a [&'a str]) -> Self {
        Self::new(starting_directory, Types::WithExtension(exts))
    }

    /// The color of the unselected entries within each `DirectoryView`.
    pub fn unselected_color(mut self, color: Color) -> Self {
        self.style.unselected_color = Some(Some(color));
        self
    }

    /// The color of the `Text` used to display the file names.
    pub fn text_color(mut self, color: Color) -> Self {
        self.style.text_color = Some(Some(color));
        self
    }

    /// Whether to show hidden files and directories.
    pub fn show_hidden_files(mut self, show_hidden: bool) -> Self {
        self.show_hidden = show_hidden;
        self
    }

    builder_methods!{
        pub font_size { style.font_size = Some(FontSize) }
    }

}


impl<'a> Widget for FileNavigator<'a> {
    type State = State;
    type Style = Style;
    type Event = Vec<Event>;

    fn common(&self) -> &widget::CommonBuilder {
        &self.common
    }

    fn common_mut(&mut self) -> &mut widget::CommonBuilder {
        &mut self.common
    }

    fn init_state(&self) -> State {
        State {
            scrollable_canvas_idx: widget::IndexSlot::new(),
            scrollbar_idx: widget::IndexSlot::new(),
            directory_stack: Vec::new(),
            directory_view_indices: Vec::new(),
            starting_directory: std::path::PathBuf::new(),
        }
    }

    fn style(&self) -> Style {
        self.style.clone()
    }

    /// Update the state of the Button.
    fn update(self, args: widget::UpdateArgs<Self>) -> Self::Event {
        let widget::UpdateArgs { idx, state, style, rect, mut ui, .. } = args;
        let FileNavigator { starting_directory, types, .. } = self;

        if starting_directory != state.starting_directory {
            state.update(|state| {
                let width = style.column_width(&ui.theme);
                let path = starting_directory.to_path_buf();
                state.starting_directory = path.clone();
                state.directory_stack.clear();
                let dir = Directory { path: path, column_width: width };
                state.directory_stack.push(dir);
            });
        }

        let color = style.color(&ui.theme);
        let unselected_color = style.unselected_color(&ui.theme)
            .unwrap_or_else(|| color.plain_contrast().plain_contrast());
        let text_color = style.text_color(&ui.theme)
            .unwrap_or_else(|| color.plain_contrast());

        let scrollable_canvas_idx = state.scrollable_canvas_idx.get(&mut ui);
        widget::Rectangle::fill(rect.dim())
            .xy(rect.xy())
            .color(color::TRANSPARENT)
            .parent(idx)
            .scroll_kids_horizontally()
            .set(scrollable_canvas_idx, &mut ui);

        // A scrollbar for the `FOOTER` canvas.
        let scrollbar_idx = state.scrollbar_idx.get(&mut ui);
        widget::Scrollbar::x_axis(scrollable_canvas_idx)
            .color(color.plain_contrast())
            .auto_hide(true)
            .set(scrollbar_idx, &mut ui);

        // Collect all events that might occur.
        let mut events = Vec::new();

        // Instantiate a view for every directory in the stack.
        let mut i = 0;
        while i < state.directory_stack.len() {

            // Retrive the NodeIndex, or create one if necessary.
            let (view_idx, resize_idx) = match state.directory_view_indices.get(i) {
                Some(&indices) => indices,
                None => {
                    let view_idx = ui.new_unique_node_index();
                    let resize_idx = ui.new_unique_node_index();
                    let new_indices = (view_idx, resize_idx);
                    state.update(|state| state.directory_view_indices.push(new_indices));
                    new_indices
                },
            };

            let resize_handle_width = style.resize_handle_width(&ui.theme);
            let mut column_width = state.directory_stack[i].column_width;

            // Check to see if the resize handle has received any events.
            if let Some(resize_rect) = ui.rect_of(resize_idx) {
                let mut scroll_x = 0.0;
                for drag in ui.widget_input(resize_idx).drags().left() {
                    let target_w = column_width + drag.delta_xy[0];
                    let min_w = resize_rect.w() * 3.0;
                    let end_w = column_width + (rect.right() - resize_rect.right());
                    column_width = min_w.max(target_w);
                    state.update(|state| state.directory_stack[i].column_width = column_width);
                    // If we've dragged the column past end of the rect, scroll it.
                    if target_w > end_w {
                        scroll_x += target_w - end_w;
                    }
                }
                if scroll_x > 0.0 {
                    ui.scroll_widget(scrollable_canvas_idx, [-scroll_x, 0.0]);
                }
            }

            // Instantiate the `DirectoryView` widget and check for events.
            enum Action { EnterDir(std::path::PathBuf), ExitDir }

            let mut maybe_action = None;

            let directory_view_width = column_width - resize_handle_width;
            let font_size = style.font_size(&ui.theme);
            for event in DirectoryView::new(&state.directory_stack[i].path, types)
                .h(rect.h())
                .w(directory_view_width)
                .and(|view| if i == 0 { view.mid_left_of(idx) } else { view.right(0.0) })
                .color(color)
                .unselected_color(unselected_color)
                .text_color(text_color)
                .font_size(font_size)
                .show_hidden_files(self.show_hidden)
                .parent(scrollable_canvas_idx)
                .set(view_idx, &mut ui)
            {
                match event {

                    // The selection has changed.
                    directory_view::Event::Selection(paths) => {
                        // Check to see if the new selection is a directory to be entered.
                        if paths.len() == 1 {
                            let path = &paths[0];
                            if path.is_dir() {
                                maybe_action = Some(Action::EnterDir(path.clone()));
                            } else {
                                maybe_action = Some(Action::ExitDir);
                            }
                        } else {
                            maybe_action = Some(Action::ExitDir);
                        }
                        let event = Event::ChangeSelection(paths);
                        events.push(event);
                    },

                    // Propagate interactions.
                    directory_view::Event::Click(e, paths) =>
                        events.push(Event::Click(e, paths)),
                    directory_view::Event::DoubleClick(e, paths) =>
                        events.push(Event::DoubleClick(e, paths)),
                    directory_view::Event::Release(e, paths) =>
                        events.push(Event::Release(e, paths)),

                    // Check for directory navigation.
                    directory_view::Event::Press(press, paths) => {
                        if let Some(key_press) = press.key() {
                            use input;
                            match key_press.key {
                                input::Key::Right => if paths.len() == 1 {
                                    if paths[0].is_dir() {
                                        // TODO: Select top child of this dir and give keyboard
                                        // capturing to newly selected child.
                                    }
                                },
                                input::Key::Left => {
                                    // TODO: Exit top dir, enter parent dir and ensure no children
                                    // are selected.
                                },
                                _ => (),
                            }
                        }
                        events.push(Event::Press(press, paths));
                    },

                }
            }

            match maybe_action {

                // If we've entered a directory, clear the stack from this point and add our new
                // directory to the top of the stack.
                Some(Action::EnterDir(path)) => {
                    state.update(|state| {
                        let num_to_remove = state.directory_stack.len() - 1 - i;
                        for _ in 0..num_to_remove {
                            state.directory_stack.pop();
                        }
                        let dir = Directory { path: path.clone(), column_width: column_width };
                        state.directory_stack.push(dir);

                        let event = Event::ChangeDirectory(path);
                        events.push(event);
                    });

                    // If the resulting total width of all `DirectoryView`s would exceed the
                    // width of the `FileNavigator` itself, scroll toward the top-most
                    // `DirectoryView`.
                    let total_w = state.directory_stack.iter().fold(0.0, |t, d| t + d.column_width);
                    let overlap = total_w - rect.w();
                    if overlap > 0.0 {
                        ui.scroll_widget(scrollable_canvas_idx, [-overlap, 0.0]);
                    }
                },

                Some(Action::ExitDir) => {
                    let num_to_remove = state.directory_stack.len() - 1 - i;
                    for _ in 0..num_to_remove {
                        state.update(|state| { state.directory_stack.pop(); });
                    }
                },

                None => (),
            }

            // Instantiate the width-resizing handle's `Rectangle`.
            let resize_color = color.plain_contrast().plain_contrast();
            let resize_color = match ui.widget_input(resize_idx).mouse() {
                Some(mouse) => match mouse.buttons.left().is_down() {
                    true => resize_color.clicked().alpha(0.5),
                    false => resize_color.highlighted().alpha(0.2),
                },
                None => resize_color.alpha(0.2),
            };
            widget::Rectangle::fill([resize_handle_width, rect.h()])
                .color(resize_color)
                .right(0.0)
                .parent(scrollable_canvas_idx)
                .set(resize_idx, &mut ui);

            i += 1;
        }

        // If the canvas is pressed.
        if ui.widget_input(scrollable_canvas_idx).presses().mouse().left().next().is_some() {
            state.update(|state| {
                // Unselect everything.
                while state.directory_stack.len() > 1 {
                    state.directory_stack.pop();
                }
                // TODO: Need to unselect the selected directory here.
            });
        }

        events
    }

}

impl<'a> Colorable for FileNavigator<'a> {
    builder_method!(color { style.color = Some(Color) });
}
