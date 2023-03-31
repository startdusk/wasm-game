use anyhow::anyhow;
use futures::Future;
use wasm_bindgen::{
    closure::{WasmClosure, WasmClosureFnOnce},
    prelude::Closure,
    JsCast, JsValue,
};
use wasm_bindgen_futures::JsFuture;
use web_sys::{CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlImageElement, Window};

macro_rules! log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!($($t)*).into())
    };
}

pub fn window() -> anyhow::Result<Window> {
    web_sys::window().ok_or_else(|| anyhow!("No window found"))
}

pub fn document() -> anyhow::Result<Document> {
    window()?
        .document()
        .ok_or_else(|| anyhow!("No document found"))
}

pub fn canvas() -> anyhow::Result<HtmlCanvasElement> {
    document()?
        .get_element_by_id("canvas")
        .ok_or_else(|| anyhow!("No Canvas Element found with ID 'canvas'"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|e| anyhow!("Error converting {:#?} to HtmlCanvasElement", e))
}

pub fn context() -> anyhow::Result<CanvasRenderingContext2d> {
    canvas()?
        .get_context("2d")
        .map_err(|js_value| anyhow!("Error getting 2d context {:#?}", js_value))?
        .ok_or_else(|| anyhow!("No 2d context found"))?
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .map_err(|e| anyhow!("Error converting {:#?} to CanvasRenderingContext2d", e))
}

pub fn spawn_local<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

pub async fn fetch_with_str(resource: &str) -> anyhow::Result<JsValue> {
    JsFuture::from(window()?.fetch_with_str(resource))
        .await
        .map_err(|err| anyhow!("Error fetching {:#?}", err))
}

pub async fn fetch_json(json_path: &str) -> anyhow::Result<JsValue> {
    let resp_value = fetch_with_str(json_path).await?;
    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|e| anyhow!("Error converting {:#?} to Response", e))?;

    JsFuture::from(
        resp.json()
            .map_err(|err| anyhow!("Could not get JSON from response {:#?}", err))?,
    )
    .await
    .map_err(|err| anyhow!("Error fetching JSON {:#?}", err))
}

pub fn new_image() -> anyhow::Result<HtmlImageElement> {
    web_sys::HtmlImageElement::new()
        .map_err(|err| anyhow!("Could not create HtmlImageElement: {:#?}", err))
}

pub fn closure_once<F, A, R>(fn_once: F) -> Closure<F::FnMut>
where
    F: WasmClosureFnOnce<A, R> + 'static,
{
    Closure::once(fn_once)
}

pub type LoopClosure = Closure<dyn FnMut(f64)>;

pub fn request_animation_frame(callback: &LoopClosure) -> anyhow::Result<i32> {
    window()?
        .request_animation_frame(callback.as_ref().unchecked_ref())
        .map_err(|err| anyhow!("Cannot request animation frame: {:#?}", err))
}
// we need closure called multiple times
pub fn create_ref_closure(f: impl FnMut(f64) + 'static) -> LoopClosure {
    closure_wrap(Box::new(f))
}

pub fn closure_wrap<T: WasmClosure + ?Sized>(data: Box<T>) -> Closure<T> {
    // The wrap function on Closure creates a Closure that can be called multiple times, it needs to be wrapped
    // in a Box and stored on the heap
    Closure::wrap(data)
}
