use md5::{Digest, Md5};
use swofty_formats::hash::djb2;
use swofty_formats::pkg::{Arr, Node, Obj, Pkg, Val, Value};

use crate::{Error, Result};

pub const PROPS: usize = 0;
pub const CHILDREN: usize = 1;
pub const COMMANDS: usize = 2;
pub const EVENTS: usize = 3;
pub const LAYOUT: usize = 4;
/// A node's own component scene, as a block's o_block.
pub const COMPONENT: usize = 5;
pub const BEHAVIOURS: usize = 6;

pub const SCREEN_BEHAVIOURS: &[&str] = &[
    "NBEL", "NBDC", "NBMG", "NBVM", "NBAC", "NBOS", "NBXA", "NBMO", "NBTR",
];
pub const GROUP_BEHAVIOURS: &[&str] = &[
    "NBEL", "NBDC", "NBMG", "NBVM", "NBFC", "NBAN", "NBAC", "NBOS", "NBXA", "NBMO",
];
const DECISIONS: &[&str] = &["SVCD", "MVCD", "DBVV", "DBRV", "DBCD", "DBAM", "WUDN", "UIVF"];

pub fn err(msg: impl std::fmt::Display) -> Error {
    Error::Other(format!("menus: {msg}"))
}

pub fn fourcc(s: &str) -> u32 {
    u32::from_le_bytes(s.as_bytes()[..4].try_into().expect("four characters"))
}

pub fn fourcc_str(v: u32) -> String {
    String::from_utf8_lossy(&v.to_le_bytes()).into_owned()
}

pub fn s(v: &str) -> Node {
    Node::Val(Val::str(v))
}

pub fn u32v(v: u32) -> Node {
    Node::Val(Val::u32(v))
}

pub fn boolv(v: bool) -> Node {
    Node::Val(Val::bool(v))
}

pub fn f32v(v: f32) -> Node {
    Node::Val(Val::raw(*b"fp32", v.to_le_bytes()))
}

pub fn i32v(v: i32) -> Node {
    Node::Val(Val::raw(*b"si32", v.to_le_bytes()))
}

pub fn obj(pairs: Vec<(&str, Node)>) -> Node {
    Node::Obj(Obj::new(
        pairs.into_iter().map(|(k, v)| (Val::str(k), v)).collect(),
    ))
}

pub fn arr(items: Vec<Node>) -> Node {
    Node::Arr(Arr::new(items))
}

pub fn prop(v: Node, db: Option<&str>) -> Node {
    let mut items = vec![("value", v)];
    if let Some(db) = db {
        items.push(("dbentry", s(db)));
    }
    obj(items)
}

pub fn component_id(path: &str) -> u64 {
    let d = Md5::digest(path.as_bytes());
    u64::from_be_bytes(d[..8].try_into().expect("8 bytes"))
}

pub trait NodeExt {
    fn o(&self) -> Result<&Obj>;
    fn o_mut(&mut self) -> Result<&mut Obj>;
    fn a(&self) -> Result<&Vec<Node>>;
    fn a_mut(&mut self) -> Result<&mut Vec<Node>>;
    fn v(&self) -> Result<&Val>;
    fn v_mut(&mut self) -> Result<&mut Val>;
    fn is_node(&self) -> bool;
    fn slot(&self, i: usize) -> Result<&Vec<Node>>;
    fn slot_mut(&mut self, i: usize) -> Result<&mut Vec<Node>>;
    fn props(&self) -> Result<&Obj>;
    fn props_mut(&mut self) -> Result<&mut Obj>;
    fn name(&self) -> &str;
    fn child(&self, name: &str) -> Result<&Node>;
    fn child_mut(&mut self, name: &str) -> Result<&mut Node>;
    fn find_mut(&mut self, path: &str) -> Result<&mut Node>;
    fn type_code(&self) -> String;
}

impl NodeExt for Node {
    fn o(&self) -> Result<&Obj> {
        self.as_obj().ok_or_else(|| err("expected an object"))
    }

    fn o_mut(&mut self) -> Result<&mut Obj> {
        self.as_obj_mut().ok_or_else(|| err("expected an object"))
    }

    fn a(&self) -> Result<&Vec<Node>> {
        self.as_arr()
            .map(|a| &a.items)
            .ok_or_else(|| err("expected a list"))
    }

    fn a_mut(&mut self) -> Result<&mut Vec<Node>> {
        self.as_arr_mut()
            .map(|a| &mut a.items)
            .ok_or_else(|| err("expected a list"))
    }

    fn v(&self) -> Result<&Val> {
        self.as_val().ok_or_else(|| err("expected a value"))
    }

    fn v_mut(&mut self) -> Result<&mut Val> {
        self.as_val_mut().ok_or_else(|| err("expected a value"))
    }

    fn is_node(&self) -> bool {
        self.as_arr().is_some_and(|a| {
            a.items.len() == 8 && a.items[0].as_obj().is_some_and(|o| o.get("name").is_some())
        })
    }

    fn slot(&self, i: usize) -> Result<&Vec<Node>> {
        self.a()?.get(i).ok_or_else(|| err("short UI node"))?.a()
    }

