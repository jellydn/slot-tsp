use slot_gfx::{Compositor, Draw, TexId, OUT_H};
use slot_input::{Action, Btn, Context, Gestures, Millis, PhysicalEvent, RawEvent, MENU_HOLD_MS};
use slot_retro::{ButtonMask, Controller};
use slot_store::{CarouselKeys, Controls, Core, Platform, ShortcutKeys};

use super::{Pad, Table};

/// A read-only rehearsal of the same PS1 gesture, shortcut and binding path as Session.
/// Nothing produced here is dispatched to the app, a core, or a hardware setter.
pub struct InputTest {
    gestures: Gestures,
    pad: Pad,
    controls: Controls,
    core: Option<Core>,
    controller: Controller,
    physical: Vec<PhysicalEvent>,
    physical_available: bool,
    pub(crate) analog_available: bool,
    logical: String,
    shortcut: String,
    seen: u16,
    menu_down_at: Option<Millis>,
    pub(crate) sticks: [[i16; 2]; 2],
}

impl InputTest {
    pub(crate) fn new(
        controls: Controls,
        shortcuts: ShortcutKeys,
        core: Option<Core>,
        controller: Controller,
    ) -> Self {
        let mut gestures = Gestures::new();
        gestures.set_context(Context::Game);
        gestures.set_entries(
            Table::new(
                &controls,
                &shortcuts,
                &CarouselKeys::default(),
                false,
                Platform::Ps,
            )
            .entries(),
        );
        Self {
            gestures,
            pad: Pad::default(),
            controls,
            core,
            controller,
            physical: Vec::new(),
            physical_available: false,
            analog_available: false,
            logical: "Waiting for a button".into(),
            shortcut: "None".into(),
            seen: 0,
            menu_down_at: None,
            sticks: [[0; 2]; 2],
        }
    }

    pub(crate) fn observe(&mut self, available: bool, events: Vec<PhysicalEvent>) {
        self.physical_available = available;
        for ev in events {
            self.physical.retain(|old| {
                (old.node.as_str(), old.kind, old.code) != (ev.node.as_str(), ev.kind, ev.code)
            });
            self.physical.insert(0, ev);
            self.physical.truncate(6);
        }
    }

    pub(crate) fn feed(&mut self, events: &[RawEvent], now: Millis) -> bool {
        let mut back = false;
        for &ev in events {
            self.logical = format!("{ev:?}");
            // Reserve the hold independently of shortcut/chord recognition so exit is
            // still available while testing a Menu modifier or repeated Menu presses.
            match ev {
                RawEvent::Down(Btn::Menu) => {
                    self.menu_down_at.get_or_insert(now);
                }
                RawEvent::Up(Btn::Menu) => {
                    back |= self
                        .menu_down_at
                        .take()
                        .is_some_and(|since| now.saturating_sub(since) >= MENU_HOLD_MS);
                }
                _ => {}
            }
            for action in self.gestures.feed(ev, now) {
                self.apply(action);
            }
        }
        for action in self.gestures.tick(now) {
            self.apply(action);
        }
        back || self
            .menu_down_at
            .is_some_and(|since| now.saturating_sub(since) >= MENU_HOLD_MS)
    }

    fn apply(&mut self, action: Action) {
        self.pad.apply(action);
        self.seen |= self.mask().0;
        if !matches!(
            action,
            Action::GbaDown(_)
                | Action::GbaUp(_)
                | Action::MenuChordDown(_)
                | Action::MenuChordUp(_)
        ) {
            self.shortcut = format!("{action:?} (not executed)");
        }
    }

    pub fn mask(&self) -> ButtonMask {
        self.pad.mask(&self.controls, Platform::Ps)
    }

    pub fn sticks(&self) -> [[i16; 2]; 2] {
        self.sticks
    }

