use std::{rc::Rc, sync::Mutex};

use anyhow::anyhow;
use futures::channel::oneshot::channel;
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::HtmlImageElement;

use crate::browser;

pub async fn load_image(source: &str) -> anyhow::Result<HtmlImageElement> {
    let image = browser::new_image()?;
    let (complete_tx, complete_rx) = channel::<anyhow::Result<()>>();
    let success_tx = Rc::new(Mutex::new(Some(complete_tx)));
    let error_tx = Rc::clone(&success_tx);
    let success_callback = browser::closure_once(move || {
        if let Some(success_tx) = success_tx.lock().ok().and_then(|mut opt| opt.take()) {
            success_tx.send(Ok(()));
        }
    });

    let error_callback: Closure<dyn FnMut(JsValue)> = browser::closure_once(move |err| {
        if let Some(error_tx) = error_tx.lock().ok().and_then(|mut opt| opt.take()) {
            error_tx.send(Err(anyhow!("Error loading image: {:#?}", err)));
        }
    });

    image.set_onload(Some(success_callback.as_ref().unchecked_ref()));
    image.set_onerror(Some(error_callback.as_ref().unchecked_ref()));
    image.set_src(source);
    complete_rx.await??;
    Ok(image)
}