    fn slot_mut(&mut self, i: usize) -> Result<&mut Vec<Node>> {
        self.a_mut()?
            .get_mut(i)
            .ok_or_else(|| err("short UI node"))?
            .a_mut()
    }

    fn props(&self) -> Result<&Obj> {
        self.a()?.first().ok_or_else(|| err("empty UI node"))?.o()
    }

    fn props_mut(&mut self) -> Result<&mut Obj> {
        self.a_mut()?
            .first_mut()
            .ok_or_else(|| err("empty UI node"))?
            .o_mut()
    }

    fn name(&self) -> &str {
        self.props()
            .ok()
            .and_then(|p| p.get("name"))
            .and_then(Node::as_val)
            .and_then(Val::as_str)
            .unwrap_or_default()
    }

    fn child(&self, name: &str) -> Result<&Node> {
        self.slot(CHILDREN)?
            .iter()
            .find(|c| c.is_node() && c.name() == name)
            .ok_or_else(|| err(format!("{} has no child {name}", self.name())))
    }

    fn child_mut(&mut self, name: &str) -> Result<&mut Node> {
        let parent = self.name().to_owned();
        self.slot_mut(CHILDREN)?
            .iter_mut()
            .find(|c| c.is_node() && c.name() == name)
            .ok_or_else(|| err(format!("{parent} has no child {name}")))
    }

    fn find_mut(&mut self, path: &str) -> Result<&mut Node> {
        let mut n = self;
        for part in path.split('.') {
            n = n.child_mut(part)?;
        }
        Ok(n)
    }

    fn type_code(&self) -> String {
        self.props()
            .ok()
            .and_then(|p| p.get("type"))
            .and_then(Node::as_val)
            .and_then(Val::as_u32)
            .map(fourcc_str)
            .unwrap_or_else(|| "?".into())
    }
}

pub trait ObjExt {
    fn at(&self, key: &str) -> Result<&Node>;
    fn at_mut(&mut self, key: &str) -> Result<&mut Node>;
    fn str_at(&self, key: &str) -> Result<&str>;
    fn drop_key(&mut self, key: &str);
}

impl ObjExt for Obj {
    fn at(&self, key: &str) -> Result<&Node> {
        self.get(key).ok_or_else(|| err(format!("missing key {key}")))
    }

    fn at_mut(&mut self, key: &str) -> Result<&mut Node> {
        self.get_mut(key).ok_or_else(|| err(format!("missing key {key}")))
    }

    fn str_at(&self, key: &str) -> Result<&str> {
        self.at(key)?
            .v()?
            .as_str()
            .ok_or_else(|| err(format!("{key} is not a string")))
    }

    fn drop_key(&mut self, key: &str) {
        self.items.retain(|(k, _)| k.as_str() != Some(key));
    }
}

pub fn set_value(o: &mut Obj, key: &str, value: Value) -> Result<()> {
    o.at_mut(key)?.v_mut()?.value = value;
    Ok(())
}

pub fn set_str(o: &mut Obj, key: &str, value: &str) -> Result<()> {
    set_value(o, key, Value::Str(value.into()))
}

pub fn set_prop_value(n: &mut Node, key: &str, value: Val) -> Result<()> {
    let p = n.props_mut()?;
    match p.get_mut(key) {
        Some(o) => o.o_mut()?.at_mut("value")?.v_mut()?.value = value.value,
        None => p.set(key, obj(vec![("value", Node::Val(value))])),
    }
    Ok(())
}

pub fn set_text_value(n: &mut Node, text: &str) -> Result<()> {
    set_str(n.props_mut()?.at_mut("text")?.o_mut()?, "value", text)
}

pub fn set_prop(n: &mut Node, key: &str, value: Node) -> Result<()> {
    n.props_mut()?.set(key, value);
    Ok(())
}

pub fn rename(n: &mut Node, name: &str) -> Result<()> {
    set_str(n.props_mut()?, "name", name)
}

fn fix_signals(list: &mut [Node]) {
    for e in list {
        let Some(o) = e.as_obj_mut() else { continue };
        let hash_of = |o: &Obj, k: &str| o.get(k).and_then(Node::as_val).and_then(Val::as_str).map(djb2);
        if let Some(h) = hash_of(o, "eventName")
            && let Some(v) = o.get_mut("event").and_then(Node::as_val_mut)
        {
            v.set_u32(h);
        }
        if let Some(h) = hash_of(o, "commandName")
            && let Some(v) = o.get_mut("command").and_then(Node::as_val_mut)
        {
            v.set_u32(h);
        }
        let Some(Node::Arr(sigs)) = o.get_mut("signals") else {
            continue;
        };
        for s in &mut sigs.items {
            let Some(s) = s.as_obj_mut() else { continue };
            for (name, hash) in [("to_object_name", "to_object"), ("to_pin_name", "to_pin")] {
                if let Some(h) = hash_of(s, name)
                    && let Some(v) = s.get_mut(hash).and_then(Node::as_val_mut)
                {
                    v.set_u32(h);
                }
            }
        }
    }
}

