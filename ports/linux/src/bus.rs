//! The IBus D-Bus objects (ibusfactory.c, ibusengine.c, ibusservice.c), adapted from the earlier Linux port. Each engine
//! object is one shell session (`SessionId` = the number in its path); every call goes to the shell thread (`hub.rs`)
//! and the signals it answers with are emitted, in order, before the call returns.
use crate::hub::{Msg, Out, Signal};
use crate::serial::{self, LookupTable, PropList, Text};
use shanjie_shell::SessionId;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use zbus::{
    interface,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value},
    Connection,
};

fn path(id: SessionId) -> String {
    format!("/org/freedesktop/IBus/Engine/{id}")
}

/// Emits each signal on its engine's path.
pub async fn emit(connection: &Connection, out: Out) {
    for (id, signal) in out {
        let p = path(id);
        let Ok(e) = SignalEmitter::new(connection, p.as_str()) else { continue };
        let result = match signal {
            Signal::Commit(text) => EngineObject::commit_text(&e, Text::new(&text).value()).await,
            Signal::Preedit(text, cursor, spans, visible, mode) => {
                EngineObject::update_preedit_text(&e, Text::with_attributes(&text, spans).value(), cursor, visible, mode).await
            }
            Signal::Lookup(candidates, labels, cursor, vertical, visible) => {
                let orientation = if vertical { serial::ORIENTATION_VERTICAL } else { serial::ORIENTATION_HORIZONTAL };
                EngineObject::update_lookup_table(&e, LookupTable::page(&candidates, &labels, cursor, orientation).value(), visible).await
            }
            Signal::Auxiliary(text, visible) => EngineObject::update_auxiliary_text(&e, Text::new(&text).value(), visible).await,
            Signal::Properties(props) => EngineObject::register_properties(&e, PropList::new(props).value()).await,
            Signal::RequireSurrounding => EngineObject::require_surrounding_text(&e).await,
        };
        if let Err(error) = result {
            eprintln!("ibus-engine-shanjie: signal failed: {error}");
        }
    }
}

async fn ask<T>(hub: &mpsc::Sender<Msg>, make: impl FnOnce(async_channel::Sender<T>) -> Msg) -> Option<T> {
    let (tx, rx) = async_channel::bounded(1);
    hub.send(make(tx)).ok()?;
    rx.recv().await.ok()
}

pub struct EngineObject {
    id: SessionId,
    hub: mpsc::Sender<Msg>,
}

impl EngineObject {
    async fn call(&self, connection: &Connection, make: impl FnOnce(SessionId, async_channel::Sender<Out>) -> Msg) {
        let id = self.id;
        if let Some(out) = ask(&self.hub, |tx| make(id, tx)).await {
            emit(connection, out).await;
        }
    }
}

#[interface(name = "org.freedesktop.IBus.Engine")]
impl EngineObject {
    async fn process_key_event(&self, keyval: u32, keycode: u32, state: u32, #[zbus(connection)] c: &Connection) -> bool {
        let id = self.id;
        match ask(&self.hub, |reply| Msg::Key { id, keyval, keycode, state, reply }).await {
            Some((handled, out)) => {
                emit(c, out).await;
                handled
            }
            None => false,
        }
    }
    async fn set_cursor_location(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}
    async fn set_cursor_location_relative(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}
    async fn process_hand_writing_event(&self, _coordinates: Vec<f64>) {}
    async fn cancel_hand_writing(&self, _strokes: u32) {}
    async fn set_capabilities(&self, _caps: u32) {}
    async fn property_activate(&self, name: String, state: u32, #[zbus(connection)] c: &Connection) {
        self.call(c, |id, reply| Msg::Property { id, name, state, reply }).await;
    }
    async fn property_show(&self, _name: String) {}
    async fn property_hide(&self, _name: String) {}
    async fn candidate_clicked(&self, index: u32, _button: u32, _state: u32, #[zbus(connection)] c: &Connection) {
        self.call(c, |id, reply| Msg::Click { id, index, reply }).await;
    }
    async fn focus_in(&self, #[zbus(connection)] c: &Connection) {
        self.call(c, |id, reply| Msg::FocusIn { id, client: None, reply }).await;
    }
    /// With FocusId true the daemon calls this with the client's name (`gtk3-im:gedit`), the app identity.
    async fn focus_in_id(&self, _object_path: String, client: String, #[zbus(connection)] c: &Connection) {
        self.call(c, |id, reply| Msg::FocusIn { id, client: Some(client), reply }).await;
    }
    async fn focus_out(&self, #[zbus(connection)] c: &Connection) {
        self.call(c, |id, reply| Msg::FocusOut { id, reply }).await;
    }
    async fn focus_out_id(&self, _object_path: String, #[zbus(connection)] c: &Connection) {
        self.call(c, |id, reply| Msg::FocusOut { id, reply }).await;
    }
    async fn reset(&self, #[zbus(connection)] c: &Connection) {
        self.call(c, |id, reply| Msg::Reset { id, reply }).await;
    }
    async fn enable(&self, #[zbus(connection)] c: &Connection) {
        self.call(c, |id, reply| Msg::Enable { id, reply }).await;
    }
    async fn disable(&self, #[zbus(connection)] c: &Connection) {
        self.call(c, |id, reply| Msg::Disable { id, reply }).await;
    }
    async fn page_up(&self, #[zbus(connection)] c: &Connection) {
        self.call(c, |id, reply| Msg::Page { id, up: true, reply }).await;
    }
    async fn page_down(&self, #[zbus(connection)] c: &Connection) {
        self.call(c, |id, reply| Msg::Page { id, up: false, reply }).await;
    }
    async fn cursor_up(&self) {}
    async fn cursor_down(&self) {}
    async fn set_surrounding_text(&self, text: OwnedValue, cursor_pos: u32, anchor_pos: u32) {
        let _ = self.hub.send(Msg::Surrounding { id: self.id, text: text_of(&text), cursor: cursor_pos, anchor: anchor_pos });
    }
    async fn panel_extension_received(&self, _event: OwnedValue) {}
    async fn panel_extension_register_keys(&self, _data: OwnedValue) {}

