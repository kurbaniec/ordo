use crate::action::BabelError;
use crate::log;
use crate::prime::PrimeNode;
use crate::{console_error, error};
use js_sys::Uint8Array;
use serde_json::Value;
use wasm_bindgen::__rt::core::cell::{Ref, RefCell};
use wasm_bindgen::__rt::std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::MessageEvent;
use web_sys::Worker;

pub(crate) struct Transport {
    node: PrimeNode,
    ctx: Rc<Worker>,
    initialized: RefCell<bool>,
    _onmessage: Closure<dyn FnMut(MessageEvent)>,
}

impl Transport {
    pub(crate) fn new(node: PrimeNode) -> Transport {
        let ctx = Rc::new(Worker::from(JsValue::from(js_sys::global())));

        let initialized = RefCell::new(false);

        let _onmessage = Transport::build_onmessage(node.clone(), ctx.clone());
        ctx.set_onmessage(Some(_onmessage.as_ref().unchecked_ref()));

        Transport {
            node,
            ctx,
            initialized,
            _onmessage,
        }
    }

    pub(crate) fn send(&self, data: Uint8Array) {
        let res = self.ctx.post_message(&data);
        match res {
            Ok(_) => {}
            Err(err) => {
                console_log!("Main: Send-Error {:?}", err);
            }
        }
    }

    fn build_onmessage(node: PrimeNode, _ctx: Rc<Worker>) -> Closure<dyn FnMut(MessageEvent)> {
        let node = node.clone();
        Closure::wrap(Box::new(move |event: MessageEvent| {
            let data: JsValue = event.data();
            console_log!("Main: Received data: {:?}", &data);

            if node.initialized() {
                match data.into_serde::<Value>() {
                    Ok(mut data) => {
                        // TODO error handling
                        // panicked at 'called `Option::unwrap()` on a `None` value', ordo_core\src\transport.rs:60:52
                        let ident = data["ident"].take();
                        let ident = ident.as_str().unwrap();
                        let action = data["action"].take();
                        match node.value_to_action(&ident, action) {
                            Ok(action) => {
                                console_log!("Init dispatch from js...");
                                let this = node.clone();
                                spawn_local(async move {
                                    this.dispatch_internal(action).await;
                                });
                            }
                            Err(err) => {
                                if err == BabelError::MissingFunc {
                                    console_error!("Ordo Critical-Error: Conversion for Action {} not found. Was it added to the babel macro?", &ident)
                                }
                            }
                        }
                    }
                    Err(_) => {
                        console_log!("UI: Received unsupported data...");
                    }
                }
            } else {
                if data.is_undefined() {
                    node.send_state();
                    console_log!("Main: Initializing...");
                } else {
                    node.set_initialized(true);
                    console_log!("Main: Initialized!");
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>)
    }

    pub(crate) fn initialized(&self) -> bool {
        self.initialized.borrow().clone()
    }

    pub(crate) fn set_initialized(&self, initialized: bool) {
        self.initialized.replace(initialized);
    }
}

pub(crate) type TransportWrapper = RefCell<Option<Transport>>;

pub(crate) trait TransportWrapperMethods {
    fn get(&self) -> Ref<Transport>;
}

impl TransportWrapperMethods for TransportWrapper {
    fn get(&self) -> Ref<Transport> {
        Ref::map(self.borrow(), |transport| transport.as_ref().unwrap())
    }
}