pub fn fix_ids(n: &mut Node, parent: Option<&str>) {
    let name = n.name().to_owned();
    let path = match parent {
        Some(p) => format!("{p}.{name}"),
        None => name.clone(),
    };
    let Some(items) = n.as_arr_mut().map(|a| &mut a.items) else {
        return;
    };
    if let Some(p) = items[PROPS].as_obj_mut() {
        if let Some(v) = p.get_mut("id").and_then(Node::as_val_mut) {
            v.set_u32(djb2(&path));
        }
        if let Some(v) = p.get_mut("nameId").and_then(Node::as_val_mut) {
            v.set_u32(djb2(&name));
        }
    }
    for i in [COMMANDS, EVENTS] {
        if let Some(a) = items[i].as_arr_mut() {
            fix_signals(&mut a.items);
        }
    }
    if let Some(lay) = items[LAYOUT].as_arr_mut() {
        for grp in &mut lay.items {
            let Some(grp) = grp.as_arr_mut() else { continue };
            for b in &mut grp.items {
                let Some(b) = b.as_obj_mut() else { continue };
                let Some(h) = b
                    .get("debugName")
                    .and_then(Node::as_val)
                    .and_then(Val::as_str)
                    .map(djb2)
                else {
                    continue;
                };
                if let Some(v) = b.get_mut("nodeId").and_then(Node::as_val_mut) {
                    v.set_u32(h);
                }
            }
        }
    }
    if let Some(kids) = items[CHILDREN].as_arr_mut() {
        for c in &mut kids.items {
            if c.is_node() {
                fix_ids(c, Some(&path));
            }
        }
    }
}

pub fn rename_refs(n: &mut Node, old: &str, new: &str) {
    match n {
        Node::Val(v) => {
            if let Some(s) = v.as_str()
                && v.ty == *b"stri"
                && (s == old || s.strip_prefix(old).is_some_and(|r| r.starts_with('.')))
            {
                let renamed = format!("{new}{}", &s[old.len()..]);
                v.set_str(renamed);
            }
        }
        Node::Obj(o) => o.items.iter_mut().for_each(|(_, v)| rename_refs(v, old, new)),
        Node::Arr(a) => a.items.iter_mut().for_each(|v| rename_refs(v, old, new)),
        Node::Iar(_) => {}
    }
}

pub fn replace_strings(n: &mut Node, map: &dyn Fn(&str) -> Option<String>) {
    match n {
        Node::Val(v) if v.ty == *b"stri" => {
            if let Some(new) = v.as_str().and_then(map) {
                v.set_str(new);
            }
        }
        Node::Obj(o) => o.items.iter_mut().for_each(|(_, v)| replace_strings(v, map)),
        Node::Arr(a) => a.items.iter_mut().for_each(|v| replace_strings(v, map)),
        _ => {}
    }
}

pub fn clone_node(src: &Node, parent: &str, new_name: &str) -> Result<Node> {
    let mut n = src.clone();
    rename_refs(
        &mut n,
        &format!("{parent}.{}", src.name()),
        &format!("{parent}.{new_name}"),
    );
    rename(&mut n, new_name)?;
    fix_ids(&mut n, Some(parent));
    Ok(n)
}

#[derive(Debug, Clone)]
pub enum Attr {
    Str(String),
    Bool(bool),
    F32(f32),
    U32(u32),
    Map(Vec<(String, Attr)>),
}

impl From<&str> for Attr {
    fn from(v: &str) -> Self {
        Attr::Str(v.into())
    }
}

impl From<String> for Attr {
    fn from(v: String) -> Self {
        Attr::Str(v)
    }
}

fn attr(v: &Attr) -> Node {
    match v {
        Attr::Map(items) => Node::Obj(Obj::new(
            items
                .iter()
                .map(|(k, x)| (Val::str(k.as_str()), attr(x)))
                .collect(),
        )),
        Attr::Bool(b) => obj(vec![("value", boolv(*b))]),
        Attr::F32(f) => obj(vec![("value", f32v(*f))]),
        Attr::U32(u) => obj(vec![("value", u32v(*u))]),
        Attr::Str(t) => obj(vec![("value", s(t))]),
    }
}

pub fn sig_with(to: &str, pin: &str, from: &str, attrs: &[(&str, Attr)]) -> Node {
    let mut items = vec![
        ("to_object", u32v(djb2(to))),
        ("to_object_name", s(to)),
        ("to_pin_name", s(pin)),
        ("to_pin", u32v(djb2(pin))),
        ("from_pin_name", s(from)),
    ];
    if !attrs.is_empty() {
        items.push((
            "attributes",
            obj(attrs.iter().map(|(k, v)| (*k, attr(v))).collect()),
        ));
    }
    obj(items)
}

pub fn sig(to: &str, pin: &str, from: &str) -> Node {
    sig_with(to, pin, from, &[])
}

pub fn event(name: &str, signals: Vec<Node>) -> Node {
    obj(vec![
        ("eventName", s(name)),
        ("signals", arr(signals)),
        ("event", u32v(djb2(name))),
    ])
}

pub fn command(name: &str, signals: Vec<Node>) -> Node {
    obj(vec![
        ("commandName", s(name)),
        ("command", u32v(djb2(name))),
        ("signals", arr(signals)),
    ])
}

pub fn behaviours(codes: &[&str]) -> Node {
    arr(codes
        .iter()
        .map(|c| obj(vec![("type", u32v(fourcc(c)))]))
        .collect())
}