    #[zbus(property)]
    async fn set_content_type(&self, value: (u32, u32)) {
        let _ = self.hub.send(Msg::ContentType { id: self.id, purpose: value.0 });
    }
    /// true: the daemon calls FocusInId with the client name. A plain boolean although the introspection says `(b)`:
    /// the daemon reads it with `g_variant_get_boolean` (bus/engineproxy.c `_get_has_focus_id_cb`), as ibusengine.c
    /// sends it.
    #[zbus(property)]
    async fn focus_id(&self) -> bool {
        true
    }
    /// The same: the daemon asks for the surrounding text on every focus in.
    #[zbus(property)]
    async fn active_surrounding_text(&self) -> bool {
        true
    }

    #[zbus(signal)]
    async fn commit_text(e: &SignalEmitter<'_>, text: OwnedValue) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn forward_key_event(e: &SignalEmitter<'_>, keyval: u32, keycode: u32, state: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn update_preedit_text(e: &SignalEmitter<'_>, text: OwnedValue, cursor_pos: u32, visible: bool, mode: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn update_auxiliary_text(e: &SignalEmitter<'_>, text: OwnedValue, visible: bool) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn update_lookup_table(e: &SignalEmitter<'_>, table: OwnedValue, visible: bool) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn register_properties(e: &SignalEmitter<'_>, props: OwnedValue) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn update_property(e: &SignalEmitter<'_>, prop: OwnedValue) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn require_surrounding_text(e: &SignalEmitter<'_>) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn delete_surrounding_text(e: &SignalEmitter<'_>, offset: i32, nchars: u32) -> zbus::Result<()>;
}

/// The string field of an IBusText variant: ("IBusText", a{sv}, s, v).
fn text_of(value: &OwnedValue) -> String {
    let mut current: &Value = value;
    while let Value::Value(inner) = current {
        current = inner;
    }
    match current {
        Value::Structure(s) => match s.fields().get(2) {
            Some(Value::Str(text)) => text.to_string(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

/// org.freedesktop.IBus.Service on each engine path: the daemon destroys engines through it.
struct ServiceObject {
    id: SessionId,
    hub: mpsc::Sender<Msg>,
}

#[interface(name = "org.freedesktop.IBus.Service")]
impl ServiceObject {
    async fn destroy(&self, #[zbus(connection)] connection: &Connection) {
        let server = connection.object_server();
        let p = path(self.id);
        let _ = server.remove::<EngineObject, _>(p.as_str()).await;
        let _ = server.remove::<ServiceObject, _>(p.as_str()).await;
        let _ = self.hub.send(Msg::Close(self.id));
    }
}

pub struct Factory {
    engine: String,
    next: AtomicU64,
    hub: mpsc::Sender<Msg>,
}

impl Factory {
    pub fn new(engine: &str, hub: mpsc::Sender<Msg>) -> Self {
        Self { engine: engine.into(), next: AtomicU64::new(1), hub }
    }
}

#[interface(name = "org.freedesktop.IBus.Factory")]
impl Factory {
    async fn create_engine(&self, name: String, #[zbus(connection)] connection: &Connection) -> zbus::fdo::Result<OwnedObjectPath> {
        if name != self.engine {
            return Err(zbus::fdo::Error::InvalidArgs(format!("no engine named {name}")));
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let p: OwnedObjectPath = ObjectPath::try_from(path(id)).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?.into();
        let _ = self.hub.send(Msg::Open(id));
        let server = connection.object_server();
        server.at(&p, EngineObject { id, hub: self.hub.clone() }).await?;
        server.at(&p, ServiceObject { id, hub: self.hub.clone() }).await?;
        Ok(p)
    }
}