    pub fn lines(&self) -> Vec<String> {
        let status = |name: &str, bit| {
            let state = if self.mask().0 & bit != 0 {
                "HELD"
            } else if self.seen & bit != 0 {
                "seen"
            } else {
                "--"
            };
            format!("{name}: {state}")
        };
        let mut lines = vec![
            "Input Test / PS1".into(),
            self.core.map_or("No PS1 cart selected".into(), |c| {
                format!("Selected: {}", c.text())
            }),
            format!("Setting: {:?} / mapping preview", self.controller),
            "Raw device / latest values".into(),
            "Effective PS1 / HELD or seen".into(),
        ];
        for i in 0..6 {
            lines.push(self.physical.get(i).map_or_else(
                || {
                    if i != 0 {
                        String::new()
                    } else if self.physical_available {
                        "Waiting for KEY / ABS events".into()
                    } else {
                        "Raw evdev unavailable".into()
                    }
                },
                |ev| {
                    format!(
                        "{} {} 0x{:03x} = {}",
                        ev.node,
                        if ev.kind == 1 { "KEY" } else { "ABS" },
                        ev.code,
                        ev.value
                    )
                },
            ));
        }
        for [(left, a), (right, z)] in [
            [("Up", ButtonMask::UP), ("Down", ButtonMask::DOWN)],
            [("Left", ButtonMask::LEFT), ("Right", ButtonMask::RIGHT)],
            [("Cross", ButtonMask::B), ("Circle", ButtonMask::A)],
            [("Square", ButtonMask::Y), ("Triangle", ButtonMask::X)],
            [("Select", ButtonMask::SELECT), ("Start", ButtonMask::START)],
            [("L1", ButtonMask::L), ("R1", ButtonMask::R)],
            [("L2", ButtonMask::L2), ("R2", ButtonMask::R2)],
        ] {
            lines.push(format!("{}   {}", status(left, a), status(right, z)));
        }
        lines.push(format!("Logical: {}", self.logical));
        lines.push(format!("Shortcut: {}", self.shortcut));
        for (i, [x, y]) in self.sticks.iter().enumerate() {
            if !self.analog_available {
                lines.push(format!("{} stick: unavailable", ["Left", "Right"][i]));
                continue;
            }
            lines.push(format!(
                "{} stick  X {:+.2}  Y {:+.2}{}",
                ["Left", "Right"][i],
                f32::from(*x) / 32767.0,
                f32::from(*y) / 32767.0,
                if *x == 0 && *y == 0 { "  CENTER" } else { "" }
            ));
        }
        lines.push(if super::evdev::pad_map() == super::evdev::PadMap::Tsp {
            "TSP: no physical L3 / R3".into()
        } else {
            "L3 / R3: not mapped by Slot".into()
        });
        lines.push(if self.controller == Controller::Digital {
            "Digital mode: core ignores stick axes".into()
        } else {
            "Axes after deadzone; center = 0.00".into()
        });
        lines.push("Hold Menu (1 second): Back".into());
        lines.push("Game input blocked. No bindings are changed.".into());
        lines
    }
}

#[derive(Default)]
pub(crate) struct InputTestView {
    text: Vec<String>,
    faces: Vec<(TexId, u32, u32)>,
}

impl InputTestView {
    pub fn draw(&mut self, test: &InputTest, c: &mut Compositor, width: f32) {
        let lines = test.lines();
        for (i, text) in lines.iter().enumerate() {
            if self.text.get(i) == Some(text) {
                continue;
            }
            let face = slot_ui::word_ink(text, [246, 244, 239]);
            if let Some((id, w, h)) = self.faces.get_mut(i) {
                c.update_texture(*id, face.w, face.h, &face.rgba);
                (*w, *h) = (face.w, face.h);
            } else {
                self.faces
                    .push((c.create_texture(face.w, face.h, &face.rgba), face.w, face.h));
            }
        }
        self.text = lines;
        let mut draws = vec![Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: OUT_H as f32,
            colour: [0.10, 0.10, 0.09, 1.0],
        }];
        for (i, &(tex, w, h)) in self.faces.iter().enumerate() {
            let (x, y) = match i {
                0..=2 => (20.0, 12.0 + i as f32 * 25.0),
                3 => (20.0, 98.0),
                4 => (width / 2.0 + 10.0, 98.0),
                5..=10 => (20.0, 129.0 + (i - 5) as f32 * 25.0),
                11..=17 => (width / 2.0 + 10.0, 129.0 + (i - 11) as f32 * 25.0),
                18..=19 => (20.0, 287.0 + (i - 18) as f32 * 25.0),
                20..=21 => (20.0 + (i - 20) as f32 * width / 2.0, 340.0),
                22..=23 => (20.0 + (i - 22) as f32 * width / 2.0, 370.0),
                _ => (20.0, 416.0 + (i - 24) as f32 * 24.0),
            };
            let max = if matches!(i, 3..=17 | 20..=23) {
                width / 2.0 - 30.0
            } else {
                width - 40.0
            };
            let scale = (max / w.max(1) as f32).min(1.0);
            draws.push(Draw::Tex {
                x,
                y,
                w: w as f32 * scale,
                h: h as f32 * scale,
                tex,
                alpha: 1.0,
            });
        }
        c.draw_list(&draws);
    }
}