pub fn layout_path(names: &[&str]) -> Node {
    arr(vec![arr(names
        .iter()
        .map(|n| obj(vec![("nodeId", u32v(djb2(n))), ("debugName", s(n))]))
        .collect())])
}

pub fn node(
    name: &str,
    kind: &str,
    props: Vec<(&str, Node)>,
    events: Vec<Node>,
    layout: &[&str],
    behav: &[&str],
) -> Node {
    let mut p = vec![
        ("id", u32v(0)),
        ("version", u32v(0)),
        ("type", u32v(fourcc(kind))),
        ("nameId", u32v(djb2(name))),
        ("name", s(name)),
    ];
    p.extend(props);
    arr(vec![
        obj(p),
        arr(vec![]),
        arr(vec![]),
        arr(events),
        if layout.is_empty() {
            arr(vec![])
        } else {
            layout_path(layout)
        },
        arr(vec![]),
        behaviours(behav),
        arr(vec![]),
    ])
}

fn event_name(e: &Node) -> Option<&str> {
    e.as_obj()?.get("eventName")?.as_val()?.as_str()
}

pub fn add_events(n: &mut Node, evs: Vec<Node>) -> Result<()> {
    let events = n.slot_mut(EVENTS)?;
    for ev in evs {
        let name = event_name(&ev).unwrap_or_default().to_owned();
        match events.iter_mut().find(|e| event_name(e) == Some(&name)) {
            Some(same) => {
                let new = ev.o()?.at("signals")?.a()?.clone();
                same.o_mut()?.at_mut("signals")?.a_mut()?.extend(new);
            }
            None => events.push(ev),
        }
    }
    Ok(())
}

pub fn raise_cmd(screen: &str, from: &str, cmd: &str, data: &str) -> Node {
    let c = Attr::Map(vec![("name".into(), cmd.into()), ("data".into(), data.into())]);
    sig_with(screen, "raise_command", from, &[("command", c)])
}

pub fn literal_label(n: &mut Node, text: &str) -> Result<()> {
    let p = n.props_mut()?;
    match p.get_mut("text") {
        None => p.set("text", prop(s(text), None)),
        Some(t) => {
            let t = t.o_mut()?;
            set_str(t, "value", text)?;
            t.drop_key("dbentry");
            t.drop_key("isloctag");
        }
    }
    set_prop_value(n, "localise", Val::bool(false))
}

pub fn literal_text(n: &mut Node, text: &str) -> Result<()> {
    let t = n.props_mut()?.at_mut("text")?.o_mut()?;
    set_str(t, "value", text)?;
    t.drop_key("dbentry");
    t.drop_key("isloctag");
    set_prop_value(n, "localise", Val::bool(false))
}

pub fn hidden(name: &str, layout: &[&str]) -> Node {
    node(
        name,
        "UIGP",
        vec![("visible", prop(boolv(false), None))],
        vec![],
        layout,
        GROUP_BEHAVIOURS,
    )
}

pub fn set_bool(n: &mut Node, key: &str, on: bool) -> Result<()> {
    set_prop(n, key, prop(boolv(on), None))
}

pub fn strip_data(n: &mut Node) -> Result<()> {
    let p = n.props_mut()?;
    p.drop_key("allowVMBinding");
    for (_, v) in &mut p.items {
        let Some(o) = v.as_obj_mut() else { continue };
        let local = o
            .get("dbentry")
            .and_then(Node::as_val)
            .and_then(Val::as_str)
            .is_some_and(|d| !d.starts_with('#'));
        if local {
            o.drop_key("dbentry");
            o.drop_key("isloctag");
        }
    }
    n.slot_mut(COMMANDS)?.clear();
    n.slot_mut(CHILDREN)?
        .retain(|c| !(c.is_node() && DECISIONS.contains(&c.type_code().as_str())));
    for c in n.slot_mut(CHILDREN)? {
        if c.is_node() {
            strip_data(c)?;
        }
    }
    Ok(())
}

pub fn drop_events_to(n: &mut Node, pins: &[&str]) -> Result<()> {
    let events = n.slot_mut(EVENTS)?;
    for e in events.iter_mut() {
        let sigs = e.o_mut()?.at_mut("signals")?.a_mut()?;
        sigs.retain(|s| {
            !s.as_obj()
                .and_then(|o| o.get("to_pin_name"))
                .and_then(Node::as_val)
                .and_then(Val::as_str)
                .is_some_and(|p| pins.contains(&p))
        });
    }
    events.retain(|e| {
        e.as_obj()
            .and_then(|o| o.get("signals"))
            .and_then(Node::as_arr)
            .is_some_and(|a| !a.items.is_empty())
    });
    for c in n.slot_mut(CHILDREN)? {
        if c.is_node() {
            drop_events_to(c, pins)?;
        }
    }
    Ok(())
}

pub fn instance(
    template: &Node,
    path: &str,
    new_name: &str,
    texts: &[(&str, &str)],
    keep_events: bool,
) -> Result<Node> {
    let mut t = template.clone();
    rename_refs(&mut t, template.name(), &format!("{path}.{new_name}"));
    rename(&mut t, new_name)?;
    strip_data(&mut t)?;
    drop_events_to(&mut t, &["command_vmContextValid"])?;
    if !keep_events {
        t.slot_mut(EVENTS)?.clear();
    }
    for (sub, text) in texts {
        literal_label(t.find_mut(sub)?, text)?;
    }
    Ok(t)
}

