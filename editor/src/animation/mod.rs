use crate::{
    animation::{
        command::ReplaceTrackCurveCommand,
        ruler::{RulerBuilder, RulerMessage},
        selection::{AnimationSelection, SelectedEntity},
        toolbar::Toolbar,
        track::TrackList,
    },
    scene::{EditorScene, Selection},
    Message,
};
use fyrox::{
    core::pool::Handle,
    engine::Engine,
    gui::{
        curve::{CurveEditorBuilder, CurveEditorMessage},
        grid::{Column, GridBuilder, Row},
        message::{MessageDirection, UiMessage},
        widget::{WidgetBuilder, WidgetMessage},
        window::{WindowBuilder, WindowMessage, WindowTitle},
        BuildContext, Thickness, UiNode, UserInterface,
    },
    scene::animation::AnimationPlayer,
};
use std::sync::mpsc::Sender;

mod command;
mod ruler;
pub mod selection;
mod toolbar;
mod track;

pub struct AnimationEditor {
    pub window: Handle<UiNode>,
    track_list: TrackList,
    curve_editor: Handle<UiNode>,
    toolbar: Toolbar,
    content: Handle<UiNode>,
    ruler: Handle<UiNode>,
}

fn fetch_selection(editor_selection: &Selection) -> AnimationSelection {
    if let Selection::Animation(ref selection) = editor_selection {
        // Some selection in an animation.
        AnimationSelection {
            animation_player: selection.animation_player,
            animation: selection.animation,
            entities: selection.entities.clone(),
        }
    } else if let Selection::Graph(ref selection) = editor_selection {
        // Only some AnimationPlayer is selected.
        AnimationSelection {
            animation_player: selection.nodes.first().cloned().unwrap_or_default(),
            animation: Default::default(),
            entities: vec![],
        }
    } else {
        // Stub in other cases.
        AnimationSelection {
            animation_player: Default::default(),
            animation: Default::default(),
            entities: vec![],
        }
    }
}

impl AnimationEditor {
    pub fn new(ctx: &mut BuildContext) -> Self {
        let curve_editor;
        let ruler;

        let track_list = TrackList::new(ctx);
        let toolbar = Toolbar::new(ctx);

        let payload = GridBuilder::new(
            WidgetBuilder::new()
                .on_row(1)
                .on_column(0)
                .with_child(track_list.panel)
                .with_child(
                    GridBuilder::new(
                        WidgetBuilder::new()
                            .on_row(0)
                            .on_column(1)
                            .with_child({
                                ruler = RulerBuilder::new(
                                    WidgetBuilder::new().on_row(0).with_margin(Thickness {
                                        left: 1.0,
                                        top: 1.0,
                                        right: 1.0,
                                        bottom: 0.0,
                                    }),
                                )
                                .with_value(0.0)
                                .build(ctx);
                                ruler
                            })
                            .with_child({
                                curve_editor = CurveEditorBuilder::new(
                                    WidgetBuilder::new().on_row(1).with_margin(Thickness {
                                        left: 1.0,
                                        top: 0.0,
                                        right: 1.0,
                                        bottom: 1.0,
                                    }),
                                )
                                .with_show_x_values(false)
                                .build(ctx);
                                curve_editor
                            }),
                    )
                    .add_row(Row::strict(25.0))
                    .add_row(Row::stretch())
                    .add_column(Column::stretch())
                    .build(ctx),
                ),
        )
        .add_row(Row::stretch())
        .add_column(Column::strict(250.0))
        .add_column(Column::stretch())
        .build(ctx);

        let content = GridBuilder::new(
            WidgetBuilder::new()
                .with_child(toolbar.panel)
                .with_child(payload),
        )
        .add_row(Row::strict(26.0))
        .add_row(Row::stretch())
        .add_column(Column::stretch())
        .build(ctx);

        let window = WindowBuilder::new(WidgetBuilder::new().with_width(600.0).with_height(500.0))
            .with_content(content)
            .open(false)
            .with_title(WindowTitle::text("Animation Editor"))
            .build(ctx);

        Self {
            window,
            track_list,
            curve_editor,
            toolbar,
            content,
            ruler,
        }
    }

