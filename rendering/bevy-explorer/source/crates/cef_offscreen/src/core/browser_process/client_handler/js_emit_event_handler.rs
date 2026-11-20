use crate::core::browser_process::client_handler::ProcessMessageHandler;
use crate::core::prelude::{IntoString, PROCESS_MESSAGE_JS_EMIT};
use async_channel::Sender;
use bevy::prelude::Entity;
use cef::{Browser, Frame, ImplFrame, ImplListValue, ListValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct IpcEventRaw {
    pub webview: Entity,
    pub payload: String,
}

pub struct JsEmitEventHandler {
    webview: Entity,
    sender: Sender<IpcEventRaw>,
}

impl JsEmitEventHandler {
    pub const fn new(webview: Entity, sender: Sender<IpcEventRaw>) -> Self {
        Self { sender, webview }
    }
}

impl ProcessMessageHandler for JsEmitEventHandler {
    fn process_name(&self) -> &'static str {
        PROCESS_MESSAGE_JS_EMIT
    }

    fn handle_message(&self, _browser: &mut Browser, frame: &mut Frame, args: Option<ListValue>) {
        // `window.cef` exists in every frame, but only the page itself speaks to the host: a
        // frame inside it is not heard, as it is not answered (host emits go to the main frame).
        if frame.is_main() == 0 {
            return;
        }
        if let Some(args) = args {
            let event = IpcEventRaw {
                webview: self.webview, // Placeholder, should be set correctly
                payload: args.string(0).into_string(),
            };
            let _ = self.sender.send_blocking(event);
        }
    }
}