pub fn screen(name: &str, layout: &str, props_from: &Obj) -> Result<Node> {
    let mut p = props_from.clone();
    p.items
        .retain(|(k, _)| !matches!(k.as_str(), Some("allowVMBinding" | "sceneAttributes")));
    set_str(&mut p, "name", name)?;
    let lay = arr(vec![arr(vec![obj(vec![
        (
            "componentId",
            Node::Val(Val::raw(*b"ui64", component_id(layout).to_le_bytes())),
        ),
        ("isComponentId", boolv(true)),
        ("debugName", s(&format!("{layout}.idf"))),
    ])])]);
    let route = |key: &str| {
        command(
            key,
            vec![sig_with(
                &format!("{name}.Action"),
                "route",
                key,
                &[("key", key.into())],
            )],
        )
    };
    Ok(arr(vec![
        Node::Obj(p),
        arr(vec![]),
        arr(vec![route("open"), route("close")]),
        arr(vec![]),
        lay,
        arr(vec![]),
        behaviours(SCREEN_BEHAVIOURS),
        arr(vec![]),
    ]))
}

pub struct Frame {
    pub action: Node,
    pub audio: Node,
    pub tooltips: Node,
    pub tooltips_old: String,
    pub back: Node,
    pub watchdog: Node,
    pub seconds: f32,
}

pub struct FrameSources<'a> {
    pub action: &'a Node,
    pub audio: &'a Node,
    pub back: &'a Node,
    pub tooltips: &'a Node,
}

pub fn frame(sname: &str, src: &FrameSources, tooltip_names: &[&str]) -> Result<Frame> {
    let back_key = "back";
    let mut action = src.action.clone();
    action.slot_mut(EVENTS)?.clear();
    let mut tooltips = src.tooltips.clone();
    tooltips
        .slot_mut(CHILDREN)?
        .retain(|c| c.is_node() && tooltip_names.contains(&c.name()));
    let mut back = src.back.clone();
    rename(&mut back, "back_listener")?;
    *back.slot_mut(EVENTS)? = vec![event(
        "pressed",
        vec![
            sig(&format!("{sname}.audio_theme_trigger"), "back", "pressed"),
            sig_with(
                &format!("{sname}.Action"),
                "route",
                "pressed",
                &[("key", back_key.into())],
            ),
        ],
    )];
    let watchdog = node(
        "watchdog",
        "TIME",
        vec![],
        vec![event(
            "time_out_expired",
            vec![sig_with(
                &format!("{sname}.Action"),
                "route",
                "time_out_expired",
                &[("key", back_key.into())],
            )],
        )],
        &[],
        &[],
    );
    Ok(Frame {
        action,
        audio: src.audio.clone(),
        tooltips,
        tooltips_old: "fe_career.tooltips".into(),
        back,
        watchdog,
        seconds: 10.0,
    })
}

pub fn finish(mut scr: Node, fr: Frame, mut kids: Vec<Node>, first_focus: &str) -> Result<Node> {
    let sname = scr.name().to_owned();
    let watchdog = format!("{sname}.watchdog");
    let tips = format!("{sname}.tooltips");
    let focus = format!("{sname}.{first_focus}");
    let reset = sig_with(
        &watchdog,
        "reset",
        "opened_screen",
        &[("duration", Attr::F32(fr.seconds))],
    );
    let Frame {
        mut action,
        audio,
        mut tooltips,
        tooltips_old,
        back,
        watchdog: wd,
        ..
    } = fr;
    *action.slot_mut(EVENTS)? = vec![
        event(
            "opened_screen",
            vec![reset, sig(&focus, "focus", "opened_screen")],
        ),
        event(
            "finishedTranIn",
            vec![
                sig(&watchdog, "stop", "finishedTranIn"),
                sig(&tips, "ttActivate", "finishedTranIn"),
                sig(&focus, "focus", "finishedTranIn"),
            ],
        ),
        event(
            "finishedTranOut",
            vec![
                sig(&watchdog, "stop", "finishedTranOut"),
                sig(&tips, "ttDeactive", "finishedTranOut"),
            ],
        ),
    ];
    rename(&mut tooltips, "tooltips")?;
    rename_refs(&mut tooltips, &tooltips_old, &tips);
    for k in kids.iter_mut().filter(|k| k.is_node()) {
        let kname = k.name().to_owned();
        let first = match first_focus.strip_prefix(&format!("{kname}.")) {
            Some(rest) => k.find_mut(rest).ok(),
            None if kname == first_focus => Some(k),
            None => None,
        };
        if let Some(first) = first {
            add_events(
                first,
                vec![event("focused", vec![sig(&watchdog, "stop", "focused")])],
            )?;
            break;
        }
    }
    let mut children = vec![action, tooltips, audio, back];
    children.append(&mut kids);
    children.push(wd);
    *scr.slot_mut(CHILDREN)? = children;
    fix_ids(&mut scr, None);
    Ok(scr)
}