    pub fn open(&self, ui: &UserInterface) {
        ui.send_message(WindowMessage::open(
            self.window,
            MessageDirection::ToWidget,
            true,
        ));
    }

    pub fn handle_ui_message(
        &mut self,
        message: &UiMessage,
        editor_scene: Option<&EditorScene>,
        engine: &mut Engine,
        sender: &Sender<Message>,
    ) {
        if let Some(editor_scene) = editor_scene {
            let selection = fetch_selection(&editor_scene.selection);

            let scene = &mut engine.scenes[editor_scene.scene];

            if let Some(animation_player) = scene
                .graph
                .try_get_mut(selection.animation_player)
                .and_then(|n| n.query_component_mut::<AnimationPlayer>())
            {
                self.toolbar.handle_ui_message(
                    message,
                    sender,
                    &engine.user_interface,
                    selection.animation_player,
                    animation_player,
                    editor_scene,
                    &selection,
                );

                self.track_list.handle_ui_message(
                    message,
                    editor_scene,
                    sender,
                    selection.animation_player,
                    selection.animation,
                    &mut engine.user_interface,
                    scene,
                );

                if let Some(msg) = message.data::<CurveEditorMessage>() {
                    if message.destination() == self.curve_editor
                        && message.direction() == MessageDirection::FromWidget
                    {
                        let ui = &engine.user_interface;
                        match msg {
                            CurveEditorMessage::Sync(curve) => {
                                sender
                                    .send(Message::do_scene_command(ReplaceTrackCurveCommand {
                                        animation_player: selection.animation_player,
                                        animation: selection.animation,
                                        curve: curve.clone(),
                                    }))
                                    .unwrap();
                            }
                            CurveEditorMessage::ViewPosition(position) => {
                                ui.send_message(RulerMessage::view_position(
                                    self.ruler,
                                    MessageDirection::ToWidget,
                                    position.x,
                                ))
                            }
                            CurveEditorMessage::Zoom(zoom) => ui.send_message(RulerMessage::zoom(
                                self.ruler,
                                MessageDirection::ToWidget,
                                zoom.x,
                            )),
                            _ => (),
                        }
                    }
                }
            }
        }
    }

    pub fn sync_to_model(&mut self, editor_scene: &EditorScene, engine: &mut Engine) {
        let selection = fetch_selection(&editor_scene.selection);

        let scene = &engine.scenes[editor_scene.scene];

        if let Some(animation_player) = scene
            .graph
            .try_get(selection.animation_player)
            .and_then(|n| n.query_component_ref::<AnimationPlayer>())
        {
            self.toolbar
                .sync_to_model(animation_player, &selection, &mut engine.user_interface);

            if let Some(animation) = animation_player.animations().try_get(selection.animation) {
                self.track_list
                    .sync_to_model(animation, &scene.graph, &mut engine.user_interface);

                // TODO: Support multi-selection.
                if let Some(SelectedEntity::Curve(selected_curve_id)) = selection.entities.first() {
                    if let Some(selected_curve) = animation.tracks().iter().find_map(|t| {
                        t.frames_container()
                            .curves_ref()
                            .iter()
                            .find(|c| &c.id() == selected_curve_id)
                    }) {
                        engine.user_interface.send_message(CurveEditorMessage::sync(
                            self.curve_editor,
                            MessageDirection::ToWidget,
                            selected_curve.clone(),
                        ));

                        engine
                            .user_interface
                            .send_message(CurveEditorMessage::zoom_to_fit(
                                self.curve_editor,
                                MessageDirection::ToWidget,
                            ));
                    }
                }
            }
            engine
                .user_interface
                .send_message(WidgetMessage::visibility(
                    self.content,
                    MessageDirection::ToWidget,
                    true,
                ));
        } else {
            engine
                .user_interface
                .send_message(WidgetMessage::visibility(
                    self.content,
                    MessageDirection::ToWidget,
                    false,
                ));
        }
    }
}