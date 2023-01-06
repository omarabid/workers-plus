use crate::error::Error;
#[cfg(feature = "queue")]
use crate::Queue;
use crate::{durable::ObjectNamespace, DynamicDispatcher, Fetcher, Result};

use js_sys::Object;
use wasm_bindgen::{prelude::*, JsCast, JsValue};
use worker_kv::KvStore;

#[wasm_bindgen]
extern "C" {
    /// Env contains any bindings you have associated with the Worker when you uploaded it.
    pub type Env;
}

impl Env {
    fn get_binding<T: EnvBinding>(&self, name: &str) -> Result<T> {
        let binding = js_sys::Reflect::get(self, &JsValue::from(name))
            .map_err(|_| Error::JsError(format!("Env does not contain binding `{}`", name)))?;
        if binding.is_undefined() {
            Err(format!("Binding `{}` is undefined.", name).into())
        } else {
            // Can't just use JsCast::dyn_into here because the type name might not be in scope
            // resulting in a terribly annoying javascript error which can't be caught
            T::get(binding)
        }
    }

    /// Access Secret value bindings added to your Worker via the UI or `wrangler`:
    /// <https://developers.cloudflare.com/workers/cli-wrangler/commands#secret>
    pub fn secret(&self, binding: &str) -> Result<Secret> {
        self.get_binding::<Secret>(binding)
    }

    /// Environment variables are defined via the `[vars]` configuration in your wrangler.toml file
    /// and are always plaintext values.
    pub fn var(&self, binding: &str) -> Result<Var> {
        self.get_binding::<Var>(binding)
    }

    /// Access a Workers KV namespace by the binding name configured in your wrangler.toml file.
    pub fn kv(&self, binding: &str) -> Result<KvStore> {
        KvStore::from_this(self, binding).map_err(From::from)
    }

    /// Access a Durable Object namespace by the binding name configured in your wrangler.toml file.
    pub fn durable_object(&self, binding: &str) -> Result<ObjectNamespace> {
        self.get_binding(binding)
    }

    /// Access a Dynamic Dispatcher for dispatching events to other workers.
    pub fn dynamic_dispatcher(&self, binding: &str) -> Result<DynamicDispatcher> {
        self.get_binding(binding)
    }

    /// Get a [Service Binding](https://developers.cloudflare.com/workers/runtime-apis/service-bindings/)
    /// for Worker-to-Worker communication.
    pub fn service(&self, binding: &str) -> Result<Fetcher> {
        self.get_binding(binding)
    }
    #[cfg(feature = "queue")]
    /// Access a Queue by the binding name configured in your wrangler.toml file.
    pub fn queue(&self, binding: &str) -> Result<Queue> {
        self.get_binding(binding)
    }
}

pub trait EnvBinding: Sized + JsCast {
    const TYPE_NAME: &'static str;

    fn get(val: JsValue) -> Result<Self> {
        let obj = Object::from(val);
        if obj.constructor().name() == Self::TYPE_NAME {
            Ok(obj.unchecked_into())
        } else {
            Err(format!("Binding cannot be cast to the type {}", Self::TYPE_NAME).into())
        }
    }
}

pub struct StringBinding(JsValue);

impl EnvBinding for StringBinding {
    const TYPE_NAME: &'static str = "String";
}

impl JsCast for StringBinding {
    fn instanceof(val: &JsValue) -> bool {
        val.is_string()
    }

    fn unchecked_from_js(val: JsValue) -> Self {
        StringBinding(val)
    }

    fn unchecked_from_js_ref(val: &JsValue) -> &Self {
        unsafe { &*(val as *const JsValue as *const Self) }
    }
}

impl AsRef<JsValue> for StringBinding {
    fn as_ref(&self) -> &wasm_bindgen::JsValue {
        unsafe { &*(&self.0 as *const JsValue) }
    }
}

impl From<JsValue> for StringBinding {
    fn from(val: JsValue) -> Self {
        StringBinding(val)
    }
}

impl From<StringBinding> for JsValue {
    fn from(sec: StringBinding) -> Self {
        sec.0
    }
}

impl ToString for StringBinding {
    fn to_string(&self) -> String {
        self.0.as_string().unwrap_or_default()
    }
}

/// A string value representing a binding to a secret in a Worker.
pub type Secret = StringBinding;
/// A string value representing a binding to an environment variable in a Worker.
pub type Var = StringBinding;