pub fn chain_focus(screen: &str, kids: &mut [Node], rows: &[usize]) -> Result<()> {
    for w in rows.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (na, nb) = (kids[a].name().to_owned(), kids[b].name().to_owned());
        add_events(
            &mut kids[a],
            vec![event(
                "navDown",
                vec![sig(&format!("{screen}.{nb}"), "focus", "navDown")],
            )],
        )?;
        add_events(
            &mut kids[b],
            vec![event(
                "navUp",
                vec![sig(&format!("{screen}.{na}"), "focus", "navUp")],
            )],
        )?;
    }
    Ok(())
}

pub fn ui_bytes(screens: Vec<Node>) -> Result<Vec<u8>> {
    let p = Pkg {
        name: [0; 4],
        root: obj(vec![("Screens", arr(screens))]),
        vca: Vec::new(),
        layout: None,
    };
    Ok(p.to_bytes()?)
}

pub fn uidb_var(name: &str, kind: &str, value: Node) -> Node {
    obj(vec![
        ("ValueType", s(kind)),
        ("Name", s(name)),
        ("ReadOnly", boolv(false)),
        ("Value", value),
        ("DBType", s("Variable")),
    ])
}

pub fn uidb_dict(name: &str, entries: Vec<(String, Node)>) -> Node {
    obj(vec![
        ("Name", s(name)),
        (
            "Contents",
            Node::Obj(Obj::new(
                entries.into_iter().map(|(k, v)| (Val::str(k), v)).collect(),
            )),
        ),
        ("DBType", s("Dictionary")),
    ])
}

pub fn canonical(n: &Node) -> String {
    let list = |items: Vec<String>| items.join(",");
    match n {
        Node::Val(v) => format!("{:?}:{:?}", v.ty, v.value),
        Node::Obj(o) => format!(
            "{{{} {}}}",
            o.version,
            list(
                o.items
                    .iter()
                    .map(|(k, v)| format!("{:?}={}", k.value, canonical(v)))
                    .collect()
            )
        ),
        Node::Arr(a) => format!("[{:?} {}]", a.tag, list(a.items.iter().map(canonical).collect())),
        Node::Iar(r) => format!("{r:?}"),
    }
}

pub fn edit(data: &[u8], f: impl FnOnce(&mut Pkg) -> Result<()>) -> Result<Vec<u8>> {
    let mut p = Pkg::parse(data)?;
    f(&mut p)?;
    Ok(p.to_bytes()?)
}

pub fn root_obj(p: &mut Pkg) -> Result<&mut Obj> {
    p.root.o_mut()
}

pub fn first_screen(p: &mut Pkg) -> Result<&mut Node> {
    p.root
        .o_mut()?
        .at_mut("Screens")?
        .a_mut()?
        .first_mut()
        .ok_or_else(|| err("the UI file has no screens"))
}

pub fn first_screen_of(data: &[u8]) -> Result<Node> {
    let mut p = Pkg::parse(data)?;
    Ok(first_screen(&mut p)?.clone())
}

pub const LABEL_BEHAVIOURS: &[&str] = &["NBEL", "NBMG", "NBOS", "NBXA", "NBMO"];
pub const FRACTION_BEHAVIOURS: &[&str] = &[
    "NBFC", "NBXA", "NBEL", "NBDC", "NBMG", "NBVM", "NBAN", "NBAC", "NBOS", "NBMO",
];

fn tagged(tag: [u8; 4], v: &str) -> Node {
    match &tag {
        b"stri" => s(v),
        b"ui32" => u32v(v.parse().unwrap_or(0)),
        b"si32" => i32v(v.parse().unwrap_or(0)),
        b"fp32" => f32v(v.parse().unwrap_or(0.0)),
        b"bool" => boolv(v == "true"),
        b"rgba" => Node::Val(Val::raw(
            *b"rgba",
            u32::from_str_radix(v.trim_start_matches('#'), 16)
                .unwrap_or(0xffff_ffff)
                .to_be_bytes(),
        )),
        _ => s(v),
    }
}

/// `{ value, dbentry }` of a property whose value has `tag`, with the stock labels' defaults
/// (ui32 1, text empty, fp32 0).
pub fn typed_prop(tag: [u8; 4], default: &str, db: &str) -> Node {
    prop(tagged(tag, default), Some(db))
}

/// A bool property: `value` the default while the binding is absent, `dbentry` the binding and
/// `dbentrynegate` first when it reads as not.
pub fn bool_prop(default: bool, db: Option<&str>, negate: bool) -> Node {
    let mut items = vec![("value", boolv(default))];
    if let Some(db) = db {
        items.push(("dbentry", s(db)));
    }
    if negate {
        items.insert(0, ("dbentrynegate", boolv(true)));
    }
    obj(items)
}

fn with_kids(mut n: Node, kids: Vec<Node>) -> Node {
    if let Ok(slot) = n.slot_mut(CHILDREN) {
        slot.extend(kids);
    }
    n
}

/// A group (`UIGP`) node over the scene element `layout`, with its properties and children.
pub fn group_node(name: &str, props: Vec<(&str, Node)>, layout: &[&str], kids: Vec<Node>) -> Node {
    with_kids(node(name, "UIGP", props, vec![], layout, GROUP_BEHAVIOURS), kids)
}

/// The node that shows or hides a scene element from a bound bool.
pub fn visible_node(name: &str, element: &str, default: bool, db: Option<&str>, negate: bool) -> Node {
    group_node(
        name,
        vec![("visible", bool_prop(default, db, negate))],
        &[element],
        vec![],
    )
}

/// The node that tints a scene element with a bound colour; `default` holds while the value is
/// absent (the stock nodes use white).
pub fn colour_node(name: &str, element: &str, db: &str, default: [u8; 4]) -> Node {
    group_node(
        name,
        vec![(
            "colourOverride",
            obj(vec![
                ("value", Node::Val(Val::raw(*b"rgba", default))),
                ("dbentry", s(db)),
            ]),
        )],
        &[element],
        vec![],
    )
}

/// The node that tints a scene element with one fixed colour (a text's own colour, a quad's tint).
pub fn static_colour_node(name: &str, element: &str, colour: [u8; 4]) -> Node {
    group_node(
        name,
        vec![(
            "colourOverride",
            obj(vec![("value", Node::Val(Val::raw(*b"rgba", colour)))]),
        )],
        &[element],
        vec![],
    )
}

/// The node that sets a scene element's texture from a bound value: `textureOverride` on `layout`
/// (an eXRef, then the quad inside its unit) with the tag the stock node binds it with.
pub fn texture_node(name: &str, layout: &[&str], tag: [u8; 4], default: &str, db: &str) -> Node {
    group_node(
        name,
        vec![("textureOverride", typed_prop(tag, default, db))],
        layout,
        vec![],
    )
}

/// The label (`UILB`) over the scene text `element` showing a bound value of `tag`, or a literal
/// when `db` is `None`.
pub fn label_node(
    name: &str,
    element: &str,
    tag: [u8; 4],
    db: Option<&str>,
    literal: &str,
    visible: Option<Node>,
) -> Node {
    let mut props = Vec::new();
    if let Some(v) = visible {
        props.push(("visible", v));
    }
    let default = if tag == *b"ui32" { "1" } else { literal };
    props.push((
        "text",
        match db {
            Some(db) => typed_prop(tag, default, db),
            None => prop(s(literal), None),
        },
    ));
    if db.is_none() {
        props.push(("localise", prop(boolv(false), None)));
    }
    node(name, "UILB", props, vec![], &[element], LABEL_BEHAVIOURS)
}

/// The node that drives the clip of the scene element `element` from a bound fraction.
pub fn fraction_node(name: &str, element: &str, db: &str) -> Node {
    node(
        name,
        "UIXA",
        vec![("fraction", typed_prop(*b"fp32", "0", db))],
        vec![],
        &[element],
        FRACTION_BEHAVIOURS,
    )
}

/// A multi-value decision node (`MVCD`) on `db`: one event per state, each with its signals.
pub fn mvcd_node(name: &str, db: &str, events: Vec<Node>) -> Node {
    node(
        name,
        "MVCD",
        vec![("dbEntry", prop(s(db), None))],
        events,
        &[],
        &[],
    )
}

/// A single-value decision node (`SVCD`): events `yes` and `no` by whether the value on `db`
/// equals `value` (no comparison operator means equal).
pub fn svcd_node(name: &str, tag: [u8; 4], value: &str, db: &str, events: Vec<Node>) -> Node {
    node(
        name,
        "SVCD",
        vec![
            ("value", prop(tagged(tag, value), None)),
            ("dbEntry", prop(s(db), None)),
        ],
        events,
        &[],
        &[],
    )
}

/// Adds the `trigger` and `untrigger` commands that register and query (or unregister) the
/// decision nodes `names` under `base`, the stock DRS widget's pattern.
pub fn decision_commands(group: &mut Node, base: &str, names: &[String]) -> Result<()> {
    let mut trig = Vec::new();
    let mut untrig = Vec::new();
    for n in names {
        let path = format!("{base}.{n}");
        trig.push(sig(&path, "register", "trigger"));
        trig.push(sig(&path, "query", "trigger"));
        untrig.push(sig(&path, "unregister", "untrigger"));
    }
    let cmds = group.slot_mut(COMMANDS)?;
    cmds.push(command("trigger", trig));
    cmds.push(command("untrigger", untrig));
    Ok(())
}

/// Adds a signal from `target`'s own `show` or `hide` command to `base`'s `trigger` or
/// `untrigger` pin. The command must exist exactly once.
pub fn wire_command(target: &mut Node, command_name: &str, base: &str, pin: &str) -> Result<()> {
    let owner = target.name().to_owned();
    let mut hits = target.slot_mut(COMMANDS)?.iter_mut().filter(|c| {
        c.as_obj()
            .and_then(|o| o.get("commandName"))
            .and_then(Node::as_val)
            .and_then(Val::as_str)
            == Some(command_name)
    });
    let c = hits
        .next()
        .ok_or_else(|| err(format!("{owner} has no {command_name} command")))?;
    if hits.next().is_some() {
        return Err(err(format!("{owner} has two {command_name} commands")));
    }
    c.o_mut()?
        .at_mut("signals")?
        .a_mut()?
        .push(sig(base, pin, command_name));
    Ok(())
}

/// One bound property of a template node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub node: String,
    pub prop: String,
    pub tag: Option<[u8; 4]>,
    pub db: String,
}

/// Every bound property under `x`, skipping the subtrees named in `skip`.
pub fn bindings(x: &Node, skip: &[&str], out: &mut Vec<Binding>) {
    if x.is_node() {
        if skip.contains(&x.name()) {
            return;
        }
        if let Ok(p) = x.props() {
            for (k, v) in &p.items {
                let Some(o) = v.as_obj() else { continue };
                let Some(db) = o.get("dbentry").and_then(Node::as_val).and_then(Val::as_str) else {
                    continue;
                };
                out.push(Binding {
                    node: x.name().to_owned(),
                    prop: k.as_str().unwrap_or_default().to_owned(),
                    tag: o.get("value").and_then(Node::as_val).map(|v| v.ty),
                    db: db.to_owned(),
                });
            }
        }
        if let Ok(kids) = x.slot(CHILDREN) {
            for c in kids {
                bindings(c, skip, out);
            }
        }
        return;
    }
    match x {
        Node::Arr(a) => a.items.iter().for_each(|c| bindings(c, skip, out)),
        Node::Obj(o) => o.items.iter().for_each(|(_, v)| bindings(v, skip, out)),
        _ => {}
    }
}

/// Every UI node under `x` with its dotted path (root name first).
pub fn node_paths_of<'a>(x: &'a Node, trail: &str, out: &mut Vec<(String, &'a Node)>) {
    if x.is_node() {
        let path = if trail.is_empty() {
            x.name().to_owned()
        } else {
            format!("{trail}.{}", x.name())
        };
        out.push((path.clone(), x));
        if let Ok(kids) = x.slot(CHILDREN) {
            for c in kids {
                node_paths_of(c, &path, out);
            }
        }
        return;
    }
    match x {
        Node::Arr(a) => a.items.iter().for_each(|c| node_paths_of(c, trail, out)),
        Node::Obj(o) => o.items.iter().for_each(|(_, v)| node_paths_of(v, trail, out)),
        _ => {}
    }
}

fn locate(x: &Node, trail: &str, want: &str, at: &mut Vec<usize>, hits: &mut Vec<Vec<usize>>) {
    if x.is_node() {
        let path = if trail.is_empty() {
            x.name().to_owned()
        } else {
            format!("{trail}.{}", x.name())
        };
        if path == want {
            hits.push(at.clone());
        }
        if (want == path || want.starts_with(&format!("{path}.")))
            && let Ok(kids) = x.slot(CHILDREN)
        {
            for (i, c) in kids.iter().enumerate() {
                at.extend([CHILDREN, i]);
                locate(c, &path, want, at, hits);
                at.truncate(at.len() - 2);
            }
        }
        return;
    }
    match x {
        Node::Arr(a) => {
            for (i, c) in a.items.iter().enumerate() {
                at.push(i);
                locate(c, trail, want, at, hits);
                at.pop();
            }
        }
        Node::Obj(o) => {
            for (i, (_, v)) in o.items.iter().enumerate() {
                at.push(i);
                locate(v, trail, want, at, hits);
                at.pop();
            }
        }
        _ => {}
    }
}

/// The index path of the one UI node at the dotted `path` (root name first); its id must be the
/// djb2 of the path.
pub fn find_path(root: &Node, path: &str) -> Result<Vec<usize>> {
    let mut hits = Vec::new();
    locate(root, "", path, &mut Vec::new(), &mut hits);
    let [at] = &hits[..] else {
        return Err(err(format!("{path}: {} UI nodes there", hits.len())));
    };
    Ok(at.clone())
}

fn step(x: &mut Node, i: usize, kids: bool) -> Result<&mut Node> {
    let miss = || err("the UI tree has changed since the node was found");
    if kids {
        return x.slot_mut(CHILDREN)?.get_mut(i).ok_or_else(miss);
    }
    match x {
        Node::Arr(a) => a.items.get_mut(i).ok_or_else(miss),
        Node::Obj(o) => o.items.get_mut(i).map(|(_, v)| v).ok_or_else(miss),
        _ => Err(miss()),
    }
}

/// The node at an index path from [`find_path`].
pub fn node_at_mut<'a>(root: &'a mut Node, at: &[usize]) -> Result<&'a mut Node> {
    let mut n = root;
    let mut i = 0;
    while i < at.len() {
        if n.is_node() {
            n = step(n, *at.get(i + 1).ok_or_else(|| err("short UI path"))?, true)?;
            i += 2;
        } else {
            n = step(n, at[i], false)?;
            i += 1;
        }
    }
    Ok(n)
}

/// The node at an index path from [`find_path`].
pub fn node_at<'a>(root: &'a Node, at: &[usize]) -> Result<&'a Node> {
    let mut n = root;
    let mut i = 0;
    let miss = || err("the UI tree has changed since the node was found");
    while i < at.len() {
        if n.is_node() {
            n = n
                .slot(CHILDREN)?
                .get(*at.get(i + 1).ok_or_else(miss)?)
                .ok_or_else(miss)?;
            i += 2;
        } else {
            n = match n {
                Node::Arr(a) => a.items.get(at[i]).ok_or_else(miss)?,
                Node::Obj(o) => o.items.get(at[i]).map(|(_, v)| v).ok_or_else(miss)?,
                _ => return Err(miss()),
            };
            i += 1;
        }
    }
    Ok(n)
}
